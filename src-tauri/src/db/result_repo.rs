use crate::*;

pub(crate) fn persist_result_payload(
    state: &DesktopState,
    task_id: i64,
    payload: &ResultPayload,
) -> anyhow::Result<()> {
    let mut conn = state.open_db()?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM dynamic_metrics WHERE task_id = ?1",
        params![task_id],
    )?;
    tx.execute(
        "DELETE FROM anomaly_events WHERE task_id = ?1",
        params![task_id],
    )?;

    tx.execute(
        "UPDATE analysis_tasks
         SET status = ?1,
             is_timeout = ?2,
             completed_at = ?3,
             failure_reason = ?4,
             video_info_json = ?5,
             performance_json = ?6,
             global_analysis_json = ?7,
             queue_order = NULL
         WHERE id = ?8",
        params![
            payload.status,
            payload.is_timeout.unwrap_or(false) as i64,
            now_string(),
            payload.failure_reason.clone(),
            payload
                .video_info
                .as_ref()
                .map(|value| serde_json::to_string(value))
                .transpose()?,
            payload
                .performance
                .as_ref()
                .map(|value| serde_json::to_string(value))
                .transpose()?,
            payload
                .global_analysis
                .as_ref()
                .map(|value| value.to_string()),
            task_id
        ],
    )?;

    if let Some(metrics) = &payload.dynamic_metrics {
        for metric in metrics {
            tx.execute(
                "INSERT INTO dynamic_metrics (task_id, frame_number, timestamp, brightness, pool_area, pool_perimeter)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    task_id,
                    metric.frame_number,
                    metric.timestamp,
                    metric.brightness,
                    metric.pool_area,
                    metric.pool_perimeter
                ],
            )?;
        }
    }

    if let Some(events) = &payload.anomaly_events {
        for event in events {
            tx.execute(
                "INSERT INTO anomaly_events (task_id, event_type, start_frame, end_frame, start_time, end_time, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    task_id,
                    event.event_type,
                    event.start_frame,
                    event.end_frame,
                    event.start_time,
                    event.end_time,
                    event.metadata.as_ref().map(|value| value.to_string())
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub(crate) fn update_task_timing_summary(
    state: &DesktopState,
    task_id: i64,
    timing_summary: &TimingSummaryData,
) -> anyhow::Result<()> {
    let conn = state.open_db()?;
    let performance_raw: Option<String> = conn
        .query_row(
            "SELECT performance_json FROM analysis_tasks WHERE id = ?1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    let mut performance_value = performance_raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| json!({}));

    if !performance_value.is_object() {
        performance_value = json!({});
    }

    if let Value::Object(object) = &mut performance_value {
        object.insert(
            "timingSummary".to_string(),
            serde_json::to_value(timing_summary)?,
        );
    }

    conn.execute(
        "UPDATE analysis_tasks SET performance_json = ?1 WHERE id = ?2",
        params![performance_value.to_string(), task_id],
    )?;

    Ok(())
}

pub(crate) fn load_task_result(
    conn: &mut Connection,
    task_id: i64,
) -> anyhow::Result<TaskResultResponse> {
    let task = load_task_response(conn, task_id)?;
    if !matches!(task.status.as_str(), "COMPLETED" | "COMPLETED_TIMEOUT") {
        return Err(anyhow!("任务尚未完成，无法获取结果"));
    }

    let dynamic_metrics = {
        let mut stmt = conn.prepare(
            "SELECT frame_number, timestamp, brightness, pool_area, pool_perimeter
               FROM dynamic_metrics WHERE task_id = ?1 ORDER BY frame_number ASC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(DynamicMetricData {
                frame_number: row.get(0)?,
                timestamp: row.get(1)?,
                brightness: row.get(2)?,
                pool_area: row.get(3)?,
                pool_perimeter: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let anomaly_events = {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, start_frame, end_frame, start_time, end_time, metadata_json
               FROM anomaly_events WHERE task_id = ?1 ORDER BY start_frame ASC, event_type ASC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            let metadata_raw: Option<String> = row.get(6)?;
            Ok(AnomalyEventData {
                event_id: row.get::<_, i64>(0)?.to_string(),
                event_type: row.get(1)?,
                start_frame: row.get(2)?,
                end_frame: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                metadata: metadata_raw
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok()),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let event_statistics = anomaly_events.iter().fold(HashMap::new(), |mut acc, item| {
        *acc.entry(item.event_type.clone()).or_insert(0) += 1;
        acc
    });

    let (video_info, performance, global_analysis) = {
        let row = conn.query_row(
            "SELECT video_info_json, performance_json, global_analysis_json
               FROM analysis_tasks WHERE id = ?1",
            params![task_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )?;
        (
            row.0
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok())
                .unwrap_or_default(),
            row.1
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok())
                .unwrap_or_default(),
            row.2
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok()),
        )
    };

    Ok(TaskResultResponse {
        task_id: task.task_id,
        name: task.name,
        status: task.status,
        is_timeout: task.is_timeout,
        video_info,
        performance,
        dynamic_metrics,
        global_analysis,
        anomaly_events,
        event_statistics,
    })
}
