use crate::*;

#[tauri::command(async)]
pub(crate) fn get_runtime_state(state: State<DesktopState>) -> CommandResult<RuntimeStateResponse> {
    Ok(runtime_state_response(&state))
}

#[tauri::command(async)]
pub(crate) fn import_runtime_zip(
    app: AppHandle,
    state: State<DesktopState>,
    request: ImportRuntimeRequest,
) -> CommandResult<RuntimeStateResponse> {
    if !state.runtime.active_tasks.read().is_empty() {
        return Err("当前存在运行中的分析任务，请等待任务结束后再更新算法包".to_string());
    }

    import_windows_runtime_zip(&state, Path::new(&request.path), Some(&app))
        .map_err(|error| error.to_string())?;
    Ok(runtime_state_response(&state))
}
