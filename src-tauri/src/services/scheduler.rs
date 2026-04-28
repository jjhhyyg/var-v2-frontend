use crate::*;

pub(crate) fn initialize_runtime_task_state(state: &DesktopState) -> anyhow::Result<()> {
    let conn = state.open_db()?;
    let recovery_tasks = load_pending_queue_recovery_tasks(&conn)?;
    *state.runtime.pending_queue_recovery.write() = if recovery_tasks.is_empty() {
        None
    } else {
        Some(recovery_tasks)
    };
    Ok(())
}

pub(crate) fn load_pending_queue_recovery_tasks(
    conn: &Connection,
) -> anyhow::Result<Vec<QueueRecoveryTask>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, COALESCE(queue_order, 0)
           FROM analysis_tasks
          WHERE status = 'QUEUED'
          ORDER BY queue_order ASC, created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(QueueRecoveryTask {
            task_id: row.get::<_, i64>(0)?.to_string(),
            name: row.get(1)?,
            queue_order: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(crate) fn spawn_scheduler_tick_loop(state: DesktopState, app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if let Err(error) = try_schedule_tasks(&state, &app) {
                backend_log_error(format!("scheduler tick failed: {error}"));
            }
        }
    });
}

pub(crate) fn has_pending_queue_recovery(state: &DesktopState) -> bool {
    state
        .runtime
        .pending_queue_recovery
        .read()
        .as_ref()
        .map(|tasks| !tasks.is_empty())
        .unwrap_or(false)
}

pub(crate) fn try_schedule_tasks(state: &DesktopState, app: &AppHandle) -> anyhow::Result<()> {
    let _guard = state.runtime.scheduler_lock.lock();
    try_schedule_tasks_locked(state, app)
}

pub(crate) fn try_schedule_tasks_locked(
    state: &DesktopState,
    app: &AppHandle,
) -> anyhow::Result<()> {
    let runtime_state = runtime_state_response(state);
    if runtime_state.runtime_required && !runtime_state.runtime_ready {
        state.emit_scheduler_state(app);
        state.emit_resource_state(app);
        return Ok(());
    }

    if has_pending_queue_recovery(state) {
        state.emit_scheduler_state(app);
        state.emit_resource_state(app);
        return Ok(());
    }

    loop {
        let settings = state.runtime.settings.read().scheduler.clone();
        let active_count = state.runtime.active_tasks.read().len();
        if active_count >= settings.max_concurrency {
            break;
        }

        if active_count > 0 && !can_launch_additional_task(state, &settings) {
            break;
        }

        let maybe_task_id = {
            let conn = state.open_db()?;
            next_queued_task_id(&conn)?
        };

        let Some(task_id) = maybe_task_id else {
            break;
        };

        dispatch_queued_task(state, app, task_id)?;
    }

    emit_queue_updates(state, app)?;
    state.emit_scheduler_state(app);
    state.emit_resource_state(app);
    Ok(())
}

pub(crate) fn can_launch_additional_task(
    state: &DesktopState,
    settings: &SchedulerSettings,
) -> bool {
    let snapshot = state.runtime.resource_probe.lock().snapshot();
    resource_thresholds_allow_additional_task(&snapshot, settings, cfg!(target_os = "windows"))
}

pub(crate) fn resource_thresholds_allow_additional_task(
    snapshot: &ResourceSnapshot,
    settings: &SchedulerSettings,
    include_windows_gpu: bool,
) -> bool {
    let available_ratio = if snapshot.total_memory_bytes == 0 {
        1.0
    } else {
        snapshot.available_memory_bytes as f64 / snapshot.total_memory_bytes as f64
    };

    let host_resources_ok = snapshot.cpu_percent < settings.mac_cpu_limit_percent
        && available_ratio >= settings.mac_min_available_memory_ratio;
    if !host_resources_ok {
        return false;
    }

    if !include_windows_gpu {
        return true;
    }

    let Some(gpu_percent) = snapshot.gpu_percent else {
        return false;
    };
    let (Some(gpu_memory_used), Some(gpu_memory_total)) = (
        snapshot.gpu_memory_used_bytes,
        snapshot.gpu_memory_total_bytes,
    ) else {
        return false;
    };
    if gpu_memory_total == 0 {
        return false;
    }

    let available_gpu_memory_ratio =
        gpu_memory_total.saturating_sub(gpu_memory_used) as f64 / gpu_memory_total as f64;
    gpu_percent < settings.windows_gpu_limit_percent
        && available_gpu_memory_ratio >= settings.windows_min_available_gpu_memory_ratio
}

