use crate::*;

pub(crate) fn workspace_root_from_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir
        .ancestors()
        .find(|ancestor| {
            ancestor.join("frontend").join("src-tauri").exists()
                && ancestor.join("ai-processor").exists()
        })
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub(crate) fn bundled_resources_root(resource_dir: &Path) -> PathBuf {
    let nested = resource_dir.join("resources");
    if nested.exists() {
        nested
    } else {
        resource_dir.to_path_buf()
    }
}

pub(crate) fn resolve_resource_file(
    paths: &ControlPaths,
    relative: &[&str],
    dev_fallback: &[&str],
) -> Option<PathBuf> {
    let bundled = relative
        .iter()
        .fold(bundled_resources_root(&paths.resource_dir), |acc, part| {
            acc.join(part)
        });
    if bundled.exists() {
        return Some(bundled);
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let dev = dev_fallback
        .iter()
        .fold(repo_root, |acc, part| acc.join(part));
    if dev.exists() { Some(dev) } else { None }
}

pub(crate) fn bundled_binary_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

pub(crate) fn runtime_platform_slug() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        (os, arch) => {
            if os == "macos" {
                "darwin-unknown"
            } else if os == "windows" {
                "windows-unknown"
            } else if arch == "x86_64" {
                "linux-x64"
            } else {
                "linux-arm64"
            }
        }
    }
}

pub(crate) fn resolve_runtime_resource_file(
    paths: &ControlPaths,
    relative: &[&str],
) -> Option<PathBuf> {
    let platform = runtime_platform_slug();
    let bundled = relative.iter().fold(
        bundled_resources_root(&paths.resource_dir)
            .join("runtime")
            .join(platform),
        |acc, part| acc.join(part),
    );
    if bundled.exists() {
        return Some(bundled);
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let dev = relative.iter().fold(
        repo_root
            .join("frontend")
            .join("src-tauri")
            .join("resources")
            .join("runtime")
            .join(platform),
        |acc, part| acc.join(part),
    );
    if dev.exists() { Some(dev) } else { None }
}

pub(crate) fn runtime_cache_platform_dir(paths: &ControlPaths) -> PathBuf {
    paths.runtime_cache_dir.join(runtime_platform_slug())
}

pub(crate) fn copy_directory_recursive(source: &Path, target: &Path) -> anyhow::Result<()> {
    for entry in WalkDir::new(source).follow_links(true) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &destination)?;
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn mark_file_executable(path: &Path) -> anyhow::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn mark_file_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

pub(crate) fn ensure_runtime_file(
    paths: &ControlPaths,
    relative: &[&str],
    cache_segments: &[&str],
    executable: bool,
) -> anyhow::Result<PathBuf> {
    let source = resolve_runtime_resource_file(paths, relative)
        .ok_or_else(|| anyhow!("缺少运行资源: {}", relative.join("/")))?;
    let target = cache_segments
        .iter()
        .fold(runtime_cache_platform_dir(paths), |acc, part| {
            acc.join(part)
        });

    let should_copy = if target.exists() {
        fs::metadata(&target)?.len() != fs::metadata(&source)?.len()
    } else {
        true
    };

    if should_copy {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &target)?;
    }

    if executable {
        mark_file_executable(&target)?;
    }

    Ok(target)
}

pub(crate) fn ensure_runtime_directory(
    paths: &ControlPaths,
    relative: &[&str],
    cache_segments: &[&str],
    executable_rel_path: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let source = resolve_runtime_resource_file(paths, relative)
        .ok_or_else(|| anyhow!("缺少运行目录资源: {}", relative.join("/")))?;
    let target = cache_segments
        .iter()
        .fold(runtime_cache_platform_dir(paths), |acc, part| {
            acc.join(part)
        });
    let executable_path = executable_rel_path.map(|path| target.join(path));
    let should_copy = !target.exists()
        || executable_path
            .as_ref()
            .map(|path| !path.exists())
            .unwrap_or(false);

    if should_copy {
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::create_dir_all(&target)?;
        copy_directory_recursive(&source, &target)?;
    }

    if let Some(executable_path) = executable_rel_path {
        mark_file_executable(&target.join(executable_path))?;
    }

    Ok(target)
}

pub(crate) fn resolve_ffmpeg_path(paths: &ControlPaths) -> PathBuf {
    let binary = bundled_binary_name("ffmpeg");
    if cfg!(target_os = "windows") {
        let path = windows_active_runtime_dir(paths)
            .join("tools")
            .join(&binary);
        if path.exists() {
            return path;
        }
        return PathBuf::from(binary);
    }

    ensure_runtime_file(
        paths,
        &["tools", binary.as_str()],
        &["tools", binary.as_str()],
        true,
    )
    .unwrap_or_else(|_| PathBuf::from(binary))
}

pub(crate) fn resolve_ffprobe_path(paths: &ControlPaths) -> PathBuf {
    let binary = bundled_binary_name("ffprobe");
    if cfg!(target_os = "windows") {
        let path = windows_active_runtime_dir(paths)
            .join("tools")
            .join(&binary);
        if path.exists() {
            return path;
        }
        return PathBuf::from(binary);
    }

    ensure_runtime_file(
        paths,
        &["tools", binary.as_str()],
        &["tools", binary.as_str()],
        true,
    )
    .unwrap_or_else(|_| PathBuf::from(binary))
}

