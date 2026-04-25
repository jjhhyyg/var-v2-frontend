use crate::*;

#[derive(Clone)]
pub(crate) struct DesktopState {
    pub(crate) paths: ControlPaths,
    pub(crate) runtime: RuntimeState,
}

#[derive(Clone)]
pub(crate) struct ControlPaths {
    pub(crate) app_config_dir: PathBuf,
    pub(crate) app_local_data_dir: PathBuf,
    pub(crate) settings_path: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) db_backup_dir: PathBuf,
    pub(crate) log_dir: PathBuf,
    pub(crate) runtime_cache_dir: PathBuf,
    pub(crate) resource_dir: PathBuf,
}

#[derive(Clone)]
pub(crate) struct RuntimeState {
    pub(crate) settings: Arc<RwLock<AppSettings>>,
    pub(crate) progress: Arc<RwLock<HashMap<i64, TaskStatusResponse>>>,
    pub(crate) active_tasks: Arc<RwLock<HashSet<i64>>>,
    pub(crate) pending_queue_recovery: Arc<RwLock<Option<Vec<QueueRecoveryTask>>>>,
    pub(crate) scheduler_lock: Arc<Mutex<()>>,
    pub(crate) resource_probe: Arc<Mutex<ResourceProbe>>,
    pub(crate) media_tokens: Arc<RwLock<HashMap<String, MediaToken>>>,
    pub(crate) media_server_port: u16,
}

pub(crate) struct ResourceProbe {
    pub(crate) system: System,
    pub(crate) last_gpu_probe_warning_at: Option<Instant>,
}

const GPU_PROBE_TIMEOUT: Duration = Duration::from_millis(800);
const GPU_PROBE_WARNING_INTERVAL: Duration = Duration::from_secs(60);
const MIB_BYTES: u64 = 1024 * 1024;
impl DesktopState {
    pub(crate) fn bootstrap(app: &AppHandle) -> anyhow::Result<Self> {
        let paths = build_control_paths(app)?;
        ensure_control_dirs(&paths)?;
        init_logging(&paths.log_dir)?;
        let settings = load_settings(&paths.settings_path)?;
        init_db(&paths.db_path, &paths.db_backup_dir)?;

        let media_tokens = Arc::new(RwLock::new(HashMap::new()));
        let media_server_port = start_media_server(media_tokens.clone())?;

        let state = Self {
            paths: paths.clone(),
            runtime: RuntimeState {
                settings: Arc::new(RwLock::new(settings)),
                progress: Arc::new(RwLock::new(HashMap::new())),
                active_tasks: Arc::new(RwLock::new(HashSet::new())),
                pending_queue_recovery: Arc::new(RwLock::new(None)),
                scheduler_lock: Arc::new(Mutex::new(())),
                resource_probe: Arc::new(Mutex::new(ResourceProbe::new())),
                media_tokens,
                media_server_port,
            },
        };

        initialize_runtime_task_state(&state)?;
        state.emit_scheduler_state(app);
        spawn_scheduler_tick_loop(state.clone(), app.clone());
        Ok(state)
    }

    pub(crate) fn current_media_root(&self) -> anyhow::Result<PathBuf> {
        let settings = self.runtime.settings.read();
        let raw = settings
            .media_library_path
            .clone()
            .ok_or_else(|| anyhow!("媒体库尚未初始化"))?;
        Ok(PathBuf::from(raw))
    }

    pub(crate) fn recommended_media_library_path(&self) -> PathBuf {
        recommended_media_library_path()
    }

    pub(crate) fn media_library_available(&self) -> bool {
        self.current_media_root()
            .ok()
            .map(|root| validate_library_root(&root).is_ok())
            .unwrap_or(false)
    }

    pub(crate) fn open_db(&self) -> anyhow::Result<Connection> {
        open_db(&self.paths.db_path)
    }

    pub(crate) fn save_settings(&self) -> anyhow::Result<()> {
        save_settings(&self.paths.settings_path, &self.runtime.settings.read())
    }

