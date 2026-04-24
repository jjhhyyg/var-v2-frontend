use crate::*;

pub(crate) fn handle_worker_event(
    state: &DesktopState,
    app: &AppHandle,
    task_id: i64,
    event: WorkerEvent,
) -> anyhow::Result<bool> {
    match event.event_type.as_str() {
        "model_version" => {
            let model_version = event
                .payload
                .get("modelVersion")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("model_version 缺少 modelVersion"))?;
            let conn = state.open_db()?;
            conn.execute(
                "UPDATE task_configs SET model_version = ?1 WHERE task_id = ?2",
                params![model_version, task_id],
            )?;
            state.emit_detail(app, task_id);
            Ok(false)
        }
        "progress" => {
            let payload: ProgressPayload = serde_json::from_value(event.payload)?;
            let response = apply_progress_payload(state, task_id, &payload)?;
            state
                .runtime
                .progress
                .write()
                .insert(task_id, response.clone());
            state.emit_status(app, &response);
            Ok(false)
        }
        "preprocessed_video_ready" => {
            let abs_path = event
                .payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("preprocessed_video_ready 缺少 path"))?;
            let rel_path =
                relative_to_task_output(task_id, Path::new(abs_path), "preprocessed.mp4")?;
            let conn = state.open_db()?;
            conn.execute(
                "UPDATE analysis_tasks SET preprocessed_video_rel_path = ?1 WHERE id = ?2",
                params![rel_path, task_id],
            )?;
            state.emit_detail(app, task_id);
            Ok(false)
        }
        "result_video_ready" => {
            let abs_path = event
                .payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("result_video_ready 缺少 path"))?;
            let rel_path = relative_to_task_output(task_id, Path::new(abs_path), "result.mp4")?;
            let conn = state.open_db()?;
            conn.execute(
                "UPDATE analysis_tasks SET result_video_rel_path = ?1 WHERE id = ?2",
                params![rel_path, task_id],
            )?;
            state.emit_detail(app, task_id);
            Ok(false)
        }
        "result" => {
            let payload: ResultPayload = serde_json::from_value(event.payload)?;
            persist_result_payload(state, task_id, &payload)?;
            let response = final_status_response(state, task_id)?;
            state.runtime.progress.write().remove(&task_id);
            state.emit_status(app, &response);
            state.emit_detail(app, task_id);
            Ok(true)
        }
        "failed" => {
            let message = event
                .payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("分析失败");
            mark_task_failed(state, app, task_id, message)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(crate) fn apply_progress_payload(
    state: &DesktopState,
    task_id: i64,
    payload: &ProgressPayload,
) -> anyhow::Result<TaskStatusResponse> {
    let mut conn = state.open_db()?;
    let mut task = load_task_response(&mut conn, task_id)?;
    let now = now_string();
    if payload.status == "PREPROCESSING" && task.started_at.is_none() {
        conn.execute(
            "UPDATE analysis_tasks SET status = ?1, started_at = ?2 WHERE id = ?3",
            params![payload.status, now, task_id],
        )?;
        task.started_at = Some(now.clone());
    } else if payload.status == "ANALYZING" {
        let preprocessing_completed_at = task
            .preprocessing_completed_at
            .clone()
            .unwrap_or_else(|| now.clone());
        conn.execute(
            "UPDATE analysis_tasks SET status = ?1, preprocessing_completed_at = ?2 WHERE id = ?3",
            params![payload.status, preprocessing_completed_at, task_id],
        )?;
    } else {
        conn.execute(
            "UPDATE analysis_tasks SET status = ?1 WHERE id = ?2",
            params![payload.status, task_id],
        )?;
    }

    Ok(TaskStatusResponse {
        task_id: task_id.to_string(),
        status: payload.status.clone(),
        phase: payload.phase.clone(),
        progress: payload.progress,
        current_frame: payload.current_frame,
        total_frames: payload.total_frames,
        preprocessing_duration: payload.preprocessing_duration,
        analyzing_elapsed_time: payload.analyzing_elapsed_time,
        is_timeout: payload.is_timeout,
        timeout_warning: payload.timeout_warning,
        failure_reason: payload.failure_reason.clone(),
        queue_position: None,
    })
}

pub(crate) fn final_status_response(
    state: &DesktopState,
    task_id: i64,
) -> anyhow::Result<TaskStatusResponse> {
    let mut conn = state.open_db()?;
    let task = load_task_response(&mut conn, task_id)?;
    Ok(TaskStatusResponse {
        task_id: task.task_id,
        status: task.status,
        phase: None,
        progress: Some(1.0),
        current_frame: None,
        total_frames: None,
        preprocessing_duration: None,
        analyzing_elapsed_time: None,
        is_timeout: Some(task.is_timeout),
        timeout_warning: Some(false),
        failure_reason: task.failure_reason,
        queue_position: None,
    })
}

pub(crate) fn relative_to_task_output(
    _task_id: i64,
    absolute: &Path,
    fallback: &str,
) -> anyhow::Result<String> {
    let file_name = absolute
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback);
    if file_name == fallback {
        Ok(format!("output/{fallback}"))
    } else {
        Ok(format!("output/{file_name}"))
    }
}

pub(crate) fn mark_task_failed(
    state: &DesktopState,
    app: &AppHandle,
    task_id: i64,
    message: &str,
) -> anyhow::Result<()> {
    let conn = state.open_db()?;
    conn.execute(
        "UPDATE analysis_tasks
         SET status = 'FAILED',
             failure_reason = ?1,
             completed_at = ?2,
             queue_order = NULL
         WHERE id = ?3",
        params![message, now_string(), task_id],
    )?;
    let response = TaskStatusResponse {
        task_id: task_id.to_string(),
        status: "FAILED".to_string(),
        phase: None,
        progress: Some(1.0),
        current_frame: None,
        total_frames: None,
        preprocessing_duration: None,
        analyzing_elapsed_time: None,
        is_timeout: Some(false),
        timeout_warning: Some(false),
        failure_reason: Some(message.to_string()),
        queue_position: None,
    };
    state.runtime.progress.write().remove(&task_id);
    state.emit_status(app, &response);
    state.emit_detail(app, task_id);
    Ok(())
}
