use crate::*;

pub(crate) fn next_queued_task_id(conn: &Connection) -> anyhow::Result<Option<i64>> {
    conn.query_row(
        "SELECT id
           FROM analysis_tasks
          WHERE status = 'QUEUED'
          ORDER BY queue_order ASC, created_at ASC, id ASC
          LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}
pub(crate) fn load_queued_task_positions(conn: &Connection) -> anyhow::Result<Vec<(i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, queue_order
           FROM analysis_tasks
          WHERE status = 'QUEUED'
          ORDER BY queue_order ASC, created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
    })?;

    Ok(rows
        .enumerate()
        .filter_map(|(index, row)| match row {
            Ok((task_id, Some(_))) => Some(Ok((task_id, (index + 1) as i64))),
            Ok((_task_id, None)) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()?)
}

pub(crate) fn queue_position_for_task(
    conn: &Connection,
    task_id: i64,
) -> anyhow::Result<Option<i64>> {
    let queue_order: Option<i64> = conn
        .query_row(
            "SELECT queue_order FROM analysis_tasks WHERE id = ?1 AND status = 'QUEUED'",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    let Some(queue_order) = queue_order else {
        return Ok(None);
    };

    let position: i64 = conn.query_row(
        "SELECT COUNT(*)
           FROM analysis_tasks
          WHERE status = 'QUEUED'
            AND queue_order IS NOT NULL
            AND queue_order <= ?1",
        params![queue_order],
        |row| row.get(0),
    )?;
    Ok(Some(position))
}
pub(crate) fn load_task_with_paths(
    conn: &mut Connection,
    task_id: i64,
) -> anyhow::Result<LoadedTask> {
    let row = {
        let mut stmt = conn.prepare(
            "SELECT id, name, original_filename, original_video_rel_path, analysis_input_rel_path,
                    result_video_rel_path, preprocessed_video_rel_path, video_duration, status,
                    timeout_threshold, is_timeout, created_at, started_at, preprocessing_completed_at,
                    completed_at, failure_reason
               FROM analysis_tasks
              WHERE id = ?1",
        )?;

        stmt.query_row(params![task_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
            ))
        })?
    };

    let config = load_task_config(conn, task_id)?;
    Ok(LoadedTask {
        response: TaskResponse {
            task_id: row.0.to_string(),
            name: row.1,
            original_filename: row.2,
            video_duration: row.7,
            result_video_path: row.5,
            preprocessed_video_path: row.6,
            status: row.8,
            timeout_threshold: row.9,
            is_timeout: row.10 != 0,
            config,
            created_at: row.11,
            started_at: row.12,
            preprocessing_completed_at: row.13,
            completed_at: row.14,
            failure_reason: row.15,
            queue_position: None,
        },
        original_rel: row.3,
        analysis_input_rel: row.4,
    })
}

pub(crate) fn load_task_response(
    conn: &mut Connection,
    task_id: i64,
) -> anyhow::Result<TaskResponse> {
    let mut response = load_task_with_paths(conn, task_id)?.response;
    response.queue_position = queue_position_for_task(conn, task_id)?;
    Ok(response)
}

pub(crate) fn load_task_config(
    conn: &mut Connection,
    task_id: i64,
) -> anyhow::Result<Option<TaskConfigData>> {
    let mut stmt = conn.prepare(
        "SELECT timeout_ratio, model_version, enable_preprocessing, preprocessing_strength,
                preprocessing_enhance_pool, frame_rate
           FROM task_configs WHERE task_id = ?1",
    )?;

    let result = stmt
        .query_row(params![task_id], |row| {
            Ok(TaskConfigData {
                timeout_ratio: row.get(0)?,
                model_version: row.get(1)?,
                enable_preprocessing: row.get::<_, i64>(2)? != 0,
                preprocessing_strength: row.get(3)?,
                preprocessing_enhance_pool: row.get::<_, i64>(4)? != 0,
                frame_rate: row.get(5)?,
            })
        })
        .optional()?;

    Ok(result)
}