pub(crate) fn resolve_model_path(paths: &ControlPaths) -> anyhow::Result<PathBuf> {
    if cfg!(debug_assertions) {
        let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
        let dev_model = repo_root
            .join("ai-processor")
            .join("weights")
            .join("best.pt");
        if dev_model.exists() {
            return Ok(dev_model);
        }
    }

    if cfg!(target_os = "windows") {
        let path = windows_active_runtime_dir(paths)
            .join("models")
            .join("best.pt");
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!("Windows 算法包尚未导入或模型文件缺失 best.pt"));
    }

    resolve_resource_file(
        paths,
        &["models", "best.pt"],
        &["ai-processor", "weights", "best.pt"],
    )
    .ok_or_else(|| anyhow!("无法解析模型文件 best.pt"))
}

pub(crate) enum WorkerLaunch {
    Packaged {
        executable: PathBuf,
    },
    PythonScript {
        python: String,
        script: PathBuf,
        python_path: PathBuf,
    },
}

pub(crate) fn resolve_worker_launch(paths: &ControlPaths) -> anyhow::Result<WorkerLaunch> {
    if cfg!(debug_assertions) {
        if let Some(launch) = resolve_source_worker_launch(paths) {
            return Ok(launch);
        }
    }

    let packaged_name = bundled_binary_name("desktop_worker");
    if cfg!(target_os = "windows") {
        let executable = windows_active_runtime_dir(paths)
            .join("worker")
            .join("desktop_worker")
            .join(&packaged_name);
        if executable.exists() {
            return Ok(WorkerLaunch::Packaged { executable });
        }
        return Err(anyhow!("Windows 算法包尚未导入或 desktop_worker.exe 缺失"));
    }

    if resolve_runtime_resource_file(paths, &["worker", "desktop_worker", packaged_name.as_str()])
        .is_some()
    {
        let worker_dir = ensure_runtime_directory(
            paths,
            &["worker", "desktop_worker"],
            &["worker", "desktop_worker"],
            Some(packaged_name.as_str()),
        )?;
        let executable = worker_dir.join(packaged_name);
        return Ok(WorkerLaunch::Packaged { executable });
    }

    if let Some(launch) = resolve_source_worker_launch(paths) {
        return Ok(launch);
    }

    Err(anyhow!("无法解析桌面 worker 可执行文件"))
}

fn resolve_source_worker_launch(paths: &ControlPaths) -> Option<WorkerLaunch> {
    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let ai_dir = repo_root.join("ai-processor");
    let script = ai_dir.join("desktop_worker.py");
    if script.exists() {
        let python = resolve_development_python_command(paths)?;
        return Some(WorkerLaunch::PythonScript {
            python,
            script,
            python_path: ai_dir,
        });
    }
    None
}

pub(crate) fn resolve_development_python_command(paths: &ControlPaths) -> Option<String> {
    for candidate in development_python_candidates(paths) {
        if python_worker_dependencies_ready(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn development_python_candidates(paths: &ControlPaths) -> Vec<String> {
    let mut candidates = Vec::new();

    if let Ok(python) = std::env::var("TAURI_WORKER_PYTHON") {
        if !python.trim().is_empty() {
            candidates.push(python);
        }
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let frontend_dir = repo_root.join("frontend");
    if cfg!(target_os = "windows") {
        let local_venv = frontend_dir
            .join(".desktop-worker-venv")
            .join("Scripts")
            .join("python.exe");
        if local_venv.exists() {
            candidates.push(local_venv.to_string_lossy().to_string());
        }

        candidates.extend(
            windows_python_candidates()
                .into_iter()
                .filter(|candidate| candidate.exists())
                .map(|candidate| candidate.to_string_lossy().to_string()),
        );
        candidates.push("python".to_string());
    } else {
        let local_venv = frontend_dir
            .join(".desktop-worker-venv")
            .join("bin")
            .join("python");
        if local_venv.exists() {
            candidates.push(local_venv.to_string_lossy().to_string());
        }
        candidates.push("python3".to_string());
        candidates.push("python".to_string());
    }

    candidates
}

fn python_worker_dependencies_ready(python: &str) -> bool {
    let mut command = Command::new(python);
    command.args([
        "-c",
        "import cv2, torch, ultralytics, dotenv, numpy, scipy, PIL, lap",
    ]);
    suppress_command_window(&mut command)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn windows_python_candidates() -> Vec<PathBuf> {
    let conda_env_name =
        std::env::var("TAURI_WORKER_CONDA_ENV").unwrap_or_else(|_| "var-env".to_string());
    let mut candidates = Vec::new();

    if let Ok(conda_prefix) = std::env::var("CONDA_PREFIX") {
        candidates.push(PathBuf::from(conda_prefix).join("python.exe"));
    }

    if let Ok(conda_exe) = std::env::var("CONDA_EXE") {
        if let Some(root) = Path::new(&conda_exe).parent().and_then(Path::parent) {
            candidates.push(root.join("envs").join(&conda_env_name).join("python.exe"));
            candidates.push(root.join("python.exe"));
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let user_root = PathBuf::from(user_profile);
        for root in [
            user_root.join(".conda"),
            user_root.join("anaconda3"),
            user_root.join("miniconda3"),
        ] {
            candidates.push(root.join("envs").join(&conda_env_name).join("python.exe"));
            candidates.push(root.join("python.exe"));
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let python_root = PathBuf::from(local_app_data)
            .join("Programs")
            .join("Python");
        for version in ["Python312", "Python311", "Python310", "Python313"] {
            candidates.push(python_root.join(version).join("python.exe"));
        }
    }

    candidates
}
