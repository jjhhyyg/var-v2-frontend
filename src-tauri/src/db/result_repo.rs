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
        "DELETE FROM tracking_objects WHERE task_id = ?1",
        params![task_id],
    )?;

    tx.execute(
        "UPDATE analysis_tasks
         SET status = ?1,
             is_timeout = ?2,
             completed_at = ?3,
             failure_reason = ?4,
             global_analysis_json = ?5,
             tracking_rel_path = COALESCE(tracking_rel_path, ?6),
             queue_order = NULL
         WHERE id = ?7",
        params![
            payload.status,
            payload.is_timeout.unwrap_or(false) as i64,
            now_string(),
            payload.failure_reason.clone(),
            payload
                .global_analysis
                .as_ref()
                .map(|value| value.to_string()),
            "output/tracking.json",
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
                "INSERT INTO anomaly_events (task_id, event_type, start_frame, end_frame, object_id, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    task_id,
                    event.event_type,
                    event.start_frame,
                    event.end_frame,
                    event.object_id,
                    event.metadata.as_ref().map(|value| value.to_string())
                ],
            )?;
        }
    }

    if let Some(objects) = &payload.tracking_objects {
        for object in objects {
            tx.execute(
                "INSERT INTO tracking_objects (task_id, object_id, category, first_frame, last_frame, trajectory_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    task_id,
                    object.object_id,
                    object.category,
                    object.first_frame,
                    object.last_frame,
                    object.trajectory.as_ref().map(|value| value.to_string())
                ],
            )?;
        }
    }

    tx.commit()?;
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
            "SELECT id, event_type, start_frame, end_frame, object_id, metadata_json
               FROM anomaly_events WHERE task_id = ?1 ORDER BY start_frame ASC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            let metadata_raw: Option<String> = row.get(5)?;
            Ok(AnomalyEventData {
                event_id: row.get::<_, i64>(0)?.to_string(),
                event_type: row.get(1)?,
                start_frame: row.get(2)?,
                end_frame: row.get(3)?,
                object_id: row.get(4)?,
                metadata: metadata_raw
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok()),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let tracking_objects = {
        let mut stmt = conn.prepare(
            "SELECT id, object_id, category, first_frame, last_frame, trajectory_json
               FROM tracking_objects WHERE task_id = ?1 ORDER BY first_frame ASC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            let trajectory_raw: Option<String> = row.get(5)?;
            Ok(TrackingObjectData {
                tracking_id: row.get::<_, i64>(0)?.to_string(),
                object_id: row.get(1)?,
                category: row.get(2)?,
                first_frame: row.get(3)?,
                last_frame: row.get(4)?,
                trajectory: trajectory_raw
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
    let object_statistics = tracking_objects
        .iter()
        .fold(HashMap::new(), |mut acc, item| {
            *acc.entry(item.category.clone()).or_insert(0) += 1;
            acc
        });

    let global_analysis = {
        let raw: Option<String> = conn.query_row(
            "SELECT global_analysis_json FROM analysis_tasks WHERE id = ?1",
            params![task_id],
            |row| row.get(0),
        )?;
        raw.and_then(|value| serde_json::from_str(&value).ok())
    };

    Ok(TaskResultResponse {
        task_id: task.task_id,
        name: task.name,
        status: task.status,
        is_timeout: task.is_timeout,
        dynamic_metrics,
        global_analysis,
        anomaly_events,
        tracking_objects,
        event_statistics,
        object_statistics,
    })
}
