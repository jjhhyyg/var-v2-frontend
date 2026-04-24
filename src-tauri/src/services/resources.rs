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
    if dev.exists() {
        Some(dev)
    } else {
        None
    }
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
    if dev.exists() {
        Some(dev)
    } else {
        None
    }
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
    ensure_runtime_file(
        paths,
        &["tools", binary.as_str()],
        &["tools", binary.as_str()],
        true,
    )
    .unwrap_or_else(|_| PathBuf::from(binary))
}

pub(crate) fn resolve_model_path(paths: &ControlPaths) -> anyhow::Result<PathBuf> {
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
        return Some(WorkerLaunch::PythonScript {
            python: "python3".to_string(),
            script,
            python_path: ai_dir,
        });
    }
    None
}