    pub(crate) fn emit_status(&self, app: &AppHandle, status: &TaskStatusResponse) {
        let _ = app.emit("task-status", status.clone());
        let progress = status.progress.unwrap_or(0.0);
        let _ = app.emit(
            "task-list-update",
            json!({
                "taskId": status.task_id,
                "status": status.status,
                "progress": progress,
                "queuePosition": status.queue_position
            }),
        );
    }

    pub(crate) fn emit_detail(&self, app: &AppHandle, task_id: i64) {
        if let Ok(task) = self.load_task_response(task_id) {
            let _ = app.emit("task-detail-update", task);
        }
    }

    pub(crate) fn emit_scheduler_state(&self, app: &AppHandle) {
        if let Ok(payload) = self.scheduler_state_response() {
            let _ = app.emit("scheduler-state-update", payload);
        }
    }

    pub(crate) fn emit_resource_state(&self, app: &AppHandle) {
        let payload = self.resource_state_response();
        let _ = app.emit("resource-state-update", payload);
    }

    pub(crate) fn register_media_token(&self, path: PathBuf) -> String {
        self.prune_media_tokens();
        let token = Uuid::new_v4().to_string();
        self.runtime.media_tokens.write().insert(
            token.clone(),
            MediaToken {
                path,
                expires_at: Instant::now() + Duration::from_secs(MEDIA_EVENT_TTL_MINUTES * 60),
            },
        );
        token
    }

    pub(crate) fn prune_media_tokens(&self) {
        let now = Instant::now();
        self.runtime
            .media_tokens
            .write()
            .retain(|_, value| value.expires_at > now);
    }

    pub(crate) fn set_media_root(&self, root: &Path) -> anyhow::Result<()> {
        {
            let mut settings = self.runtime.settings.write();
            settings.media_library_path = Some(root.to_string_lossy().to_string());
        }
        self.save_settings()
    }

    pub(crate) fn queued_task_count(&self) -> anyhow::Result<usize> {
        let conn = self.open_db()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM analysis_tasks WHERE status = 'QUEUED'",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub(crate) fn scheduler_state_response(&self) -> anyhow::Result<SchedulerStateResponse> {
        let settings = self.runtime.settings.read();
        Ok(SchedulerStateResponse {
            max_concurrency: settings.scheduler.max_concurrency,
            active_task_count: self.runtime.active_tasks.read().len(),
            queued_task_count: self.queued_task_count()?,
        })
    }

    pub(crate) fn resource_state_response(&self) -> ResourceStateResponse {
        let snapshot = self.runtime.resource_probe.lock().snapshot();
        ResourceStateResponse {
            cpu_percent: clamp_percent(snapshot.cpu_percent),
            memory_used_percent: memory_used_percent(
                snapshot.available_memory_bytes,
                snapshot.total_memory_bytes,
            ),
            gpu_percent: snapshot.gpu_percent.map(clamp_percent),
            gpu_memory_used_percent: gpu_memory_used_percent(
                snapshot.gpu_memory_used_bytes,
                snapshot.gpu_memory_total_bytes,
            ),
        }
    }

    pub(crate) fn load_task_response(&self, task_id: i64) -> anyhow::Result<TaskResponse> {
        let mut conn = self.open_db()?;
        load_task_response(&mut conn, task_id)
    }
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn memory_used_percent(available_bytes: u64, total_bytes: u64) -> f64 {
    if total_bytes == 0 {
        return 0.0;
    }

    let used_ratio = 1.0 - (available_bytes as f64 / total_bytes as f64);
    clamp_percent(used_ratio * 100.0)
}

fn gpu_memory_used_percent(used_bytes: Option<u64>, total_bytes: Option<u64>) -> Option<f64> {
    let (Some(used_bytes), Some(total_bytes)) = (used_bytes, total_bytes) else {
        return None;
    };
    if total_bytes == 0 {
        return None;
    }

    Some(clamp_percent(
        used_bytes as f64 / total_bytes as f64 * 100.0,
    ))
}

impl ResourceProbe {
    pub(crate) fn new() -> Self {
        let mut system = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        system.refresh_memory();
        Self {
            system,
            last_gpu_probe_warning_at: None,
        }
    }