pub(crate) fn dispatch_queued_task(
    state: &DesktopState,
    app: &AppHandle,
    task_id: i64,
) -> anyhow::Result<()> {
    let mut conn = state.open_db()?;
    let task = load_task_response(&mut conn, task_id)?;
    if task.status != "QUEUED" {
        return Ok(());
    }

    let config = task.config.ok_or_else(|| anyhow!("任务缺少配置"))?;
    let initial_status = if config.enable_preprocessing {
        "PREPROCESSING"
    } else {
        "ANALYZING"
    };
    let now = now_string();
    let updated = conn.execute(
        "UPDATE analysis_tasks
         SET status = ?1,
             queue_order = NULL,
             started_at = ?2,
             preprocessing_completed_at = CASE WHEN ?1 = 'ANALYZING' THEN ?2 ELSE NULL END,
             completed_at = NULL,
             failure_reason = NULL
         WHERE id = ?3 AND status = 'QUEUED'",
        params![initial_status, now, task_id],
    )?;
    if updated == 0 {
        return Ok(());
    }

    let response = TaskStatusResponse {
        task_id: task_id.to_string(),
        status: initial_status.to_string(),
        phase: Some("任务已启动".to_string()),
        progress: Some(0.0),
        current_frame: None,
        total_frames: None,
        preprocessing_duration: None,
        analyzing_elapsed_time: None,
        is_timeout: Some(false),
        timeout_warning: Some(false),
        failure_reason: None,
        queue_position: None,
    };

    state.runtime.active_tasks.write().insert(task_id);
    state
        .runtime
        .progress
        .write()
        .insert(task_id, response.clone());
    state.emit_status(app, &response);
    state.emit_detail(app, task_id);
    spawn_task_execution(state.clone(), app.clone(), task_id);
    Ok(())
}

pub(crate) fn spawn_task_execution(state: DesktopState, app: AppHandle, task_id: i64) {
    std::thread::spawn(move || {
        let result = run_task_worker(&state, &app, task_id);
        if let Err(error) = result {
            backend_log_error(format!("task {task_id} worker execution failed: {error:#}"));
            let _ = mark_task_failed(&state, &app, task_id, &error.to_string());
        }

        state.runtime.active_tasks.write().remove(&task_id);
        state.emit_scheduler_state(&app);
        if let Err(error) = try_schedule_tasks(&state, &app) {
            backend_log_error(format!(
                "scheduler dispatch failed after task completion: {error}"
            ));
        }
    });
}

