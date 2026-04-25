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

    let model = ai_dir.join("weights").join("best.pt");
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
    validate_manifest_files_with_progress(&windows_active_runtime_dir(paths), &manifest, app, 0.6, 0.85)?;
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
    let model = root.join("models").join("best.pt");
    for required in [worker, ffmpeg, ffprobe, model] {
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

    emit_runtime_progress(app, "preparing", 0.02, "准备导入算法包");
    let platform_root = windows_runtime_platform_root(&state.paths);
    fs::create_dir_all(&platform_root)?;
    let temp_dir = platform_root.join(format!(".import-{}", Uuid::new_v4()));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let result = (|| -> anyhow::Result<()> {
        extract_runtime_zip_with_progress(zip_path, &temp_dir, app, 0.05, 0.45)?;
        emit_runtime_progress(app, "manifest", 0.48, "读取算法包清单");
        let manifest = read_runtime_manifest_from_dir(&temp_dir)?;
        emit_runtime_progress(app, "manifest", 0.52, "校验算法包清单");
        validate_runtime_manifest(&manifest)?;
        emit_runtime_progress(app, "layout", 0.58, "检查算法包文件结构");
        validate_runtime_layout(&temp_dir)?;
        validate_manifest_files_with_progress(&temp_dir, &manifest, app, 0.6, 0.82)?;
        emit_runtime_progress(app, "self-check", 0.84, "运行算法包自检");
        run_runtime_self_check(&temp_dir)?;

        emit_runtime_progress(app, "activating", 0.92, "启用算法包");
        let target = windows_active_runtime_dir(&state.paths);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::rename(&temp_dir, &target)?;
        emit_runtime_progress(app, "cleanup", 0.97, "清理旧算法包");
        cleanup_inactive_windows_runtimes(&platform_root, &target)?;
        emit_runtime_progress(app, "completed", 1.0, "算法包导入完成");
        Ok(())
    })();

    if result.is_err() && temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    result
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

fn emit_runtime_progress(
    app: Option<&AppHandle>,
    stage: &str,
    progress: f64,
    message: &str,
) {
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

pub(crate) fn run_runtime_self_check(root: &Path) -> anyhow::Result<()> {
    let worker = root
        .join("worker")
        .join("desktop_worker")
        .join(bundled_binary_name("desktop_worker"));
    let ffmpeg = root.join("tools").join(bundled_binary_name("ffmpeg"));
    let ffprobe = root.join("tools").join(bundled_binary_name("ffprobe"));

    run_runtime_check(&worker, &["--self-check"], "desktop_worker --self-check")?;
    run_runtime_check(&ffmpeg, &["-version"], "ffmpeg -version")?;
    run_runtime_check(&ffprobe, &["-version"], "ffprobe -version")?;
    Ok(())
}

pub(crate) fn run_runtime_check(path: &Path, args: &[&str], label: &str) -> anyhow::Result<()> {
    let mut command = Command::new(path);
    command
        .args(args)
        .env("ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }

    let output = suppress_command_window(&mut command)
        .output()
        .with_context(|| format!("无法运行算法包自检: {label}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let details = match (stdout.is_empty(), stderr.is_empty()) {
            (true, true) => format!("退出码: {}", output.status),
            (false, true) => stdout,
            (true, false) => stderr,
            (false, false) => format!("{stderr}\n{stdout}"),
        };
        return Err(anyhow!("算法包自检失败: {label}\n{details}"));
    }
    Ok(())
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