    pub(crate) fn snapshot(&mut self) -> ResourceSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let mut snapshot = ResourceSnapshot {
            cpu_percent: self.system.global_cpu_usage() as f64,
            available_memory_bytes: self.system.available_memory(),
            total_memory_bytes: self.system.total_memory(),
            gpu_percent: None,
            gpu_memory_used_bytes: None,
            gpu_memory_total_bytes: None,
        };

        if cfg!(target_os = "windows") {
            match probe_windows_gpu() {
                Ok(gpu) => {
                    snapshot.gpu_percent = Some(gpu.percent);
                    snapshot.gpu_memory_used_bytes = Some(gpu.memory_used_bytes);
                    snapshot.gpu_memory_total_bytes = Some(gpu.memory_total_bytes);
                    self.last_gpu_probe_warning_at = None;
                }
                Err(error) => self.log_gpu_probe_failure(&error),
            }
        }

        snapshot
    }

    fn log_gpu_probe_failure(&mut self, error: &anyhow::Error) {
        let now = Instant::now();
        let should_log = self
            .last_gpu_probe_warning_at
            .map(|last| now.duration_since(last) >= GPU_PROBE_WARNING_INTERVAL)
            .unwrap_or(true);
        if should_log {
            backend_log_info(format!("Windows GPU probe unavailable: {error}"));
            self.last_gpu_probe_warning_at = Some(now);
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResourceSnapshot {
    pub(crate) cpu_percent: f64,
    pub(crate) available_memory_bytes: u64,
    pub(crate) total_memory_bytes: u64,
    pub(crate) gpu_percent: Option<f64>,
    pub(crate) gpu_memory_used_bytes: Option<u64>,
    pub(crate) gpu_memory_total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GpuProbeSnapshot {
    pub(crate) percent: f64,
    pub(crate) memory_used_bytes: u64,
    pub(crate) memory_total_bytes: u64,
}

pub(crate) fn parse_nvidia_smi_csv(raw: &str) -> anyhow::Result<GpuProbeSnapshot> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow!("nvidia-smi 输出为空"))?;
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return Err(anyhow!("nvidia-smi 输出列数无效: {line}"));
    }

    let percent = parts[0]
        .parse::<f64>()
        .with_context(|| format!("GPU 利用率无效: {}", parts[0]))?;
    let memory_used_mib = parts[1]
        .parse::<u64>()
        .with_context(|| format!("已用显存无效: {}", parts[1]))?;
    let memory_total_mib = parts[2]
        .parse::<u64>()
        .with_context(|| format!("总显存无效: {}", parts[2]))?;
    if memory_total_mib == 0 {
        return Err(anyhow!("总显存不能为 0"));
    }

    Ok(GpuProbeSnapshot {
        percent,
        memory_used_bytes: memory_used_mib.saturating_mul(MIB_BYTES),
        memory_total_bytes: memory_total_mib.saturating_mul(MIB_BYTES),
    })
}

fn probe_windows_gpu() -> anyhow::Result<GpuProbeSnapshot> {
    let mut errors = Vec::new();
    for binary in nvidia_smi_candidates() {
        match query_nvidia_smi(&binary).and_then(|raw| parse_nvidia_smi_csv(&raw)) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) => errors.push(format!("{}: {error}", binary.display())),
        }
    }

    Err(anyhow!(errors.join("; ")))
}

fn nvidia_smi_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("nvidia-smi.exe"),
        PathBuf::from(r"C:\Windows\System32\nvidia-smi.exe"),
    ]
}

