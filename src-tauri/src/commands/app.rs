use crate::*;

#[tauri::command]
pub(crate) fn get_app_state(state: State<DesktopState>) -> CommandResult<AppStateResponse> {
    let settings = state.runtime.settings.read();
    let media_library_path = settings.media_library_path.clone();
    let initialized = media_library_path.is_some();
    let media_library_available = state.media_library_available();
    let scheduler = settings.scheduler.clone();
    drop(settings);

    Ok(AppStateResponse {
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
        active_task_count: state.runtime.active_tasks.read().len(),
        queued_task_count: state
            .queued_task_count()
            .map_err(|error| error.to_string())?,
        platform: std::env::consts::OS.to_string(),
        version: APP_VERSION.to_string(),
    })
}