pub(crate) fn list_task_page(
    conn: &mut Connection,
    request: &ListTasksRequest,
) -> anyhow::Result<PageResult<TaskResponse>> {
    let page = request.page.unwrap_or(0).max(0);
    let size = request.size.unwrap_or(20).clamp(1, 100);
    let offset = page * size;
    let order_clause = build_task_list_order_clause(
        request.status.as_deref(),
        request.sort_by.as_deref(),
        request.sort_direction.as_deref(),
    );

    let (total, ids) = if let Some(status) = request.status.clone() {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM analysis_tasks WHERE status = ?1",
            params![status.clone()],
            |row| row.get(0),
        )?;
        let query_sql = format!(
            "SELECT id
               FROM analysis_tasks
              WHERE status = ?1
              ORDER BY {order_clause}
              LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&query_sql)?;
        let rows = stmt.query_map(params![status, size, offset], |row| row.get::<_, i64>(0))?;
        let ids = rows.collect::<Result<Vec<_>, _>>()?;
        (total, ids)
    } else {
        let total: i64 =
            conn.query_row("SELECT COUNT(*) FROM analysis_tasks", [], |row| row.get(0))?;
        let query_sql = format!(
            "SELECT id
               FROM analysis_tasks
              ORDER BY {order_clause}
              LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&query_sql)?;
        let rows = stmt.query_map(params![size, offset], |row| row.get::<_, i64>(0))?;
        let ids = rows.collect::<Result<Vec<_>, _>>()?;
        (total, ids)
    };

    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        items.push(load_task_response(conn, id)?);
    }

    let total_pages = if total == 0 {
        0
    } else {
        ((total as f64) / (size as f64)).ceil() as i64
    };
    Ok(PageResult {
        items,
        total,
        total_pages,
        page,
        page_size: size,
        has_next: total_pages > 0 && page < total_pages - 1,
        has_previous: page > 0,
    })
}

pub(crate) fn dequeue_task_record(
    conn: &mut Connection,
    task_id: i64,
) -> anyhow::Result<TaskResponse> {
    let task = load_task_response(conn, task_id)?;
    if task.status != "QUEUED" {
        return Err(anyhow!("当前任务状态不允许移出队列: {}", task.status));
    }

    let updated = conn.execute(
        "UPDATE analysis_tasks
         SET status = 'PENDING',
             queue_order = NULL
         WHERE id = ?1 AND status = 'QUEUED'",
        params![task_id],
    )?;
    if updated == 0 {
        return Err(anyhow!("任务状态已变化，无法移出队列"));
    }

    Ok(task)
}

pub(crate) fn cancel_recovery_tasks(
    conn: &Connection,
    recovery_tasks: &[QueueRecoveryTask],
) -> anyhow::Result<()> {
    for task in recovery_tasks {
        let task_id = task.task_id.parse::<i64>()?;
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

pub(crate) fn delete_task_record(conn: &Connection, task_id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM analysis_tasks WHERE id = ?1", params![task_id])?;
    Ok(())
}

pub(crate) fn delete_task_records(conn: &mut Connection, task_ids: &[i64]) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    for task_id in task_ids {
        tx.execute("DELETE FROM analysis_tasks WHERE id = ?1", params![task_id])?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn reset_task_for_reanalysis(conn: &Connection, task_id: i64) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM dynamic_metrics WHERE task_id = ?1",
        params![task_id],
    )?;
    conn.execute(
        "DELETE FROM anomaly_events WHERE task_id = ?1",
        params![task_id],
    )?;
    conn.execute(
        "UPDATE analysis_tasks
         SET status = 'PENDING',
             is_timeout = 0,
             video_info_json = NULL,
             performance_json = NULL,
             global_analysis_json = NULL,
             result_video_rel_path = NULL,
             preprocessed_video_rel_path = NULL,
             queue_order = NULL,
             started_at = NULL,
             preprocessing_completed_at = NULL,
             completed_at = NULL,
             failure_reason = NULL
         WHERE id = ?1",
        params![task_id],
    )?;
    Ok(())
}

pub(crate) fn build_task_list_order_clause(
    status_filter: Option<&str>,
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
) -> &'static str {
    match (sort_by, sort_direction) {
        (Some("createdAt"), Some("asc")) => "created_at ASC, id ASC",
        (Some("createdAt"), _) => "created_at DESC, id DESC",
        (Some("status"), Some("desc")) => {
            "CASE status
               WHEN 'PENDING' THEN 1
               WHEN 'QUEUED' THEN 2
               WHEN 'PREPROCESSING' THEN 3
               WHEN 'ANALYZING' THEN 4
               WHEN 'COMPLETED' THEN 5
               WHEN 'COMPLETED_TIMEOUT' THEN 6
               WHEN 'FAILED' THEN 7
               ELSE 999
             END DESC,
             created_at DESC,
             id DESC"
        }
        (Some("status"), _) => {
            "CASE status
               WHEN 'PENDING' THEN 1
               WHEN 'QUEUED' THEN 2
               WHEN 'PREPROCESSING' THEN 3
               WHEN 'ANALYZING' THEN 4
               WHEN 'COMPLETED' THEN 5
               WHEN 'COMPLETED_TIMEOUT' THEN 6
               WHEN 'FAILED' THEN 7
               ELSE 999
             END ASC,
             created_at DESC,
             id DESC"
        }
        (Some("completedAt"), Some("asc")) => {
            "completed_at IS NULL ASC,
             completed_at ASC,
             created_at DESC,
             id DESC"
        }
        (Some("completedAt"), _) => {
            "completed_at IS NULL ASC,
             completed_at DESC,
             created_at DESC,
             id DESC"
        }
        _ if status_filter == Some("QUEUED") => "queue_order ASC, created_at ASC, id ASC",
        _ => "created_at DESC, id DESC",
    }
}
