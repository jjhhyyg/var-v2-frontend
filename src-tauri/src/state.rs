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
}
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

    pub(crate) fn load_task_response(&self, task_id: i64) -> anyhow::Result<TaskResponse> {
        let mut conn = self.open_db()?;
        load_task_response(&mut conn, task_id)
    }
}

impl ResourceProbe {
    pub(crate) fn new() -> Self {
        let mut system = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        system.refresh_memory();
        Self { system }
    }

    pub(crate) fn snapshot(&mut self) -> ResourceSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        ResourceSnapshot {
            cpu_percent: self.system.global_cpu_usage() as f64,
            available_memory_bytes: self.system.available_memory(),
            total_memory_bytes: self.system.total_memory(),
            gpu_percent: None,
            gpu_memory_used_bytes: None,
            gpu_memory_total_bytes: None,
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
    let runtime_cache_dir = app_local_data_dir
        .join("runtime")
        .join(APP_VERSION)
        .join(runtime_build_id());

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
