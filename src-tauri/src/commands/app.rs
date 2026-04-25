use crate::*;

#[tauri::command(async)]
pub(crate) fn get_app_state(
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    build_app_state_response(&state, Some(&app))
}

pub(crate) fn build_app_state_response(
    state: &DesktopState,
    app: Option<&AppHandle>,
) -> CommandResult<AppStateResponse> {
    if let Some(app) = app {
        let _ = app.emit(
            "desktop-initialization-progress",
            json!({ "stage": "settings", "progress": 0.1, "message": "读取应用配置" }),
        );
    }
    let settings = state.runtime.settings.read();
    let media_library_path = settings.media_library_path.clone();
    let initialized = media_library_path.is_some();

    if let Some(app) = app {
        let _ = app.emit(
            "desktop-initialization-progress",
            json!({ "stage": "library", "progress": 0.25, "message": "检查媒体库" }),
        );
    }
    let media_library_available = state.media_library_available();
    let scheduler = settings.scheduler.clone();
    drop(settings);

    if let Some(app) = app {
        let _ = app.emit(
            "desktop-initialization-progress",
            json!({ "stage": "runtime", "progress": 0.45, "message": "校验算法运行时" }),
        );
    }
    let runtime_state = runtime_state_response_with_progress(state, app);

    if let Some(app) = app {
        let _ = app.emit(
            "desktop-initialization-progress",
            json!({ "stage": "database", "progress": 0.9, "message": "读取任务队列" }),
        );
    }

    let response = AppStateResponse {
        initialized,
        media_library_available,
        media_library_path,
        recommended_media_library_path: state
            .recommended_media_library_path()
            .to_string_lossy()
            .to_string(),
        max_concurrency: scheduler.max_concurrency,
        mac_cpu_limit_percent: scheduler.mac_cpu_limit_percent,
        mac_min_available_memory_ratio: scheduler.mac_min_available_memory_ratio,
        windows_gpu_limit_percent: scheduler.windows_gpu_limit_percent,
        windows_min_available_gpu_memory_ratio: scheduler.windows_min_available_gpu_memory_ratio,
        active_task_count: state.runtime.active_tasks.read().len(),
        queued_task_count: state
            .queued_task_count()
            .map_err(|error| error.to_string())?,
        platform: std::env::consts::OS.to_string(),
        version: APP_VERSION.to_string(),
        runtime_required: runtime_state.runtime_required,
        runtime_ready: runtime_state.runtime_ready,
        runtime_build_id: runtime_state.runtime_build_id,
        required_runtime_build_id: runtime_state.required_runtime_build_id,
        runtime_platform: runtime_state.runtime_platform,
        runtime_error: runtime_state.runtime_error,
    };

    if let Some(app) = app {
        let _ = app.emit(
            "desktop-initialization-progress",
            json!({ "stage": "completed", "progress": 1.0, "message": "桌面环境已就绪" }),
        );
    }

    Ok(response)
}

#[tauri::command(async)]
pub(crate) fn get_resource_state(
    state: State<DesktopState>,
) -> CommandResult<ResourceStateResponse> {
    Ok(state.resource_state_response())
}

#[tauri::command]
pub(crate) fn request_app_exit(
    app: AppHandle,
    state: State<DesktopState>,
    force: bool,
) -> CommandResult<()> {
    let active_task_count = state.runtime.active_tasks.read().len();
    if active_task_count > 0 && !force {
        return Err(format!("当前仍有 {active_task_count} 个分析任务正在运行"));
    }

    app.exit(0);
    Ok(())
}