fn query_nvidia_smi(binary: &Path) -> anyhow::Result<String> {
    let mut command = Command::new(binary);
    command
        .arg("--id=0")
        .arg("--query-gpu=utilization.gpu,memory.used,memory.total")
        .arg("--format=csv,noheader,nounits")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    suppress_command_window(&mut command);

    let mut child = command
        .spawn()
        .with_context(|| format!("无法启动 {}", binary.display()))?;
    let started_at = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!(
                    "nvidia-smi 退出失败: {}; {}",
                    output.status,
                    stderr.trim()
                ));
            }
            return String::from_utf8(output.stdout).context("nvidia-smi 输出不是 UTF-8");
        }

        if started_at.elapsed() >= GPU_PROBE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("nvidia-smi 超时"));
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(crate) fn build_control_paths(app: &AppHandle) -> anyhow::Result<ControlPaths> {
    let path = app.path();
    let app_config_dir = path.app_config_dir().context("无法解析 AppConfig 目录")?;
    let app_local_data_dir = path
        .app_local_data_dir()
        .context("无法解析 AppLocalData 目录")?;
    let resource_dir = path.resource_dir().context("无法解析资源目录")?;

    let settings_path = app_config_dir.join("settings.json");
    let db_dir = app_local_data_dir.join("db");
    let db_path = db_dir.join("app.sqlite3");
    let db_backup_dir = app_local_data_dir.join("backups").join("db");
    let log_dir = app_local_data_dir.join("logs");
    let runtime_cache_dir = if cfg!(target_os = "windows") {
        app_local_data_dir.join("runtime")
    } else {
        app_local_data_dir
            .join("runtime")
            .join(APP_VERSION)
            .join(runtime_build_id())
    };

    Ok(ControlPaths {
        app_config_dir,
        app_local_data_dir,
        settings_path,
        db_path,
        db_backup_dir,
        log_dir,
        runtime_cache_dir,
        resource_dir,
    })
}

pub(crate) fn runtime_build_id() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|timestamp| timestamp.as_secs().to_string())
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|timestamp| timestamp.as_secs().to_string())
        })
        .unwrap_or_else(|| "runtime".to_string())
}

pub(crate) fn ensure_control_dirs(paths: &ControlPaths) -> anyhow::Result<()> {
    fs::create_dir_all(&paths.app_config_dir)?;
    fs::create_dir_all(&paths.app_local_data_dir)?;
    fs::create_dir_all(paths.db_path.parent().context("db 目录缺失")?)?;
    fs::create_dir_all(&paths.db_backup_dir)?;
    fs::create_dir_all(&paths.log_dir)?;
    fs::create_dir_all(paths.log_dir.join("backend"))?;
    fs::create_dir_all(paths.log_dir.join("worker"))?;
    fs::create_dir_all(&paths.runtime_cache_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_csv() {
        let snapshot = parse_nvidia_smi_csv("42, 1024, 8192\n").unwrap();

        assert_eq!(snapshot.percent, 42.0);
        assert_eq!(snapshot.memory_used_bytes, 1024 * MIB_BYTES);
        assert_eq!(snapshot.memory_total_bytes, 8192 * MIB_BYTES);
    }

    #[test]
    fn parses_nvidia_smi_csv_with_spaces() {
        let snapshot = parse_nvidia_smi_csv(" 7 , 512 , 4096 \r\n").unwrap();

        assert_eq!(snapshot.percent, 7.0);
        assert_eq!(snapshot.memory_used_bytes, 512 * MIB_BYTES);
        assert_eq!(snapshot.memory_total_bytes, 4096 * MIB_BYTES);
    }

    #[test]
    fn rejects_invalid_nvidia_smi_csv() {
        assert!(parse_nvidia_smi_csv("not,a,number").is_err());
        assert!(parse_nvidia_smi_csv("42,1024").is_err());
    }

    #[test]
    fn rejects_zero_total_gpu_memory() {
        assert!(parse_nvidia_smi_csv("42, 0, 0").is_err());
    }
}
