use crate::commands::app::build_app_state_response;
use crate::*;

#[tauri::command]
pub(crate) fn update_scheduler_settings(
    request: SchedulerSettingsInput,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    {
        let mut settings = state.runtime.settings.write();
        if let Some(max_concurrency) = request.max_concurrency {
            if !(1..=6).contains(&max_concurrency) {
                return Err("最大并发数必须在 1 到 6 之间".to_string());
            }
            settings.scheduler.max_concurrency = max_concurrency;
        }
        if let Some(cpu_limit) = request.mac_cpu_limit_percent {
            if !(1.0..=100.0).contains(&cpu_limit) {
                return Err("CPU 阈值必须在 1 到 100 之间".to_string());
            }
            settings.scheduler.mac_cpu_limit_percent = cpu_limit;
        }
        if let Some(memory_ratio) = request.mac_min_available_memory_ratio {
            if !(0.0..=1.0).contains(&memory_ratio) {
                return Err("可用内存阈值必须在 0 到 1 之间".to_string());
            }
            settings.scheduler.mac_min_available_memory_ratio = memory_ratio;
        }
        if let Some(gpu_limit) = request.windows_gpu_limit_percent {
            if !(1.0..=100.0).contains(&gpu_limit) {
                return Err("GPU 阈值必须在 1 到 100 之间".to_string());
            }
            settings.scheduler.windows_gpu_limit_percent = gpu_limit;
        }
        if let Some(gpu_memory_ratio) = request.windows_min_available_gpu_memory_ratio {
            if !(0.0..=1.0).contains(&gpu_memory_ratio) {
                return Err("剩余显存阈值必须在 0 到 1 之间".to_string());
            }
            settings.scheduler.windows_min_available_gpu_memory_ratio = gpu_memory_ratio;
        }
    }

    state.save_settings().map_err(|error| error.to_string())?;
    state.emit_scheduler_state(&app);
    try_schedule_tasks(&state, &app).map_err(|error| error.to_string())?;
    build_app_state_response(&state, Some(&app))
}

#[tauri::command]
pub(crate) fn get_queue_recovery_state(
    state: State<DesktopState>,
) -> CommandResult<QueueRecoveryStateResponse> {
    let tasks = state
        .runtime
        .pending_queue_recovery
        .read()
        .clone()
        .unwrap_or_default();
    Ok(QueueRecoveryStateResponse {
        has_pending_recovery: !tasks.is_empty(),
        tasks,
    })
}

#[tauri::command]
pub(crate) fn resolve_queue_recovery(
    request: ResolveQueueRecoveryRequest,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let recovery_tasks = {
        let mut pending = state.runtime.pending_queue_recovery.write();
        pending.take().unwrap_or_default()
    };

    if recovery_tasks.is_empty() {
        return Ok("当前没有待恢复的排队任务".to_string());
    }

    if request.continue_analysis {
        emit_queue_updates(&state, &app).map_err(|error| error.to_string())?;
        state.emit_scheduler_state(&app);
        try_schedule_tasks(&state, &app).map_err(|error| error.to_string())?;
        return Ok("已恢复上次排队任务".to_string());
    }

    {
        let _guard = state.runtime.scheduler_lock.lock();
        let conn = state.open_db().map_err(|error| error.to_string())?;
        cancel_recovery_tasks(&conn, &recovery_tasks).map_err(|error| error.to_string())?;
    }

    for task in &recovery_tasks {
        if let Ok(task_id) = task.task_id.parse::<i64>() {
            state.emit_detail(&app, task_id);
            let status = TaskStatusResponse {
                task_id: task.task_id.clone(),
                status: "PENDING".to_string(),
                phase: None,
                progress: None,
                current_frame: None,
                total_frames: None,
                preprocessing_duration: None,
                analyzing_elapsed_time: None,
                is_timeout: Some(false),
                timeout_warning: Some(false),
                failure_reason: None,
                queue_position: None,
            };
            state.emit_status(&app, &status);
        }
    }
    state.emit_scheduler_state(&app);
    Ok("已取消恢复并将任务改回待启动".to_string())
}
