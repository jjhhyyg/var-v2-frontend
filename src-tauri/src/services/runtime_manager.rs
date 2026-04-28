use crate::*;

pub(crate) fn required_runtime_build_id() -> String {
    APP_VERSION.to_string()
}

pub(crate) fn runtime_state_response(paths: &DesktopState) -> RuntimeStateResponse {
    runtime_state_from_paths_with_progress(&paths.paths, None)
}

pub(crate) fn runtime_state_response_with_progress(
    state: &DesktopState,
    app: Option<&AppHandle>,
) -> RuntimeStateResponse {
    runtime_state_from_paths_with_progress(&state.paths, app)
}

pub(crate) fn runtime_state_from_paths_with_progress(
    paths: &ControlPaths,
    app: Option<&AppHandle>,
) -> RuntimeStateResponse {
    let required_runtime_build_id = required_runtime_build_id();
    let runtime_platform = runtime_platform_slug().to_string();

    if cfg!(debug_assertions) {
        match validate_development_source_runtime(paths) {
            Ok(()) => {
                emit_runtime_progress(app, "dev-source", 1.0, "开发模式使用 Python 源码 worker");
                return RuntimeStateResponse {
                    runtime_required: false,
                    runtime_ready: true,
                    runtime_build_id: Some("dev-source".to_string()),
                    required_runtime_build_id,
                    runtime_platform,
                    runtime_error: None,
                };
            }
            Err(error) => {
                emit_runtime_progress(
                    app,
                    "dev-source",
                    0.5,
                    &format!("Python 源码 worker 不可用: {error}"),
                );
            }
        }
    }

    if !cfg!(target_os = "windows") {
        return RuntimeStateResponse {
            runtime_required: false,
            runtime_ready: true,
            runtime_build_id: Some(required_runtime_build_id.clone()),
            required_runtime_build_id,
            runtime_platform,
            runtime_error: None,
        };
    }

    match validate_active_windows_runtime_with_progress(paths, app) {
        Ok(manifest) => RuntimeStateResponse {
            runtime_required: true,
            runtime_ready: true,
            runtime_build_id: Some(manifest.runtime_build_id),
            required_runtime_build_id,
            runtime_platform,
            runtime_error: None,
        },
        Err(error) => RuntimeStateResponse {
            runtime_required: true,
            runtime_ready: false,
            runtime_build_id: read_active_runtime_manifest(paths)
                .ok()
                .map(|manifest| manifest.runtime_build_id),
            required_runtime_build_id,
            runtime_platform,
            runtime_error: Some(error.to_string()),
        },
    }
}

fn validate_development_source_runtime(paths: &ControlPaths) -> anyhow::Result<()> {
    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let ai_dir = repo_root.join("ai-processor");
    let script = ai_dir.join("desktop_worker.py");
    if !script.exists() {
        return Err(anyhow!("缺少 {}", script.display()));
    }

    let model = if cfg!(target_os = "windows") {
        ai_dir.join("weights").join("best.onnx")
    } else {
        ai_dir.join("weights").join("best.pt")
    };
    if !model.exists() {
        return Err(anyhow!("缺少 {}", model.display()));
    }

    resolve_development_python_command(paths)
        .ok_or_else(|| anyhow!("未找到已安装 worker 依赖的 Python 环境"))?;

    Ok(())
}

pub(crate) fn windows_runtime_platform_root(paths: &ControlPaths) -> PathBuf {
    paths.runtime_cache_dir.join(runtime_platform_slug())
}

pub(crate) fn windows_active_runtime_dir(paths: &ControlPaths) -> PathBuf {
    windows_runtime_platform_root(paths).join(required_runtime_build_id())
}

