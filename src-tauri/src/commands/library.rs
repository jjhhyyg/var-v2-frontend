use crate::commands::app::build_app_state_response;
use crate::*;

#[tauri::command(async)]
pub(crate) fn initialize_media_library(
    path: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let root = PathBuf::from(path);
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "initializing", "progress": 0.15, "message": "创建媒体库目录结构" }),
    );
    ensure_library_structure(&root).map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "saving", "progress": 0.75, "message": "保存媒体库位置" }),
    );
    state
        .set_media_root(&root)
        .map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "completed", "progress": 1.0, "message": "媒体库初始化完成" }),
    );
    build_app_state_response(&state, Some(&app))
}

#[tauri::command(async)]
pub(crate) fn select_existing_media_library(
    path: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let root = PathBuf::from(path);
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "validating", "progress": 0.25, "message": "校验媒体库目录" }),
    );
    validate_library_root(&root).map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "saving", "progress": 0.75, "message": "保存媒体库位置" }),
    );
    state
        .set_media_root(&root)
        .map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "completed", "progress": 1.0, "message": "媒体库切换完成" }),
    );
    build_app_state_response(&state, Some(&app))
}

#[tauri::command(async)]
pub(crate) fn migrate_media_library(
    path: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let current_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let target_root = PathBuf::from(path);
    if current_root == target_root {
        return Err("媒体库目标目录不能与当前目录相同".to_string());
    }
    if target_root.exists()
        && fs::read_dir(&target_root)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err("迁移目标目录必须为空目录或不存在".to_string());
    }

    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "preparing", "progress": 0.0, "message": "准备迁移媒体库" }),
    );
    ensure_library_structure(&target_root).map_err(|error| error.to_string())?;

    let entries: Vec<_> = WalkDir::new(&current_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect();
    let total = entries.len().max(1);

    for (index, entry) in entries.iter().enumerate() {
        let relative = entry
            .path()
            .strip_prefix(&current_root)
            .map_err(|error| error.to_string())?;
        let target = target_root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(entry.path(), &target).map_err(|error| error.to_string())?;
        let _ = app.emit(
            "library-migration-progress",
            json!({
                "stage": "copying",
                "progress": (index + 1) as f64 / total as f64,
                "message": relative.to_string_lossy()
            }),
        );
    }

    validate_library_root(&target_root).map_err(|error| error.to_string())?;
    {
        let mut settings = state.runtime.settings.write();
        settings.recent_library_migrations.push(MigrationRecord {
            from: current_root.to_string_lossy().to_string(),
            to: target_root.to_string_lossy().to_string(),
            migrated_at: now_string(),
        });
        settings.media_library_path = Some(target_root.to_string_lossy().to_string());
    }
    state.save_settings().map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "completed", "progress": 1.0, "message": "媒体库迁移完成" }),
    );
    build_app_state_response(&state, Some(&app))
}
