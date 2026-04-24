use crate::*;

#[tauri::command]
pub(crate) fn import_video_task(
    request: ImportTaskRequest,
    state: State<DesktopState>,
) -> CommandResult<TaskResponse> {
    create_task_from_import(&state, &request.file_path, request.name, request.config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn import_video_tasks(
    request: ImportTasksRequest,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<ImportTasksResponse> {
    if request.auto_start.unwrap_or(false) && has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }

    let mut created_tasks = Vec::new();
    let mut failed_files = Vec::new();

    for item in request.items {
        match create_task_from_import(
            &state,
            &item.file_path,
            item.name.clone(),
            request.config.clone(),
        ) {
            Ok(task) => created_tasks.push(task),
            Err(error) => {
                let file_path = item.file_path.clone();
                let file_name = Path::new(&file_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&file_path)
                    .to_string();
                failed_files.push(ImportTasksFailure {
                    file_path,
                    file_name,
                    reason: error.to_string(),
                });
            }
        }
    }

    let queued_task_ids = if request.auto_start.unwrap_or(false) && !created_tasks.is_empty() {
        let task_ids = created_tasks
            .iter()
            .filter_map(|task| task.task_id.parse::<i64>().ok())
            .collect::<Vec<_>>();
        enqueue_tasks(&state, &app, &task_ids, &["PENDING"])
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|task_id| task_id.to_string())
            .collect()
    } else {
        Vec::new()
    };

    let created_tasks = created_tasks
        .into_iter()
        .map(|task| {
            task.task_id
                .parse::<i64>()
                .ok()
                .and_then(|task_id| state.load_task_response(task_id).ok())
                .unwrap_or(task)
        })
        .collect();

    Ok(ImportTasksResponse {
        created_tasks,
        failed_files,
        queued_task_ids,
    })
}
#[tauri::command]
pub(crate) fn list_tasks(
    request: ListTasksRequest,
    state: State<DesktopState>,
) -> CommandResult<PageResult<TaskResponse>> {
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    list_task_page(&mut conn, &request).map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) fn get_task(task_id: String, state: State<DesktopState>) -> CommandResult<TaskResponse> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    state
        .load_task_response(task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_task_status(
    task_id: String,
    state: State<DesktopState>,
) -> CommandResult<TaskStatusResponse> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    if let Some(status) = state.runtime.progress.read().get(&task_id).cloned() {
        return Ok(status);
    }

    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let task = load_task_response(&mut conn, task_id).map_err(|error| error.to_string())?;
    Ok(TaskStatusResponse {
        task_id: task.task_id,
        status: task.status,
        phase: None,
        progress: None,
        current_frame: None,
        total_frames: None,
        preprocessing_duration: None,
        analyzing_elapsed_time: None,
        is_timeout: Some(task.is_timeout),
        timeout_warning: Some(false),
        failure_reason: task.failure_reason,
        queue_position: task.queue_position,
    })
}

#[tauri::command]
pub(crate) fn get_task_result(
    task_id: String,
    state: State<DesktopState>,
) -> CommandResult<TaskResultResponse> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    load_task_result(&mut conn, task_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn start_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    enqueue_tasks(&state, &app, &[task_id], &["PENDING"]).map_err(|error| error.to_string())?;
    Ok("任务已加入分析队列".to_string())
}

#[tauri::command]
pub(crate) fn dequeue_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    if has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }

    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    dequeue_task_record(&mut conn, task_id).map_err(|error| error.to_string())?;

    state.runtime.progress.write().remove(&task_id);
    let response = TaskStatusResponse {
        task_id: task_id.to_string(),
        status: "PENDING".to_string(),
        phase: Some("待启动".to_string()),
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
    state.emit_status(&app, &response);
    state.emit_detail(&app, task_id);
    emit_queue_updates(&state, &app).map_err(|error| error.to_string())?;
    state.emit_scheduler_state(&app);
    Ok("任务已移出分析队列".to_string())
}

#[tauri::command]
pub(crate) fn reanalyze_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    if has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }
    let media_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let task_dir = task_root(&media_root, task_id);

    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let task = load_task_response(&mut conn, task_id).map_err(|error| error.to_string())?;
    if !matches!(
        task.status.as_str(),
        "COMPLETED" | "COMPLETED_TIMEOUT" | "FAILED"
    ) {
        return Err(format!("当前任务状态不允许重新分析: {}", task.status));
    }

    reset_task_for_reanalysis(&conn, task_id).map_err(|error| error.to_string())?;

    let _ = fs::remove_file(task_dir.join("output").join("result.mp4"));
    let _ = fs::remove_file(task_dir.join("output").join("preprocessed.mp4"));
    let _ = fs::remove_file(task_dir.join("output").join("tracking.json"));

    state.runtime.progress.write().remove(&task_id);
    enqueue_tasks(&state, &app, &[task_id], &["PENDING"]).map_err(|error| error.to_string())?;
    Ok("任务已重新进入分析队列".to_string())
}