pub(crate) fn emit_queue_updates(state: &DesktopState, app: &AppHandle) -> anyhow::Result<()> {
    let conn = state.open_db()?;
    let queued_positions = load_queued_task_positions(&conn)?;
    for (task_id, queue_position) in queued_positions {
        let response = TaskStatusResponse {
            task_id: task_id.to_string(),
            status: "QUEUED".to_string(),
            phase: Some("排队中".to_string()),
            progress: None,
            current_frame: None,
            total_frames: None,
            preprocessing_duration: None,
            analyzing_elapsed_time: None,
            is_timeout: Some(false),
            timeout_warning: Some(false),
            failure_reason: None,
            queue_position: Some(queue_position),
        };
        state.emit_status(app, &response);
        state.emit_detail(app, task_id);
    }
    Ok(())
}
pub(crate) fn enqueue_tasks(
    state: &DesktopState,
    app: &AppHandle,
    task_ids: &[i64],
    allowed_statuses: &[&str],
) -> anyhow::Result<Vec<i64>> {
    if has_pending_queue_recovery(state) {
        return Err(anyhow!("存在待恢复的排队任务，请先处理恢复弹窗"));
    }

    {
        let _guard = state.runtime.scheduler_lock.lock();
        let runtime_state = runtime_state_response(state);
        if runtime_state.runtime_required && !runtime_state.runtime_ready {
            state.emit_scheduler_state(app);
            state.emit_resource_state(app);
            return Err(anyhow!(
                "算法包不可用，请先成功导入匹配版本的算法包: {}",
                runtime_state
                    .runtime_error
                    .unwrap_or_else(|| "未知错误".to_string())
            ));
        }

        let conn = state.open_db()?;
        let mut next_order = next_queue_order(&conn)?;
        for task_id in task_ids {
            let status: String = conn.query_row(
                "SELECT status FROM analysis_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get(0),
            )?;
            if !allowed_statuses.iter().any(|item| *item == status) {
                return Err(anyhow!("当前任务状态不允许加入队列: {}", status));
            }
            conn.execute(
                "UPDATE analysis_tasks
                 SET status = 'QUEUED',
                     queue_order = ?1
                 WHERE id = ?2",
                params![next_order, task_id],
            )?;
            next_order += 1;
        }

        if let Err(error) = try_schedule_tasks_locked(state, app) {
            rollback_queued_tasks(&conn, task_ids)?;
            emit_queue_updates(state, app)?;
            state.emit_scheduler_state(app);
            state.emit_resource_state(app);
            return Err(error).context("任务加入队列后调度失败，已恢复为待启动状态");
        }
    }
    Ok(task_ids.to_vec())
}

fn rollback_queued_tasks(conn: &Connection, task_ids: &[i64]) -> anyhow::Result<()> {
    for task_id in task_ids {
        conn.execute(
            "UPDATE analysis_tasks
             SET status = 'PENDING',
                 queue_order = NULL
             WHERE id = ?1 AND status = 'QUEUED'",
            params![task_id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SchedulerSettings {
        SchedulerSettings {
            max_concurrency: 3,
            mac_cpu_limit_percent: 85.0,
            mac_min_available_memory_ratio: 0.2,
            windows_gpu_limit_percent: 60.0,
            windows_min_available_gpu_memory_ratio: 0.15,
        }
    }

    fn snapshot() -> ResourceSnapshot {
        ResourceSnapshot {
            cpu_percent: 50.0,
            available_memory_bytes: 8,
            total_memory_bytes: 16,
            gpu_percent: Some(30.0),
            gpu_memory_used_bytes: Some(4),
            gpu_memory_total_bytes: Some(16),
        }
    }

    #[test]
    fn allows_when_windows_gpu_thresholds_pass() {
        assert!(resource_thresholds_allow_additional_task(
            &snapshot(),
            &settings(),
            true
        ));
    }

    #[test]
    fn blocks_when_windows_gpu_utilization_is_too_high() {
        let mut snapshot = snapshot();
        snapshot.gpu_percent = Some(60.0);

        assert!(!resource_thresholds_allow_additional_task(
            &snapshot,
            &settings(),
            true
        ));
    }

    #[test]
    fn blocks_when_windows_gpu_memory_is_too_low() {
        let mut snapshot = snapshot();
        snapshot.gpu_memory_used_bytes = Some(14);

        assert!(!resource_thresholds_allow_additional_task(
            &snapshot,
            &settings(),
            true
        ));
    }

    #[test]
    fn blocks_when_windows_gpu_metrics_are_missing() {
        let mut snapshot = snapshot();
        snapshot.gpu_percent = None;

        assert!(!resource_thresholds_allow_additional_task(
            &snapshot,
            &settings(),
            true
        ));
    }

    #[test]
    fn ignores_gpu_metrics_for_non_windows() {
        let mut snapshot = snapshot();
        snapshot.gpu_percent = None;
        snapshot.gpu_memory_used_bytes = None;
        snapshot.gpu_memory_total_bytes = None;

        assert!(resource_thresholds_allow_additional_task(
            &snapshot,
            &settings(),
            false
        ));
    }
}