pub(crate) fn read_active_runtime_manifest(
    paths: &ControlPaths,
) -> anyhow::Result<RuntimeManifest> {
    let raw = fs::read_to_string(windows_active_runtime_dir(paths).join("runtime-manifest.json"))?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn validate_active_windows_runtime_with_progress(
    paths: &ControlPaths,
    app: Option<&AppHandle>,
) -> anyhow::Result<RuntimeManifest> {
    let manifest = read_active_runtime_manifest(paths)?;
    emit_runtime_progress(app, "manifest", 0.5, "校验算法包清单");
    validate_runtime_manifest(&manifest)?;
    emit_runtime_progress(app, "layout", 0.6, "检查算法包文件结构");
    validate_runtime_layout(&windows_active_runtime_dir(paths))?;
    validate_manifest_files_with_progress(
        &windows_active_runtime_dir(paths),
        &manifest,
        app,
        0.6,
        0.85,
    )?;
    Ok(manifest)
}

pub(crate) fn validate_runtime_manifest(manifest: &RuntimeManifest) -> anyhow::Result<()> {
    let expected_platform = runtime_platform_slug();
    let expected_runtime_build_id = required_runtime_build_id();
    if manifest.platform != expected_platform {
        return Err(anyhow!(
            "算法包平台不匹配：需要 {expected_platform}，实际 {}",
            manifest.platform
        ));
    }
    if manifest.runtime_build_id != expected_runtime_build_id {
        return Err(anyhow!(
            "算法包版本不匹配：需要 {expected_runtime_build_id}，实际 {}",
            manifest.runtime_build_id
        ));
    }
    if manifest.app_version != APP_VERSION {
        return Err(anyhow!(
            "算法包 App 版本不匹配：需要 {}，实际 {}",
            APP_VERSION,
            manifest.app_version
        ));
    }
    Ok(())
}

pub(crate) fn validate_runtime_layout(root: &Path) -> anyhow::Result<()> {
    let worker = root
        .join("worker")
        .join("desktop_worker")
        .join(bundled_binary_name("desktop_worker"));
    let ffmpeg = root.join("tools").join(bundled_binary_name("ffmpeg"));
    let ffprobe = root.join("tools").join(bundled_binary_name("ffprobe"));
    let var_gpu_preprocessor = root
        .join("tools")
        .join(bundled_binary_name("var-gpu-preprocessor"));
    let var_video_analyzer = root
        .join("tools")
        .join(bundled_binary_name("var-video-analyzer"));
    let model = root.join("models").join("best.onnx");
    for required in [
        worker,
        ffmpeg,
        ffprobe,
        var_gpu_preprocessor,
        var_video_analyzer,
        model,
    ] {
        if !required.exists() {
            return Err(anyhow!("算法包缺少文件: {}", required.display()));
        }
    }
    Ok(())
}

pub(crate) fn validate_manifest_files_with_progress(
    root: &Path,
    manifest: &RuntimeManifest,
    app: Option<&AppHandle>,
    start_progress: f64,
    end_progress: f64,
) -> anyhow::Result<()> {
    let total = manifest.files.len().max(1);
    for (index, file) in manifest.files.iter().enumerate() {
        let relative = Path::new(&file.path);
        if relative.is_absolute() || file.path.contains("..") {
            return Err(anyhow!("算法包 manifest 包含非法路径: {}", file.path));
        }
        let path = root.join(relative);
        let metadata = fs::metadata(&path)
            .with_context(|| format!("算法包缺少 manifest 文件: {}", file.path))?;
        if metadata.len() != file.size {
            return Err(anyhow!("算法包文件大小不匹配: {}", file.path));
        }
        let digest = sha256_file(&path)?;
        if !digest.eq_ignore_ascii_case(&file.sha256) {
            return Err(anyhow!("算法包文件校验失败: {}", file.path));
        }
        let ratio = (index + 1) as f64 / total as f64;
        let progress = start_progress + (end_progress - start_progress) * ratio;
        emit_runtime_progress(app, "verifying", progress, &format!("校验 {}", file.path));
    }
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn import_windows_runtime_zip(
    state: &DesktopState,
    zip_path: &Path,
    app: Option<&AppHandle>,
) -> anyhow::Result<()> {
    if !cfg!(target_os = "windows") {
        return Err(anyhow!("当前平台不需要导入 Windows 算法包"));
    }
    if !zip_path.exists() {
        return Err(anyhow!("算法包不存在: {}", zip_path.display()));
    }

    backend_log_info(format!(
        "[runtime-import] start zip={} exists=true",
        zip_path.display()
    ));
    emit_runtime_progress(app, "preparing", 0.02, "准备导入算法包");
    validate_runtime_package_zip(&state.paths, zip_path, app)?;
    let platform_root = windows_runtime_platform_root(&state.paths);
    fs::create_dir_all(&platform_root)?;
    terminate_runtime_processes(&platform_root, "before runtime import");
    let temp_dir = platform_root.join(format!(".import-{}", Uuid::new_v4()));
    backend_log_info(format!(
        "[runtime-import] platform_root={} temp_dir={}",
        platform_root.display(),
        temp_dir.display()
    ));
    if temp_dir.exists() {
        backend_log_info(format!(
            "[runtime-import] remove existing temp_dir={}",
            temp_dir.display()
        ));
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let result = (|| -> anyhow::Result<()> {
        backend_log_info("[runtime-import] extracting runtime zip");
        extract_runtime_zip_with_progress(zip_path, &temp_dir, app, 0.05, 0.45)?;
        emit_runtime_progress(app, "manifest", 0.48, "读取算法包清单");
        backend_log_info("[runtime-import] reading runtime manifest");
        let manifest = read_runtime_manifest_from_dir(&temp_dir)?;
        backend_log_info(format!(
            "[runtime-import] manifest platform={} runtime_build_id={} app_version={} files={}",
            manifest.platform,
            manifest.runtime_build_id,
            manifest.app_version,
            manifest.files.len()
        ));
        emit_runtime_progress(app, "manifest", 0.52, "校验算法包清单");
        validate_runtime_manifest(&manifest)?;
        emit_runtime_progress(app, "layout", 0.58, "检查算法包文件结构");
        backend_log_info("[runtime-import] validating runtime layout");
        validate_runtime_layout(&temp_dir)?;
        backend_log_info("[runtime-import] validating runtime manifest files");
        validate_manifest_files_with_progress(&temp_dir, &manifest, app, 0.6, 0.82)?;
        emit_runtime_progress(app, "self-check", 0.84, "运行算法包自检");
        backend_log_info("[runtime-import] running runtime self-check");
        run_runtime_self_check(&temp_dir, app)?;

        emit_runtime_progress(app, "activating", 0.92, "启用算法包");
        let target = windows_active_runtime_dir(&state.paths);
        backend_log_info(format!(
            "[runtime-import] activating runtime target={}",
            target.display()
        ));
        terminate_runtime_processes(&temp_dir, "before activating imported runtime");
        if target.exists() {
            terminate_runtime_processes(&target, "before replacing active runtime");
            remove_dir_all_with_retry(&target, "删除旧算法包目录")?;
        }
        rename_with_retry(&temp_dir, &target, "启用算法包")?;
        emit_runtime_progress(app, "cleanup", 0.97, "清理旧算法包");
        cleanup_inactive_windows_runtimes(&platform_root, &target)
            .context("清理旧算法包目录失败")?;
        emit_runtime_progress(app, "completed", 1.0, "算法包导入完成");
        backend_log_info("[runtime-import] completed");
        Ok(())
    })();

    if result.is_err() && temp_dir.exists() {
        backend_log_error(format!(
            "[runtime-import] failed, removing temp_dir={}",
            temp_dir.display()
        ));
        if let Err(cleanup_error) = remove_dir_all_with_retry(&temp_dir, "清理失败的临时算法包目录")
        {
            backend_log_error(format!(
                "[runtime-import] failed to remove temp_dir={} error={cleanup_error:#}",
                temp_dir.display()
            ));
        }
    }

    if let Err(error) = &result {
        backend_log_error(format!("[runtime-import] failed: {error:#}"));
    }

    result
}

fn read_runtime_package_lock(paths: &ControlPaths) -> anyhow::Result<RuntimePackageLock> {
    let path = resolve_runtime_resource_file(paths, &["runtime-package-lock.json"])
        .ok_or_else(|| anyhow!("主程序缺少算法包 SHA256 锁定文件 runtime-package-lock.json"))?;
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("无法读取算法包 SHA256 锁定文件: {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("算法包 SHA256 锁定文件格式无效: {}", path.display()))
}

fn validate_runtime_package_zip(
    paths: &ControlPaths,
    zip_path: &Path,
    app: Option<&AppHandle>,
) -> anyhow::Result<()> {
    emit_runtime_progress(app, "package-sha256", 0.03, "校验算法包 SHA256");
    let lock = read_runtime_package_lock(paths)?;
    let expected_platform = runtime_platform_slug();
    let expected_runtime_build_id = required_runtime_build_id();
    if lock.platform != expected_platform {
        return Err(anyhow!(
            "主程序算法包锁定文件平台不匹配：需要 {expected_platform}，实际 {}",
            lock.platform
        ));
    }
    if lock.runtime_build_id != expected_runtime_build_id {
        return Err(anyhow!(
            "主程序算法包锁定文件版本不匹配：需要 {expected_runtime_build_id}，实际 {}",
            lock.runtime_build_id
        ));
    }
    if lock.app_version != APP_VERSION {
        return Err(anyhow!(
            "主程序算法包锁定文件 App 版本不匹配：需要 {}，实际 {}",
            APP_VERSION,
            lock.app_version
        ));
    }

    let metadata = fs::metadata(zip_path)
        .with_context(|| format!("无法读取算法包文件信息: {}", zip_path.display()))?;
    if metadata.len() != lock.size {
        return Err(anyhow!(
            "算法包大小不匹配：请选择主程序配套的算法包 {}",
            lock.package_name
        ));
    }
    let digest = sha256_file(zip_path)
        .with_context(|| format!("无法计算算法包 SHA256: {}", zip_path.display()))?;
    backend_log_info(format!(
        "[runtime-import] package sha256 expected={} actual={} file={}",
        lock.sha256,
        digest,
        zip_path.display()
    ));
    if !digest.eq_ignore_ascii_case(&lock.sha256) {
        return Err(anyhow!(
            "算法包 SHA256 不匹配：请选择主程序配套的算法包 {}",
            lock.package_name
        ));
    }
    Ok(())
}

fn terminate_runtime_processes(root: &Path, reason: &str) {
    if !cfg!(target_os = "windows") || !root.exists() {
        return;
    }

    let root_key = normalized_path_key(root);
    backend_log_info(format!(
        "[runtime-import] scanning runtime processes root={} reason={reason}",
        root.display()
    ));

    let mut system = System::new_all();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut killed = 0usize;
    for (pid, process) in system.processes() {
        let Some(exe) = process.exe() else {
            continue;
        };
        let exe_key = normalized_path_key(exe);
        if !exe_key.starts_with(&root_key) {
            continue;
        }

        backend_log_error(format!(
            "[runtime-import] terminating stale runtime process pid={pid} exe={} reason={reason}",
            exe.display()
        ));
        if process.kill() {
            killed += 1;
        } else {
            backend_log_error(format!(
                "[runtime-import] failed to terminate stale runtime process pid={pid} exe={}",
                exe.display()
            ));
        }
    }

    if killed > 0 {
        wait_runtime_processes_exit(root, Duration::from_secs(5));
    }
}

fn wait_runtime_processes_exit(root: &Path, timeout: Duration) {
    let root_key = normalized_path_key(root);
    let started_at = Instant::now();
    loop {
        let mut system = System::new_all();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let remaining = system.processes().values().any(|process| {
            process
                .exe()
                .map(|exe| normalized_path_key(exe).starts_with(&root_key))
                .unwrap_or(false)
        });
        if !remaining {
            backend_log_info(format!(
                "[runtime-import] stale runtime processes exited root={}",
                root.display()
            ));
            return;
        }
        if started_at.elapsed() >= timeout {
            backend_log_error(format!(
                "[runtime-import] stale runtime processes still running after {}ms root={}",
                timeout.as_millis(),
                root.display()
            ));
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn normalized_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn remove_dir_all_with_retry(path: &Path, label: &str) -> anyhow::Result<()> {
    retry_io(label, path, None, || fs::remove_dir_all(path))
}

fn rename_with_retry(source: &Path, target: &Path, label: &str) -> anyhow::Result<()> {
    retry_io(label, source, Some(target), || fs::rename(source, target))
}

fn retry_io<F>(
    label: &str,
    source: &Path,
    target: Option<&Path>,
    mut operation: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
{
    const MAX_ATTEMPTS: usize = 25;
    let mut last_error = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match operation() {
            Ok(()) => return Ok(()),
            Err(error) => {
                backend_log_error(format!(
                    "[runtime-import] {label} attempt={attempt}/{MAX_ATTEMPTS} source={} target={} error={error}",
                    source.display(),
                    target
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "<none>".to_string())
                ));
                last_error = Some(error);
                if attempt < MAX_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    let error = last_error.unwrap_or_else(|| std::io::Error::other("unknown runtime import error"));
    match target {
        Some(target) => Err(error).with_context(|| {
            format!(
                "{label}失败，无法将 {} 移动到 {}",
                source.display(),
                target.display()
            )
        }),
        None => Err(error).with_context(|| format!("{label}失败: {}", source.display())),
    }
}

pub(crate) fn read_runtime_manifest_from_dir(root: &Path) -> anyhow::Result<RuntimeManifest> {
    let raw = fs::read_to_string(root.join("runtime-manifest.json"))
        .context("算法包缺少 runtime-manifest.json")?;
    Ok(serde_json::from_str(&raw).context("runtime-manifest.json 格式无效")?)
}

pub(crate) fn extract_runtime_zip_with_progress(
    zip_path: &Path,
    target: &Path,
    app: Option<&AppHandle>,
    start_progress: f64,
    end_progress: f64,
) -> anyhow::Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("无法读取算法包 zip")?;
    let total = archive.len().max(1);

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let Some(enclosed) = file.enclosed_name() else {
            return Err(anyhow!("算法包包含非法路径: {}", file.name()));
        };
        let output = target.join(enclosed);
        if file.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output_file = fs::File::create(&output)?;
        std::io::copy(&mut file, &mut output_file)?;
        let ratio = (index + 1) as f64 / total as f64;
        let progress = start_progress + (end_progress - start_progress) * ratio;
        emit_runtime_progress(app, "extracting", progress, file.name());
    }

    Ok(())
}

fn emit_runtime_progress(app: Option<&AppHandle>, stage: &str, progress: f64, message: &str) {
    if let Some(app) = app {
        let _ = app.emit(
            "runtime-import-progress",
            json!({
                "stage": stage,
                "progress": progress.clamp(0.0, 1.0),
                "message": message
            }),
        );
    }
}

const RUNTIME_SELF_CHECK_TIMEOUT: Duration = Duration::from_secs(60);
const MIN_NVIDIA_VIDEO_CODEC_DRIVER_MAJOR: u32 = 570;

pub(crate) fn run_runtime_self_check(root: &Path, app: Option<&AppHandle>) -> anyhow::Result<()> {
    backend_log_info(format!("[runtime-self-check] root={}", root.display()));
    let worker = root
        .join("worker")
        .join("desktop_worker")
        .join(bundled_binary_name("desktop_worker"));
    let ffmpeg = root.join("tools").join(bundled_binary_name("ffmpeg"));
    let ffprobe = root.join("tools").join(bundled_binary_name("ffprobe"));
    let var_gpu_preprocessor = root
        .join("tools")
        .join(bundled_binary_name("var-gpu-preprocessor"));
    let var_video_analyzer = root
        .join("tools")
        .join(bundled_binary_name("var-video-analyzer"));

    let model = root.join("models").join("best.onnx");
    let model_arg = model.to_string_lossy().to_string();
    emit_runtime_progress(app, "self-check", 0.84, "自检 Python worker");
    run_runtime_check(&worker, &["--self-check"], "desktop_worker --self-check")?;
    emit_runtime_progress(app, "self-check", 0.855, "自检 ffmpeg");
    run_runtime_check(&ffmpeg, &["-version"], "ffmpeg -version")?;
    emit_runtime_progress(app, "self-check", 0.87, "自检 ffprobe");
    run_runtime_check(&ffprobe, &["-version"], "ffprobe -version")?;
    emit_runtime_progress(app, "self-check", 0.878, "检查 NVIDIA 驱动");
    validate_nvidia_video_codec_driver()?;
    validate_nvidia_driver_exports()?;
    emit_runtime_progress(app, "self-check", 0.885, "自检 GPU 预处理");
    run_runtime_check(
        &var_gpu_preprocessor,
        &["--self-check"],
        "var-gpu-preprocessor --self-check",
    )?;
    emit_runtime_progress(app, "self-check", 0.9, "自检缺陷检测模型");
    run_runtime_check(
        &var_video_analyzer,
        &["--self-check-onnx", "--model", model_arg.as_str()],
        "var-video-analyzer --self-check-onnx",
    )?;
    backend_log_info("[runtime-self-check] completed");
    Ok(())
}

pub(crate) fn run_runtime_check(path: &Path, args: &[&str], label: &str) -> anyhow::Result<()> {
    let started_at = Instant::now();
    let mut command = Command::new(path);
    command
        .args(args)
        .env("ONNX_REQUIRE_CUDA", "1")
        .env("ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }

    let cwd = path
        .parent()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    backend_log_info(format!(
        "[runtime-self-check] command start label={label} path={} cwd={} args={}",
        path.display(),
        cwd,
        args.join(" ")
    ));

    suppress_command_window(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("无法运行算法包自检: {label}"))?;

    let output = loop {
        if child.try_wait()?.is_some() {
            break child
                .wait_with_output()
                .with_context(|| format!("无法读取算法包自检输出: {label}"))?;
        }

        if started_at.elapsed() >= RUNTIME_SELF_CHECK_TIMEOUT {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .with_context(|| format!("算法包自检超时且无法读取输出: {label}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            backend_log_error(format!(
                "[runtime-self-check] command timeout label={label} elapsed_ms={} status={} stdout={} stderr={}",
                started_at.elapsed().as_millis(),
                output.status,
                log_excerpt(&stdout),
                log_excerpt(&stderr)
            ));
            let details = match (stdout.is_empty(), stderr.is_empty()) {
                (true, true) => "无输出".to_string(),
                (false, true) => stdout,
                (true, false) => stderr,
                (false, false) => format!("{stderr}\n{stdout}"),
            };
            return Err(anyhow!(
                "算法包自检超时: {label}，已等待 {} 秒\n{details}",
                RUNTIME_SELF_CHECK_TIMEOUT.as_secs()
            ));
        }

        std::thread::sleep(Duration::from_millis(50));
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    backend_log_info(format!(
        "[runtime-self-check] command finish label={label} elapsed_ms={} status={} code={:?} stdout={} stderr={}",
        started_at.elapsed().as_millis(),
        output.status,
        output.status.code(),
        log_excerpt(&stdout),
        log_excerpt(&stderr)
    ));
    if !output.status.success() {
        let details = match (stdout.is_empty(), stderr.is_empty()) {
            (true, true) => format!("退出码: {}", output.status),
            (false, true) => stdout,
            (true, false) => stderr,
            (false, false) => format!("{stderr}\n{stdout}"),
        };
        if let Some(hint) = runtime_check_exit_hint(&output.status) {
            return Err(anyhow!("算法包自检失败: {label}\n{details}\n{hint}"));
        }
        return Err(anyhow!("算法包自检失败: {label}\n{details}"));
    }
    Ok(())
}

fn log_excerpt(value: &str) -> String {
    const MAX_LOG_CHARS: usize = 4000;
    let normalized = value.replace('\r', "\\r").replace('\n', "\\n");
    if normalized.chars().count() <= MAX_LOG_CHARS {
        return normalized;
    }
    let excerpt: String = normalized.chars().take(MAX_LOG_CHARS).collect();
    format!("{excerpt}...<truncated>")
}

fn runtime_check_exit_hint(status: &std::process::ExitStatus) -> Option<&'static str> {
    match status.code().map(|code| code as u32) {
        Some(0xc0000135) => {
            Some("提示：Windows 报告缺少 DLL。请重新导入新版算法包，或检查算法包文件是否完整。")
        }
        Some(0xc0000139) => Some(
            "提示：Windows 报告 DLL 入口点不匹配。常见原因是 NVIDIA 显卡驱动版本过旧；当前算法包要求 Windows NVIDIA 驱动 570 或更高版本。",
        ),
        _ => None,
    }
}

fn validate_nvidia_video_codec_driver() -> anyhow::Result<()> {
    backend_log_info("[runtime-self-check] nvidia driver version check start");
    let output = query_nvidia_driver_version()
        .context("无法读取 NVIDIA 驱动版本，请确认已安装 NVIDIA 显卡驱动和 nvidia-smi")?;
    backend_log_info(format!(
        "[runtime-self-check] nvidia-smi driver query output={}",
        log_excerpt(output.trim())
    ));
    let first_line = output
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow!("nvidia-smi 未返回 NVIDIA 驱动版本"))?;
    let driver_version = first_line.split(',').next().unwrap_or(first_line).trim();
    let major = driver_version
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| anyhow!("无法解析 NVIDIA 驱动版本: {driver_version}"))?;

    if major < MIN_NVIDIA_VIDEO_CODEC_DRIVER_MAJOR {
        backend_log_error(format!(
            "[runtime-self-check] nvidia driver too old version={driver_version} minimum_major={MIN_NVIDIA_VIDEO_CODEC_DRIVER_MAJOR}"
        ));
        return Err(anyhow!(
            "NVIDIA 驱动版本过旧：当前 {driver_version}，算法包要求 Windows NVIDIA 驱动 {} 或更高版本。请更新显卡驱动后重新导入算法包。",
            MIN_NVIDIA_VIDEO_CODEC_DRIVER_MAJOR
        ));
    }

    backend_log_info(format!(
        "[runtime-self-check] nvidia driver version ok version={driver_version}"
    ));
    Ok(())
}

fn query_nvidia_driver_version() -> anyhow::Result<String> {
    let candidates = [
        PathBuf::from("nvidia-smi.exe"),
        PathBuf::from(r"C:\Windows\System32\nvidia-smi.exe"),
    ];
    let mut errors = Vec::new();

    for binary in candidates {
        backend_log_info(format!(
            "[runtime-self-check] nvidia-smi query candidate={}",
            binary.display()
        ));
        let mut command = Command::new(&binary);
        command
            .arg("--query-gpu=driver_version,name")
            .arg("--format=csv,noheader")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        suppress_command_window(&mut command);

        match run_nvidia_smi_query(command) {
            Ok(output) if output.status.success() => {
                backend_log_info(format!(
                    "[runtime-self-check] nvidia-smi query success candidate={} status={} stdout={} stderr={}",
                    binary.display(),
                    output.status,
                    log_excerpt(String::from_utf8_lossy(&output.stdout).trim()),
                    log_excerpt(String::from_utf8_lossy(&output.stderr).trim())
                ));
                return String::from_utf8(output.stdout).context("nvidia-smi 输出不是 UTF-8");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                backend_log_error(format!(
                    "[runtime-self-check] nvidia-smi query failed candidate={} status={} stderr={}",
                    binary.display(),
                    output.status,
                    log_excerpt(&stderr)
                ));
                errors.push(format!("{}: {}", binary.display(), stderr));
            }
            Err(error) => {
                backend_log_error(format!(
                    "[runtime-self-check] nvidia-smi query error candidate={} error={error:#}",
                    binary.display()
                ));
                errors.push(format!("{}: {error}", binary.display()));
            }
        }
    }

    Err(anyhow!(errors.join("; ")))
}

fn validate_nvidia_driver_exports() -> anyhow::Result<()> {
    backend_log_info("[runtime-self-check] nvidia driver export check start");
    validate_nvidia_driver_exports_impl()
        .inspect(|()| backend_log_info("[runtime-self-check] nvidia driver export check ok"))
        .inspect_err(|error| {
            backend_log_error(format!(
                "[runtime-self-check] nvidia driver export check failed: {error:#}"
            ));
        })
}

#[cfg(windows)]
fn validate_nvidia_driver_exports_impl() -> anyhow::Result<()> {
    use std::ffi::{CString, c_void};
    use std::os::windows::ffi::OsStrExt;

    type Hmodule = *mut c_void;

    unsafe extern "system" {
        fn LoadLibraryW(lpLibFileName: *const u16) -> Hmodule;
        fn FreeLibrary(hLibModule: Hmodule) -> i32;
        fn GetProcAddress(hModule: Hmodule, lpProcName: *const i8) -> *mut c_void;
    }

    struct Library(Hmodule);

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }

    fn load_library(name: &str) -> anyhow::Result<Library> {
        backend_log_info(format!(
            "[runtime-self-check] load nvidia driver dll name={name}"
        ));
        let wide: Vec<u16> = std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            backend_log_error(format!(
                "[runtime-self-check] load nvidia driver dll failed name={name}"
            ));
            return Err(anyhow!("无法加载 NVIDIA 驱动 DLL: {name}"));
        }
        backend_log_info(format!(
            "[runtime-self-check] load nvidia driver dll ok name={name}"
        ));
        Ok(Library(handle))
    }

    fn require_exports(library: &Library, dll_name: &str, exports: &[&str]) -> anyhow::Result<()> {
        for export in exports {
            let proc_name = CString::new(*export)?;
            let address = unsafe { GetProcAddress(library.0, proc_name.as_ptr()) };
            if address.is_null() {
                backend_log_error(format!(
                    "[runtime-self-check] nvidia export missing dll={dll_name} export={export}"
                ));
                return Err(anyhow!(
                    "NVIDIA 驱动 DLL 入口点不匹配：{dll_name} 缺少 {export}。请确认 Windows/System32 下的 NVIDIA 驱动文件已随显卡驱动更新，必要时使用 NVIDIA 官方安装包执行“干净安装”。"
                ));
            }
            backend_log_info(format!(
                "[runtime-self-check] nvidia export ok dll={dll_name} export={export}"
            ));
        }
        Ok(())
    }

    let nvcuda = load_library("nvcuda.dll")?;
    require_exports(
        &nvcuda,
        "nvcuda.dll",
        &[
            "cuMemcpy2DUnaligned_v2",
            "cuMemFree_v2",
            "cuCtxGetCurrent",
            "cuMemcpyDtoDAsync_v2",
            "cuMemAllocPitch_v2",
            "cuMemcpy2DAsync_v2",
            "cuMemcpy2D_v2",
            "cuCtxPushCurrent_v2",
            "cuCtxPopCurrent_v2",
        ],
    )?;

    let nvcuvid = load_library("nvcuvid.dll")?;
    require_exports(
        &nvcuvid,
        "nvcuvid.dll",
        &[
            "cuvidGetSourceVideoFormat",
            "cuvidGetDecoderCaps",
            "cuvidSetVideoSourceState",
            "cuvidDestroyDecoder",
            "cuvidReconfigureDecoder",
            "cuvidDestroyVideoSource",
            "cuvidCreateVideoParser",
            "cuvidParseVideoData",
            "cuvidMapVideoFrame64",
            "cuvidUnmapVideoFrame64",
            "cuvidCtxLockCreate",
            "cuvidCtxLock",
            "cuvidCtxUnlock",
            "cuvidDestroyVideoParser",
            "cuvidCreateVideoSource",
            "cuvidGetVideoSourceState",
            "cuvidCreateDecoder",
            "cuvidDecodePicture",
        ],
    )?;

    let nvenc = load_library("nvEncodeAPI64.dll")?;
    require_exports(
        &nvenc,
        "nvEncodeAPI64.dll",
        &[
            "NvEncodeAPIGetMaxSupportedVersion",
            "NvEncodeAPICreateInstance",
        ],
    )?;

    Ok(())
}

#[cfg(not(windows))]
fn validate_nvidia_driver_exports_impl() -> anyhow::Result<()> {
    Ok(())
}

fn run_nvidia_smi_query(mut command: Command) -> anyhow::Result<std::process::Output> {
    let mut child = command.spawn().context("无法启动 nvidia-smi")?;
    let started_at = Instant::now();

    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().context("无法读取 nvidia-smi 输出");
        }

        if started_at.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("nvidia-smi 超时"));
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn cleanup_inactive_windows_runtimes(
    platform_root: &Path,
    active: &Path,
) -> anyhow::Result<()> {
    if !platform_root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(platform_root)? {
        let entry = entry?;
        let path = entry.path();
        if path == active {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