#[tauri::command]
pub(crate) fn delete_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    if state.runtime.active_tasks.read().contains(&task_id) {
        return Err("正在执行中的任务不能删除".to_string());
    }

    let media_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let task_dir = task_root(&media_root, task_id);
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let task = load_task_response(&mut conn, task_id).map_err(|error| error.to_string())?;
    if matches!(task.status.as_str(), "PREPROCESSING" | "ANALYZING") {
        return Err("正在执行中的任务不能删除".to_string());
    }
    if task.status == "QUEUED" && has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }

    delete_task_record(&conn, task_id).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(task_dir);
    state.runtime.progress.write().remove(&task_id);
    if task.status == "QUEUED" {
        emit_queue_updates(&state, &app).map_err(|error| error.to_string())?;
        state.emit_scheduler_state(&app);
    }
    Ok("任务删除成功".to_string())
}

#[tauri::command]
pub(crate) fn delete_tasks(
    request: DeleteTasksRequest,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<DeleteTasksResponse> {
    let mut task_ids = Vec::new();
    for raw_task_id in request.task_ids {
        let task_id = raw_task_id
            .parse::<i64>()
            .map_err(|error| error.to_string())?;
        if !task_ids.contains(&task_id) {
            task_ids.push(task_id);
        }
    }

    if task_ids.is_empty() {
        return Err("请选择要删除的任务".to_string());
    }

    {
        let active_tasks = state.runtime.active_tasks.read();
        if task_ids
            .iter()
            .any(|task_id| active_tasks.contains(task_id))
        {
            return Err("选中的任务包含正在执行中的任务，不能批量删除".to_string());
        }
    }

    let media_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let mut loaded_tasks = Vec::with_capacity(task_ids.len());
    let mut task_dirs = Vec::with_capacity(task_ids.len());
    let mut deleted_queued = false;

    for task_id in &task_ids {
        let task = load_task_response(&mut conn, *task_id).map_err(|error| error.to_string())?;
        if matches!(task.status.as_str(), "PREPROCESSING" | "ANALYZING") {
            return Err(format!("任务“{}”正在执行中，不能批量删除", task.name));
        }
        if task.status == "QUEUED" {
            deleted_queued = true;
        }
        loaded_tasks.push(task);
        task_dirs.push(task_root(&media_root, *task_id));
    }

    if deleted_queued && has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }

    delete_task_records(&mut conn, &task_ids).map_err(|error| error.to_string())?;
    for task_dir in task_dirs {
        let _ = fs::remove_dir_all(task_dir);
    }
    {
        let mut progress = state.runtime.progress.write();
        for task_id in &task_ids {
            progress.remove(task_id);
        }
    }

    if deleted_queued {
        emit_queue_updates(&state, &app).map_err(|error| error.to_string())?;
        state.emit_scheduler_state(&app);
    }

    Ok(DeleteTasksResponse {
        deleted_task_ids: loaded_tasks.into_iter().map(|task| task.task_id).collect(),
    })
}

#[tauri::command]
pub(crate) fn get_video_stream_url(
    task_id: String,
    video_type: String,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    let media_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let task_dir = task_root(&media_root, task_id);
    let loaded = {
        let mut conn = state.open_db().map_err(|error| error.to_string())?;
        load_task_with_paths(&mut conn, task_id).map_err(|error| error.to_string())?
    };

    let relative = match video_type.as_str() {
        "original" => loaded.analysis_input_rel.unwrap_or(loaded.original_rel),
        "preprocessed" => loaded
            .response
            .preprocessed_video_path
            .clone()
            .ok_or_else(|| "预处理视频尚未生成".to_string())?,
        "result" => loaded
            .response
            .result_video_path
            .clone()
            .ok_or_else(|| "结果视频尚未生成".to_string())?,
        _ => return Err("不支持的视频类型".to_string()),
    };

    let path = task_dir.join(relative);
    if !path.exists() {
        return Err("视频文件不存在".to_string());
    }

    let token = state.register_media_token(path);
    Ok(format!(
        "http://127.0.0.1:{}/media/{}",
        state.runtime.media_server_port, token
    ))
}
