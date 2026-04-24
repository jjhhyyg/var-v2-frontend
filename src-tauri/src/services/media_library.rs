use crate::*;

pub(crate) fn recommended_media_library_path() -> PathBuf {
    match std::env::consts::OS {
        "macos" => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Movies")
            .join("VAR Desktop Library"),
        "windows" => {
            for drive in ['D', 'E', 'F', 'G'] {
                let path = PathBuf::from(format!("{drive}:\\"));
                if path.exists() {
                    return path.join("VARDesktopData");
                }
            }
            PathBuf::from("C:\\VARDesktopData")
        }
        _ => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("VARDesktopData"),
    }
}

pub(crate) fn library_marker_path(root: &Path) -> PathBuf {
    root.join(LIBRARY_MARKER_FILENAME)
}

pub(crate) fn validate_library_root(root: &Path) -> anyhow::Result<()> {
    let marker_path = library_marker_path(root);
    if !marker_path.exists() {
        return Err(anyhow!("媒体库缺少 {}", LIBRARY_MARKER_FILENAME));
    }

    let raw = fs::read_to_string(marker_path)?;
    let marker: Value = serde_json::from_str(&raw)?;
    let identifier = marker
        .get("identifier")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("媒体库标记缺少 identifier"))?;
    if identifier != "cn.edu.ustb.hyy.var-desktop" {
        return Err(anyhow!("媒体库 identifier 不匹配"));
    }
    Ok(())
}

pub(crate) fn ensure_library_structure(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root.join("tasks"))?;
    let marker = json!({
        "identifier": "cn.edu.ustb.hyy.var-desktop",
        "version": APP_VERSION,
        "createdAt": now_string()
    });
    fs::write(
        library_marker_path(root),
        serde_json::to_string_pretty(&marker)?,
    )?;
    Ok(())
}

pub(crate) fn task_root(media_root: &Path, task_id: i64) -> PathBuf {
    media_root.join("tasks").join(task_id.to_string())
}

pub(crate) fn ensure_task_dirs(task_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(task_dir.join("input"))?;
    fs::create_dir_all(task_dir.join("work").join("tmp"))?;
    fs::create_dir_all(task_dir.join("output"))?;
    remove_empty_legacy_task_log_dir(task_dir);
    Ok(())
}

fn remove_empty_legacy_task_log_dir(task_dir: &Path) {
    let legacy_log_dir = task_dir.join("logs");
    if legacy_log_dir.exists() {
        let _ = fs::remove_dir(legacy_log_dir);
    }
}
