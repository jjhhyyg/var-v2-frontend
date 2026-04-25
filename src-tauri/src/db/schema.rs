use crate::*;

pub(crate) fn init_db(db_path: &Path, backup_dir: &Path) -> anyhow::Result<()> {
    if db_path.exists() {
        let backup_name = format!(
            "pre-migration-{}.sqlite3",
            Local::now().format("%Y%m%d-%H%M%S")
        );
        let backup_path = backup_dir.join(backup_name);
        fs::copy(db_path, backup_path)?;
    }

    let conn = open_db(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if user_version < 3 {
        conn.execute_batch(
            r#"
            BEGIN;
            DROP TABLE IF EXISTS tracking_objects;
            DROP TABLE IF EXISTS anomaly_events;
            DROP TABLE IF EXISTS dynamic_metrics;
            DROP TABLE IF EXISTS task_configs;
            DROP TABLE IF EXISTS analysis_tasks;
            COMMIT;
            "#,
        )?;
    }

    conn.execute_batch(
        r#"
        BEGIN;
        CREATE TABLE IF NOT EXISTS analysis_tasks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          original_filename TEXT,
          original_video_rel_path TEXT NOT NULL,
          analysis_input_rel_path TEXT,
          result_video_rel_path TEXT,
          preprocessed_video_rel_path TEXT,
          video_duration INTEGER NOT NULL,
          status TEXT NOT NULL,
          timeout_threshold INTEGER NOT NULL,
          is_timeout INTEGER NOT NULL DEFAULT 0,
          video_info_json TEXT,
          performance_json TEXT,
          global_analysis_json TEXT,
          created_at TEXT NOT NULL,
          started_at TEXT,
          preprocessing_completed_at TEXT,
          completed_at TEXT,
          failure_reason TEXT,
          queue_order INTEGER
        );
        CREATE TABLE IF NOT EXISTS task_configs (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id INTEGER NOT NULL UNIQUE,
          timeout_ratio TEXT NOT NULL,
          model_version TEXT,
          enable_preprocessing INTEGER NOT NULL,
          preprocessing_strength TEXT NOT NULL,
          preprocessing_enhance_pool INTEGER NOT NULL,
          enable_dynamic_metrics INTEGER NOT NULL DEFAULT 1,
          frame_rate REAL NOT NULL,
          FOREIGN KEY(task_id) REFERENCES analysis_tasks(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS dynamic_metrics (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id INTEGER NOT NULL,
          frame_number INTEGER NOT NULL,
          timestamp REAL NOT NULL,
          brightness REAL,
          pool_area INTEGER,
          pool_perimeter REAL,
          FOREIGN KEY(task_id) REFERENCES analysis_tasks(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS anomaly_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id INTEGER NOT NULL,
          event_type TEXT NOT NULL,
          start_frame INTEGER NOT NULL,
          end_frame INTEGER NOT NULL,
          start_time REAL,
          end_time REAL,
          metadata_json TEXT,
          FOREIGN KEY(task_id) REFERENCES analysis_tasks(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        PRAGMA user_version = 3;
        COMMIT;
        "#,
    )?;
    ensure_queue_schema(&conn)?;
    ensure_task_config_schema(&conn)?;
    repair_inconsistent_task_statuses(&conn)?;
    ensure_queue_orders_for_queued_tasks(&conn)?;
    Ok(())
}

pub(crate) fn open_db(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub(crate) fn repair_inconsistent_task_statuses(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE analysis_tasks
         SET status = 'FAILED',
             queue_order = NULL,
             completed_at = COALESCE(completed_at, ?1),
             failure_reason = COALESCE(failure_reason, '桌面端异常退出或重启中断')
         WHERE status IN ('PREPROCESSING', 'ANALYZING')",
        params![now_string()],
    )?;
    Ok(())
}

pub(crate) fn ensure_task_config_schema(conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(task_configs)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let has_enable_dynamic_metrics = columns
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column| column == "enable_dynamic_metrics");

    if !has_enable_dynamic_metrics {
        conn.execute(
            "ALTER TABLE task_configs ADD COLUMN enable_dynamic_metrics INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }

    Ok(())
}

pub(crate) fn ensure_queue_schema(conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(analysis_tasks)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let has_queue_order = columns
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column| column == "queue_order");

    if !has_queue_order {
        conn.execute(
            "ALTER TABLE analysis_tasks ADD COLUMN queue_order INTEGER",
            [],
        )?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_analysis_tasks_status_queue_order
         ON analysis_tasks(status, queue_order)",
        [],
    )?;
    Ok(())
}

pub(crate) fn ensure_queue_orders_for_queued_tasks(conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id
           FROM analysis_tasks
          WHERE status = 'QUEUED' AND queue_order IS NULL
          ORDER BY created_at ASC, id ASC",
    )?;
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    if ids.is_empty() {
        return Ok(());
    }

    let mut next_order = next_queue_order(conn)?;
    for task_id in ids {
        conn.execute(
            "UPDATE analysis_tasks SET queue_order = ?1 WHERE id = ?2",
            params![next_order, task_id],
        )?;
        next_order += 1;
    }

    Ok(())
}

pub(crate) fn next_queue_order(conn: &Connection) -> anyhow::Result<i64> {
    let max_order: Option<i64> =
        conn.query_row("SELECT MAX(queue_order) FROM analysis_tasks", [], |row| {
            row.get(0)
        })?;
    Ok(max_order.unwrap_or(0) + 1)
}
