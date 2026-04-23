use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State as AxumState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use base64::Engine;
use chrono::Local;
use parking_lot::{Mutex, RwLock};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager, State};
use tower::ServiceExt;
use tower_http::services::ServeFile;
use uuid::Uuid;
use walkdir::WalkDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const LIBRARY_MARKER_FILENAME: &str = "library.json";
const MEDIA_EVENT_TTL_MINUTES: u64 = 10;

type CommandResult<T> = Result<T, String>;

#[derive(Clone)]
struct DesktopState {
    paths: ControlPaths,
    runtime: RuntimeState,
}

#[derive(Clone)]
struct ControlPaths {
    app_config_dir: PathBuf,
    app_local_data_dir: PathBuf,
    settings_path: PathBuf,
    db_path: PathBuf,
    db_backup_dir: PathBuf,
    log_dir: PathBuf,
    runtime_cache_dir: PathBuf,
    resource_dir: PathBuf,
}

#[derive(Clone)]
struct RuntimeState {
    settings: Arc<RwLock<AppSettings>>,
    progress: Arc<RwLock<HashMap<i64, TaskStatusResponse>>>,
    active_tasks: Arc<RwLock<HashSet<i64>>>,
    pending_queue_recovery: Arc<RwLock<Option<Vec<QueueRecoveryTask>>>>,
    scheduler_lock: Arc<Mutex<()>>,
    resource_probe: Arc<Mutex<ResourceProbe>>,
    media_tokens: Arc<RwLock<HashMap<String, MediaToken>>>,
    media_server_port: u16,
}

struct ResourceProbe {
    system: System,
}

#[derive(Clone)]
struct MediaToken {
    path: PathBuf,
    expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    media_library_path: Option<String>,
    cleanup_preprocessed: bool,
    recent_library_migrations: Vec<MigrationRecord>,
    scheduler: SchedulerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchedulerSettings {
    max_concurrency: usize,
    mac_cpu_limit_percent: f64,
    mac_min_available_memory_ratio: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            media_library_path: None,
            cleanup_preprocessed: true,
            recent_library_migrations: Vec::new(),
            scheduler: SchedulerSettings::default(),
        }
    }
}

impl Default for SchedulerSettings {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            mac_cpu_limit_percent: 85.0,
            mac_min_available_memory_ratio: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationRecord {
    from: String,
    to: String,
    migrated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStateResponse {
    initialized: bool,
    media_library_available: bool,
    media_library_path: Option<String>,
    recommended_media_library_path: String,
    max_concurrency: usize,
    mac_cpu_limit_percent: f64,
    mac_min_available_memory_ratio: f64,
    active_task_count: usize,
    queued_task_count: usize,
    platform: String,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulerStateResponse {
    max_concurrency: usize,
    active_task_count: usize,
    queued_task_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueRecoveryTask {
    task_id: String,
    name: String,
    queue_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QueueRecoveryStateResponse {
    has_pending_recovery: bool,
    tasks: Vec<QueueRecoveryTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskConfigInput {
    timeout_ratio: Option<String>,
    enable_preprocessing: Option<bool>,
    preprocessing_strength: Option<String>,
    preprocessing_enhance_pool: Option<bool>,
    enable_tracking_merge: Option<bool>,
    tracking_merge_strategy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportTaskRequest {
    file_path: String,
    name: Option<String>,
    config: Option<TaskConfigInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportTaskItemRequest {
    file_path: String,
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportTasksRequest {
    items: Vec<ImportTaskItemRequest>,
    config: Option<TaskConfigInput>,
    auto_start: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportTasksFailure {
    file_path: String,
    file_name: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportTasksResponse {
    created_tasks: Vec<TaskResponse>,
    failed_files: Vec<ImportTasksFailure>,
    queued_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchedulerSettingsInput {
    max_concurrency: Option<usize>,
    mac_cpu_limit_percent: Option<f64>,
    mac_min_available_memory_ratio: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveQueueRecoveryRequest {
    continue_analysis: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListTasksRequest {
    page: Option<i64>,
    size: Option<i64>,
    status: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportReportRequest {
    path: String,
    text_content: Option<String>,
    base64_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerJob {
    task_id: i64,
    video_path: String,
    video_duration: i64,
    timeout_threshold: i64,
    model_path: String,
    device: String,
    log_path: String,
    preprocessed_output_path: String,
    result_output_path: String,
    config: WorkerJobConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerJobConfig {
    confidence_threshold: f64,
    iou_threshold: f64,
    timeout_ratio: String,
    frame_rate: f64,
    enable_preprocessing: bool,
    preprocessing_strength: String,
    preprocessing_enhance_pool: bool,
    enable_tracking_merge: bool,
    tracking_merge_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerEvent {
    #[serde(rename = "type")]
    event_type: String,
    payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskResponse {
    task_id: String,
    name: String,
    original_filename: Option<String>,
    video_duration: i64,
    result_video_path: Option<String>,
    preprocessed_video_path: Option<String>,
    status: String,
    timeout_threshold: i64,
    is_timeout: bool,
    config: Option<TaskConfigData>,
    created_at: String,
    started_at: Option<String>,
    preprocessing_completed_at: Option<String>,
    completed_at: Option<String>,
    failure_reason: Option<String>,
    queue_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskConfigData {
    timeout_ratio: String,
    model_version: Option<String>,
    enable_preprocessing: bool,
    preprocessing_strength: String,
    preprocessing_enhance_pool: bool,
    enable_tracking_merge: bool,
    tracking_merge_strategy: String,
    frame_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskStatusResponse {
    task_id: String,
    status: String,
    phase: Option<String>,
    progress: Option<f64>,
    current_frame: Option<i64>,
    total_frames: Option<i64>,
    preprocessing_duration: Option<i64>,
    analyzing_elapsed_time: Option<i64>,
    is_timeout: Option<bool>,
    timeout_warning: Option<bool>,
    failure_reason: Option<String>,
    queue_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskResultResponse {
    task_id: String,
    name: String,
    status: String,
    is_timeout: bool,
    dynamic_metrics: Vec<DynamicMetricData>,
    global_analysis: Option<Value>,
    anomaly_events: Vec<AnomalyEventData>,
    tracking_objects: Vec<TrackingObjectData>,
    event_statistics: HashMap<String, i64>,
    object_statistics: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DynamicMetricData {
    frame_number: i64,
    timestamp: f64,
    brightness: Option<f64>,
    pool_area: Option<i64>,
    pool_perimeter: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnomalyEventData {
    event_id: String,
    event_type: String,
    start_frame: i64,
    end_frame: i64,
    object_id: Option<i64>,
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackingObjectData {
    tracking_id: String,
    object_id: i64,
    category: String,
    first_frame: i64,
    last_frame: i64,
    trajectory: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PageResult<T> {
    items: Vec<T>,
    total: i64,
    total_pages: i64,
    page: i64,
    page_size: i64,
    has_next: bool,
    has_previous: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResultPayload {
    status: String,
    is_timeout: Option<bool>,
    failure_reason: Option<String>,
    dynamic_metrics: Option<Vec<DynamicMetricPayload>>,
    global_analysis: Option<Value>,
    anomaly_events: Option<Vec<AnomalyEventPayload>>,
    tracking_objects: Option<Vec<TrackingObjectPayload>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DynamicMetricPayload {
    frame_number: i64,
    timestamp: f64,
    brightness: Option<f64>,
    pool_area: Option<i64>,
    pool_perimeter: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnomalyEventPayload {
    event_type: String,
    start_frame: i64,
    end_frame: i64,
    object_id: Option<i64>,
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackingObjectPayload {
    object_id: i64,
    category: String,
    first_frame: i64,
    last_frame: i64,
    trajectory: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    status: String,
    phase: Option<String>,
    progress: Option<f64>,
    current_frame: Option<i64>,
    total_frames: Option<i64>,
    preprocessing_duration: Option<i64>,
    analyzing_elapsed_time: Option<i64>,
    is_timeout: Option<bool>,
    timeout_warning: Option<bool>,
    failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct VideoInfo {
    duration_seconds: i64,
    frame_rate: f64,
    codec_name: Option<String>,
}

#[derive(Debug)]
struct LoadedTask {
    response: TaskResponse,
    original_rel: String,
    analysis_input_rel: Option<String>,
}

impl DesktopState {
    fn bootstrap(app: &AppHandle) -> anyhow::Result<Self> {
        let paths = build_control_paths(app)?;
        ensure_control_dirs(&paths)?;
        let settings = load_settings(&paths.settings_path)?;
        init_db(&paths.db_path, &paths.db_backup_dir)?;

        let media_tokens = Arc::new(RwLock::new(HashMap::new()));
        let media_server_port = start_media_server(media_tokens.clone())?;

        let state = Self {
            paths: paths.clone(),
            runtime: RuntimeState {
                settings: Arc::new(RwLock::new(settings)),
                progress: Arc::new(RwLock::new(HashMap::new())),
                active_tasks: Arc::new(RwLock::new(HashSet::new())),
                pending_queue_recovery: Arc::new(RwLock::new(None)),
                scheduler_lock: Arc::new(Mutex::new(())),
                resource_probe: Arc::new(Mutex::new(ResourceProbe::new())),
                media_tokens,
                media_server_port,
            },
        };

        initialize_runtime_task_state(&state)?;
        state.emit_scheduler_state(app);
        spawn_scheduler_tick_loop(state.clone(), app.clone());
        Ok(state)
    }

    fn current_media_root(&self) -> anyhow::Result<PathBuf> {
        let settings = self.runtime.settings.read();
        let raw = settings
            .media_library_path
            .clone()
            .ok_or_else(|| anyhow!("媒体库尚未初始化"))?;
        Ok(PathBuf::from(raw))
    }

    fn recommended_media_library_path(&self) -> PathBuf {
        recommended_media_library_path()
    }

    fn media_library_available(&self) -> bool {
        self.current_media_root()
            .ok()
            .map(|root| validate_library_root(&root).is_ok())
            .unwrap_or(false)
    }

    fn open_db(&self) -> anyhow::Result<Connection> {
        open_db(&self.paths.db_path)
    }

    fn save_settings(&self) -> anyhow::Result<()> {
        save_settings(&self.paths.settings_path, &self.runtime.settings.read())
    }

    fn emit_status(&self, app: &AppHandle, status: &TaskStatusResponse) {
        let _ = app.emit("task-status", status.clone());
        let progress = status.progress.unwrap_or(0.0);
        let _ = app.emit(
            "task-list-update",
            json!({
                "taskId": status.task_id,
                "status": status.status,
                "progress": progress,
                "queuePosition": status.queue_position
            }),
        );
    }

    fn emit_detail(&self, app: &AppHandle, task_id: i64) {
        if let Ok(task) = self.load_task_response(task_id) {
            let _ = app.emit("task-detail-update", task);
        }
    }

    fn emit_scheduler_state(&self, app: &AppHandle) {
        if let Ok(payload) = self.scheduler_state_response() {
            let _ = app.emit("scheduler-state-update", payload);
        }
    }

    fn register_media_token(&self, path: PathBuf) -> String {
        self.prune_media_tokens();
        let token = Uuid::new_v4().to_string();
        self.runtime.media_tokens.write().insert(
            token.clone(),
            MediaToken {
                path,
                expires_at: Instant::now() + Duration::from_secs(MEDIA_EVENT_TTL_MINUTES * 60),
            },
        );
        token
    }

    fn prune_media_tokens(&self) {
        let now = Instant::now();
        self.runtime
            .media_tokens
            .write()
            .retain(|_, value| value.expires_at > now);
    }

    fn set_media_root(&self, root: &Path) -> anyhow::Result<()> {
        {
            let mut settings = self.runtime.settings.write();
            settings.media_library_path = Some(root.to_string_lossy().to_string());
        }
        self.save_settings()
    }

    fn queued_task_count(&self) -> anyhow::Result<usize> {
        let conn = self.open_db()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM analysis_tasks WHERE status = 'QUEUED'",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    fn scheduler_state_response(&self) -> anyhow::Result<SchedulerStateResponse> {
        let settings = self.runtime.settings.read();
        Ok(SchedulerStateResponse {
            max_concurrency: settings.scheduler.max_concurrency,
            active_task_count: self.runtime.active_tasks.read().len(),
            queued_task_count: self.queued_task_count()?,
        })
    }

    fn load_task_response(&self, task_id: i64) -> anyhow::Result<TaskResponse> {
        let mut conn = self.open_db()?;
        load_task_response(&mut conn, task_id)
    }
}

impl ResourceProbe {
    fn new() -> Self {
        let mut system = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        system.refresh_memory();
        Self { system }
    }

    fn snapshot(&mut self) -> ResourceSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        ResourceSnapshot {
            cpu_percent: self.system.global_cpu_usage() as f64,
            available_memory_bytes: self.system.available_memory(),
            total_memory_bytes: self.system.total_memory(),
            gpu_percent: None,
            gpu_memory_used_bytes: None,
            gpu_memory_total_bytes: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ResourceSnapshot {
    cpu_percent: f64,
    available_memory_bytes: u64,
    total_memory_bytes: u64,
    gpu_percent: Option<f64>,
    gpu_memory_used_bytes: Option<u64>,
    gpu_memory_total_bytes: Option<u64>,
}

fn build_control_paths(app: &AppHandle) -> anyhow::Result<ControlPaths> {
    let path = app.path();
    let app_config_dir = path.app_config_dir().context("无法解析 AppConfig 目录")?;
    let app_local_data_dir = path
        .app_local_data_dir()
        .context("无法解析 AppLocalData 目录")?;
    let resource_dir = path.resource_dir().context("无法解析资源目录")?;

    let settings_path = app_config_dir.join("settings.json");
    let db_dir = app_local_data_dir.join("db");
    let db_path = db_dir.join("app.sqlite3");
    let db_backup_dir = app_local_data_dir.join("backups").join("db");
    let log_dir = app_local_data_dir.join("logs");
    let runtime_cache_dir = app_local_data_dir
        .join("runtime")
        .join(APP_VERSION)
        .join(runtime_build_id());

    Ok(ControlPaths {
        app_config_dir,
        app_local_data_dir,
        settings_path,
        db_path,
        db_backup_dir,
        log_dir,
        runtime_cache_dir,
        resource_dir,
    })
}

fn runtime_build_id() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|timestamp| timestamp.as_secs().to_string())
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|timestamp| timestamp.as_secs().to_string())
        })
        .unwrap_or_else(|| "runtime".to_string())
}

fn ensure_control_dirs(paths: &ControlPaths) -> anyhow::Result<()> {
    fs::create_dir_all(&paths.app_config_dir)?;
    fs::create_dir_all(&paths.app_local_data_dir)?;
    fs::create_dir_all(paths.db_path.parent().context("db 目录缺失")?)?;
    fs::create_dir_all(&paths.db_backup_dir)?;
    fs::create_dir_all(&paths.log_dir)?;
    fs::create_dir_all(&paths.runtime_cache_dir)?;
    Ok(())
}

fn load_settings(settings_path: &Path) -> anyhow::Result<AppSettings> {
    if !settings_path.exists() {
        let settings = AppSettings::default();
        save_settings(settings_path, &settings)?;
        return Ok(settings);
    }

    let raw = fs::read_to_string(settings_path)?;
    let settings = serde_json::from_str(&raw).unwrap_or_default();
    Ok(settings)
}

fn save_settings(settings_path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(settings_path, raw)?;
    Ok(())
}

fn init_db(db_path: &Path, backup_dir: &Path) -> anyhow::Result<()> {
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
          tracking_rel_path TEXT,
          video_duration INTEGER NOT NULL,
          status TEXT NOT NULL,
          timeout_threshold INTEGER NOT NULL,
          is_timeout INTEGER NOT NULL DEFAULT 0,
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
          enable_tracking_merge INTEGER NOT NULL,
          tracking_merge_strategy TEXT NOT NULL,
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
          object_id INTEGER,
          metadata_json TEXT,
          FOREIGN KEY(task_id) REFERENCES analysis_tasks(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS tracking_objects (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id INTEGER NOT NULL,
          object_id INTEGER NOT NULL,
          category TEXT NOT NULL,
          first_frame INTEGER NOT NULL,
          last_frame INTEGER NOT NULL,
          trajectory_json TEXT,
          FOREIGN KEY(task_id) REFERENCES analysis_tasks(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        PRAGMA user_version = 2;
        COMMIT;
        "#,
    )?;
    ensure_queue_schema(&conn)?;
    repair_inconsistent_task_statuses(&conn)?;
    ensure_queue_orders_for_queued_tasks(&conn)?;
    Ok(())
}

fn open_db(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

fn repair_inconsistent_task_statuses(conn: &Connection) -> anyhow::Result<()> {
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

fn ensure_queue_schema(conn: &Connection) -> anyhow::Result<()> {
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

fn ensure_queue_orders_for_queued_tasks(conn: &Connection) -> anyhow::Result<()> {
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

fn next_queue_order(conn: &Connection) -> anyhow::Result<i64> {
    let max_order: Option<i64> =
        conn.query_row("SELECT MAX(queue_order) FROM analysis_tasks", [], |row| {
            row.get(0)
        })?;
    Ok(max_order.unwrap_or(0) + 1)
}

fn recommended_media_library_path() -> PathBuf {
    match std::env::consts::OS {
        "macos" => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Movies")
            .join("VAR Desktop Library"),
        "windows" => {
            for drive in ['D', 'E', 'F', 'G'] {
                let path = PathBuf::from(format!("{drive}:\\"));
                if path.exists() {
                    return path.join("VARDesktopData");
                }
            }
            PathBuf::from("C:\\VARDesktopData")
        }
        _ => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("VARDesktopData"),
    }
}

fn library_marker_path(root: &Path) -> PathBuf {
    root.join(LIBRARY_MARKER_FILENAME)
}

fn validate_library_root(root: &Path) -> anyhow::Result<()> {
    let marker_path = library_marker_path(root);
    if !marker_path.exists() {
        return Err(anyhow!("媒体库缺少 {}", LIBRARY_MARKER_FILENAME));
    }

    let raw = fs::read_to_string(marker_path)?;
    let marker: Value = serde_json::from_str(&raw)?;
    let identifier = marker
        .get("identifier")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("媒体库标记缺少 identifier"))?;
    if identifier != "cn.edu.ustb.hyy.var-desktop" {
        return Err(anyhow!("媒体库 identifier 不匹配"));
    }
    Ok(())
}

fn ensure_library_structure(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root.join("tasks"))?;
    let marker = json!({
        "identifier": "cn.edu.ustb.hyy.var-desktop",
        "version": APP_VERSION,
        "createdAt": now_string()
    });
    fs::write(
        library_marker_path(root),
        serde_json::to_string_pretty(&marker)?,
    )?;
    Ok(())
}

fn task_root(media_root: &Path, task_id: i64) -> PathBuf {
    media_root.join("tasks").join(task_id.to_string())
}

fn ensure_task_dirs(task_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(task_dir.join("input"))?;
    fs::create_dir_all(task_dir.join("work").join("tmp"))?;
    fs::create_dir_all(task_dir.join("output"))?;
    fs::create_dir_all(task_dir.join("logs"))?;
    Ok(())
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_timeout_ratio(timeout_ratio: &str, video_duration: i64) -> anyhow::Result<i64> {
    let mut parts = timeout_ratio.split(':');
    let numerator: i64 = parts
        .next()
        .ok_or_else(|| anyhow!("无效超时比例"))?
        .parse()?;
    let denominator: i64 = parts
        .next()
        .ok_or_else(|| anyhow!("无效超时比例"))?
        .parse()?;
    if numerator <= 0 || denominator <= 0 {
        return Err(anyhow!("超时比例必须为正整数"));
    }
    Ok(video_duration * denominator / numerator)
}

fn run_ffprobe(ffprobe_path: &Path, file_path: &Path) -> anyhow::Result<VideoInfo> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=avg_frame_rate,r_frame_rate,codec_name",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(file_path)
        .output()
        .context("执行 ffprobe 失败")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let value: Value = serde_json::from_slice(&output.stdout)?;
    let duration = value
        .get("format")
        .and_then(|v| v.get("duration"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        .round() as i64;
    let stream = value
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .ok_or_else(|| anyhow!("ffprobe 未返回视频流"))?;
    let frame_rate = stream
        .get("avg_frame_rate")
        .and_then(Value::as_str)
        .or_else(|| stream.get("r_frame_rate").and_then(Value::as_str))
        .map(parse_ratio)
        .transpose()?
        .unwrap_or(25.0);
    let codec_name = stream
        .get("codec_name")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(VideoInfo {
        duration_seconds: duration,
        frame_rate,
        codec_name,
    })
}

fn parse_ratio(raw: &str) -> anyhow::Result<f64> {
    if let Some((left, right)) = raw.split_once('/') {
        let numerator: f64 = left.parse()?;
        let denominator: f64 = right.parse()?;
        if denominator == 0.0 {
            return Ok(25.0);
        }
        Ok(numerator / denominator)
    } else {
        Ok(raw.parse()?)
    }
}

fn is_web_friendly_codec(path: &Path, codec_name: Option<&str>) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let codec = codec_name.unwrap_or_default().to_ascii_lowercase();
    matches!(extension.as_str(), "mp4" | "mov")
        && (codec.contains("h264")
            || codec.contains("264")
            || codec.contains("avc")
            || codec.contains("hevc")
            || codec.contains("h265"))
}

fn normalize_analysis_input(
    ffmpeg_path: &Path,
    source: &Path,
    target: &Path,
) -> anyhow::Result<()> {
    let status = Command::new(ffmpeg_path)
        .args([
            "-i",
            source.to_string_lossy().as_ref(),
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            "-y",
            target.to_string_lossy().as_ref(),
        ])
        .status()
        .context("执行 ffmpeg 标准化失败")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg 标准化视频失败"));
    }
    Ok(())
}

fn workspace_root_from_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir
        .ancestors()
        .find(|ancestor| {
            ancestor.join("frontend").join("src-tauri").exists()
                && ancestor.join("ai-processor").exists()
        })
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn bundled_resources_root(resource_dir: &Path) -> PathBuf {
    let nested = resource_dir.join("resources");
    if nested.exists() {
        nested
    } else {
        resource_dir.to_path_buf()
    }
}

fn resolve_resource_file(
    paths: &ControlPaths,
    relative: &[&str],
    dev_fallback: &[&str],
) -> Option<PathBuf> {
    let bundled = relative
        .iter()
        .fold(bundled_resources_root(&paths.resource_dir), |acc, part| {
            acc.join(part)
        });
    if bundled.exists() {
        return Some(bundled);
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let dev = dev_fallback
        .iter()
        .fold(repo_root, |acc, part| acc.join(part));
    if dev.exists() {
        Some(dev)
    } else {
        None
    }
}

fn bundled_binary_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn runtime_platform_slug() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        (os, arch) => {
            if os == "macos" {
                "darwin-unknown"
            } else if os == "windows" {
                "windows-unknown"
            } else if arch == "x86_64" {
                "linux-x64"
            } else {
                "linux-arm64"
            }
        }
    }
}

fn resolve_runtime_resource_file(paths: &ControlPaths, relative: &[&str]) -> Option<PathBuf> {
    let platform = runtime_platform_slug();
    let bundled = relative.iter().fold(
        bundled_resources_root(&paths.resource_dir)
            .join("runtime")
            .join(platform),
        |acc, part| acc.join(part),
    );
    if bundled.exists() {
        return Some(bundled);
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let dev = relative.iter().fold(
        repo_root
            .join("frontend")
            .join("src-tauri")
            .join("resources")
            .join("runtime")
            .join(platform),
        |acc, part| acc.join(part),
    );
    if dev.exists() {
        Some(dev)
    } else {
        None
    }
}

fn runtime_cache_platform_dir(paths: &ControlPaths) -> PathBuf {
    paths.runtime_cache_dir.join(runtime_platform_slug())
}

fn copy_directory_recursive(source: &Path, target: &Path) -> anyhow::Result<()> {
    for entry in WalkDir::new(source).follow_links(true) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &destination)?;
    }
    Ok(())
}

#[cfg(unix)]
fn mark_file_executable(path: &Path) -> anyhow::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn mark_file_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn ensure_runtime_file(
    paths: &ControlPaths,
    relative: &[&str],
    cache_segments: &[&str],
    executable: bool,
) -> anyhow::Result<PathBuf> {
    let source = resolve_runtime_resource_file(paths, relative)
        .ok_or_else(|| anyhow!("缺少运行资源: {}", relative.join("/")))?;
    let target = cache_segments
        .iter()
        .fold(runtime_cache_platform_dir(paths), |acc, part| {
            acc.join(part)
        });

    let should_copy = if target.exists() {
        fs::metadata(&target)?.len() != fs::metadata(&source)?.len()
    } else {
        true
    };

    if should_copy {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &target)?;
    }

    if executable {
        mark_file_executable(&target)?;
    }

    Ok(target)
}

fn ensure_runtime_directory(
    paths: &ControlPaths,
    relative: &[&str],
    cache_segments: &[&str],
    executable_rel_path: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let source = resolve_runtime_resource_file(paths, relative)
        .ok_or_else(|| anyhow!("缺少运行目录资源: {}", relative.join("/")))?;
    let target = cache_segments
        .iter()
        .fold(runtime_cache_platform_dir(paths), |acc, part| {
            acc.join(part)
        });
    let executable_path = executable_rel_path.map(|path| target.join(path));
    let should_copy = !target.exists()
        || executable_path
            .as_ref()
            .map(|path| !path.exists())
            .unwrap_or(false);

    if should_copy {
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::create_dir_all(&target)?;
        copy_directory_recursive(&source, &target)?;
    }

    if let Some(executable_path) = executable_rel_path {
        mark_file_executable(&target.join(executable_path))?;
    }

    Ok(target)
}

fn resolve_ffmpeg_path(paths: &ControlPaths) -> PathBuf {
    let binary = bundled_binary_name("ffmpeg");
    ensure_runtime_file(
        paths,
        &["tools", binary.as_str()],
        &["tools", binary.as_str()],
        true,
    )
    .unwrap_or_else(|_| PathBuf::from(binary))
}

fn resolve_ffprobe_path(paths: &ControlPaths) -> PathBuf {
    let binary = bundled_binary_name("ffprobe");
    ensure_runtime_file(
        paths,
        &["tools", binary.as_str()],
        &["tools", binary.as_str()],
        true,
    )
    .unwrap_or_else(|_| PathBuf::from(binary))
}

fn resolve_model_path(paths: &ControlPaths) -> anyhow::Result<PathBuf> {
    resolve_resource_file(
        paths,
        &["models", "best.pt"],
        &["ai-processor", "weights", "best.pt"],
    )
    .ok_or_else(|| anyhow!("无法解析模型文件 best.pt"))
}

enum WorkerLaunch {
    Packaged {
        executable: PathBuf,
    },
    PythonScript {
        python: String,
        script: PathBuf,
        python_path: PathBuf,
    },
}

fn resolve_worker_launch(paths: &ControlPaths) -> anyhow::Result<WorkerLaunch> {
    let packaged_name = bundled_binary_name("desktop_worker");
    if resolve_runtime_resource_file(paths, &["worker", "desktop_worker", packaged_name.as_str()])
        .is_some()
    {
        let worker_dir = ensure_runtime_directory(
            paths,
            &["worker", "desktop_worker"],
            &["worker", "desktop_worker"],
            Some(packaged_name.as_str()),
        )?;
        let executable = worker_dir.join(packaged_name);
        return Ok(WorkerLaunch::Packaged { executable });
    }

    let repo_root = workspace_root_from_resource_dir(&paths.resource_dir);
    let ai_dir = repo_root.join("ai-processor");
    let script = ai_dir.join("desktop_worker.py");
    if script.exists() {
        return Ok(WorkerLaunch::PythonScript {
            python: "python3".to_string(),
            script,
            python_path: ai_dir,
        });
    }

    Err(anyhow!("无法解析桌面 worker 可执行文件"))
}

fn start_media_server(tokens: Arc<RwLock<HashMap<String, MediaToken>>>) -> anyhow::Result<u16> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = std_listener.local_addr()?.port();
    std_listener.set_nonblocking(true)?;

    let app = Router::new()
        .route("/media/{token}", any(media_handler))
        .with_state(MediaServerState { tokens });

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("failed to create media runtime: {error}");
                return;
            }
        };

        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(listener) => listener,
                Err(error) => {
                    eprintln!("failed to create media listener: {error}");
                    return;
                }
            };

            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("media server stopped: {error}");
            }
        });
    });

    Ok(port)
}

#[derive(Clone)]
struct MediaServerState {
    tokens: Arc<RwLock<HashMap<String, MediaToken>>>,
}

async fn media_handler(
    AxumState(state): AxumState<MediaServerState>,
    AxumPath(token): AxumPath<String>,
    request: Request<Body>,
) -> Response {
    let now = Instant::now();
    let path = {
        let tokens = state.tokens.read();
        tokens
            .get(&token)
            .filter(|media| media.expires_at > now)
            .map(|media| media.path.clone())
    };

    match path {
        Some(path) => match ServeFile::new(path).oneshot(request).await {
            Ok(response) => response.into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn initialize_runtime_task_state(state: &DesktopState) -> anyhow::Result<()> {
    let conn = state.open_db()?;
    let recovery_tasks = load_pending_queue_recovery_tasks(&conn)?;
    *state.runtime.pending_queue_recovery.write() = if recovery_tasks.is_empty() {
        None
    } else {
        Some(recovery_tasks)
    };
    Ok(())
}

fn load_pending_queue_recovery_tasks(conn: &Connection) -> anyhow::Result<Vec<QueueRecoveryTask>> {
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

fn spawn_scheduler_tick_loop(state: DesktopState, app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        if let Err(error) = try_schedule_tasks(&state, &app) {
            eprintln!("scheduler tick failed: {error}");
        }
    });
}

fn has_pending_queue_recovery(state: &DesktopState) -> bool {
    state
        .runtime
        .pending_queue_recovery
        .read()
        .as_ref()
        .map(|tasks| !tasks.is_empty())
        .unwrap_or(false)
}

fn try_schedule_tasks(state: &DesktopState, app: &AppHandle) -> anyhow::Result<()> {
    let _guard = state.runtime.scheduler_lock.lock();
    try_schedule_tasks_locked(state, app)
}

fn try_schedule_tasks_locked(state: &DesktopState, app: &AppHandle) -> anyhow::Result<()> {
    if has_pending_queue_recovery(state) {
        state.emit_scheduler_state(app);
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
    Ok(())
}

fn can_launch_additional_task(state: &DesktopState, settings: &SchedulerSettings) -> bool {
    let snapshot = state.runtime.resource_probe.lock().snapshot();
    let available_ratio = if snapshot.total_memory_bytes == 0 {
        1.0
    } else {
        snapshot.available_memory_bytes as f64 / snapshot.total_memory_bytes as f64
    };

    let _ = (
        snapshot.gpu_percent,
        snapshot.gpu_memory_used_bytes,
        snapshot.gpu_memory_total_bytes,
    );

    snapshot.cpu_percent < settings.mac_cpu_limit_percent
        && available_ratio >= settings.mac_min_available_memory_ratio
}

fn next_queued_task_id(conn: &Connection) -> anyhow::Result<Option<i64>> {
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

fn dispatch_queued_task(state: &DesktopState, app: &AppHandle, task_id: i64) -> anyhow::Result<()> {
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

fn spawn_task_execution(state: DesktopState, app: AppHandle, task_id: i64) {
    std::thread::spawn(move || {
        let result = run_task_worker(&state, &app, task_id);
        if let Err(error) = result {
            let _ = mark_task_failed(&state, &app, task_id, &error.to_string());
        }

        state.runtime.active_tasks.write().remove(&task_id);
        state.emit_scheduler_state(&app);
        if let Err(error) = try_schedule_tasks(&state, &app) {
            eprintln!("scheduler dispatch failed after task completion: {error}");
        }
    });
}

fn emit_queue_updates(state: &DesktopState, app: &AppHandle) -> anyhow::Result<()> {
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

fn load_queued_task_positions(conn: &Connection) -> anyhow::Result<Vec<(i64, i64)>> {
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

fn queue_position_for_task(conn: &Connection, task_id: i64) -> anyhow::Result<Option<i64>> {
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

fn enqueue_tasks(
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
    }

    try_schedule_tasks(state, app)?;
    Ok(task_ids.to_vec())
}

fn run_task_worker(state: &DesktopState, app: &AppHandle, task_id: i64) -> anyhow::Result<()> {
    let media_root = state.current_media_root()?;
    let task_dir = task_root(&media_root, task_id);
    let mut conn = state.open_db()?;
    let loaded = load_task_with_paths(&mut conn, task_id)?;
    let config = loaded
        .response
        .config
        .clone()
        .ok_or_else(|| anyhow!("任务缺少配置"))?;

    let video_abs = if let Some(rel) = loaded.analysis_input_rel.clone() {
        task_dir.join(rel)
    } else {
        task_dir.join(loaded.original_rel.clone())
    };

    let preprocessed_output_path = task_dir.join("output").join("preprocessed.mp4");
    let result_output_path = task_dir.join("output").join("result.mp4");
    let log_path = task_dir.join("logs").join("worker.log");
    let job_path = task_dir.join("work").join("task-job.json");

    let job = WorkerJob {
        task_id,
        video_path: video_abs.to_string_lossy().to_string(),
        video_duration: loaded.response.video_duration,
        timeout_threshold: loaded.response.timeout_threshold,
        model_path: resolve_model_path(&state.paths)?
            .to_string_lossy()
            .to_string(),
        device: String::new(),
        log_path: log_path.to_string_lossy().to_string(),
        preprocessed_output_path: preprocessed_output_path.to_string_lossy().to_string(),
        result_output_path: result_output_path.to_string_lossy().to_string(),
        config: WorkerJobConfig {
            confidence_threshold: 0.5,
            iou_threshold: 0.45,
            timeout_ratio: config.timeout_ratio,
            frame_rate: config.frame_rate,
            enable_preprocessing: config.enable_preprocessing,
            preprocessing_strength: config.preprocessing_strength,
            preprocessing_enhance_pool: config.preprocessing_enhance_pool,
            enable_tracking_merge: config.enable_tracking_merge,
            tracking_merge_strategy: config.tracking_merge_strategy,
        },
    };
    fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;

    let ffmpeg_path = resolve_ffmpeg_path(&state.paths);
    let ffprobe_path = resolve_ffprobe_path(&state.paths);
    let worker_launch = resolve_worker_launch(&state.paths)?;

    let mut command = match worker_launch {
        WorkerLaunch::Packaged { executable } => {
            let mut command = Command::new(executable);
            command.arg(&job_path);
            command
        }
        WorkerLaunch::PythonScript {
            python,
            script,
            python_path,
        } => {
            let mut command = Command::new(python);
            command.arg(script).arg(&job_path);
            command.env("PYTHONPATH", python_path);
            command
        }
    };

    command
        .env("YOLO_MODEL_PATH", resolve_model_path(&state.paths)?)
        .env("ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS", "1")
        .env("FFMPEG_BIN", ffmpeg_path)
        .env("FFPROBE_BIN", ffprobe_path)
        .env("STORAGE_BASE_PATH", &task_dir)
        .env("STORAGE_PREPROCESSED_VIDEOS_SUBDIR", "output")
        .env("STORAGE_RESULT_VIDEOS_SUBDIR", "output")
        .env("STORAGE_TRACKING_RESULTS_SUBDIR", "output")
        .env("TRACKING_RESULTS_FILENAME_TEMPLATE", "tracking.json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().context("启动桌面 worker 失败")?;
    let stdout = child.stdout.take().context("worker stdout 不可用")?;
    let stderr = child.stderr.take().context("worker stderr 不可用")?;

    let stderr_log_path = log_path.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log_path)
            .ok();
        for line in reader.lines().map_while(Result::ok) {
            if let Some(file) = log_file.as_mut() {
                let _ = writeln!(file, "{line}");
            }
        }
    });

    let mut final_received = false;
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<WorkerEvent>(&line) {
            Ok(event) => {
                final_received |= handle_worker_event(state, app, task_id, event)?;
            }
            Err(error) => {
                eprintln!("invalid worker event: {error} {line}");
            }
        }
    }

    let status = child.wait()?;
    if !status.success() && !final_received {
        return Err(anyhow!("worker 非正常退出: {status}"));
    }

    Ok(())
}

fn handle_worker_event(
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

fn apply_progress_payload(
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

fn persist_result_payload(
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

fn final_status_response(state: &DesktopState, task_id: i64) -> anyhow::Result<TaskStatusResponse> {
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

fn relative_to_task_output(
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

fn mark_task_failed(
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

fn load_task_with_paths(conn: &mut Connection, task_id: i64) -> anyhow::Result<LoadedTask> {
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

fn load_task_response(conn: &mut Connection, task_id: i64) -> anyhow::Result<TaskResponse> {
    let mut response = load_task_with_paths(conn, task_id)?.response;
    response.queue_position = queue_position_for_task(conn, task_id)?;
    Ok(response)
}

fn load_task_config(conn: &mut Connection, task_id: i64) -> anyhow::Result<Option<TaskConfigData>> {
    let mut stmt = conn.prepare(
        "SELECT timeout_ratio, model_version, enable_preprocessing, preprocessing_strength,
                preprocessing_enhance_pool, enable_tracking_merge, tracking_merge_strategy, frame_rate
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
                enable_tracking_merge: row.get::<_, i64>(5)? != 0,
                tracking_merge_strategy: row.get(6)?,
                frame_rate: row.get(7)?,
            })
        })
        .optional()?;

    Ok(result)
}

#[tauri::command]
fn get_app_state(state: State<DesktopState>) -> CommandResult<AppStateResponse> {
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

#[tauri::command]
fn initialize_media_library(
    path: String,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let root = PathBuf::from(path);
    ensure_library_structure(&root).map_err(|error| error.to_string())?;
    state
        .set_media_root(&root)
        .map_err(|error| error.to_string())?;
    get_app_state(state)
}

#[tauri::command]
fn select_existing_media_library(
    path: String,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let root = PathBuf::from(path);
    validate_library_root(&root).map_err(|error| error.to_string())?;
    state
        .set_media_root(&root)
        .map_err(|error| error.to_string())?;
    get_app_state(state)
}

#[tauri::command]
fn migrate_media_library(
    path: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<AppStateResponse> {
    let current_root = state
        .current_media_root()
        .map_err(|error| error.to_string())?;
    let target_root = PathBuf::from(path);
    if current_root == target_root {
        return Err("媒体库目标目录不能与当前目录相同".to_string());
    }
    if target_root.exists()
        && fs::read_dir(&target_root)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err("迁移目标目录必须为空目录或不存在".to_string());
    }

    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "preparing", "progress": 0.0, "message": "准备迁移媒体库" }),
    );
    ensure_library_structure(&target_root).map_err(|error| error.to_string())?;

    let entries: Vec<_> = WalkDir::new(&current_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect();
    let total = entries.len().max(1);

    for (index, entry) in entries.iter().enumerate() {
        let relative = entry
            .path()
            .strip_prefix(&current_root)
            .map_err(|error| error.to_string())?;
        let target = target_root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(entry.path(), &target).map_err(|error| error.to_string())?;
        let _ = app.emit(
            "library-migration-progress",
            json!({
                "stage": "copying",
                "progress": (index + 1) as f64 / total as f64,
                "message": relative.to_string_lossy()
            }),
        );
    }

    validate_library_root(&target_root).map_err(|error| error.to_string())?;
    {
        let mut settings = state.runtime.settings.write();
        settings.recent_library_migrations.push(MigrationRecord {
            from: current_root.to_string_lossy().to_string(),
            to: target_root.to_string_lossy().to_string(),
            migrated_at: now_string(),
        });
        settings.media_library_path = Some(target_root.to_string_lossy().to_string());
    }
    state.save_settings().map_err(|error| error.to_string())?;
    let _ = app.emit(
        "library-migration-progress",
        json!({ "stage": "completed", "progress": 1.0, "message": "媒体库迁移完成" }),
    );
    get_app_state(state)
}

fn default_task_config_input() -> TaskConfigInput {
    TaskConfigInput {
        timeout_ratio: Some("1:4".to_string()),
        enable_preprocessing: Some(false),
        preprocessing_strength: Some("moderate".to_string()),
        preprocessing_enhance_pool: Some(false),
        enable_tracking_merge: Some(false),
        tracking_merge_strategy: Some("auto".to_string()),
    }
}

fn default_task_name(source_path: &Path) -> String {
    source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名任务")
        .to_string()
}

fn create_task_from_import(
    state: &DesktopState,
    file_path: &str,
    name: Option<String>,
    config: Option<TaskConfigInput>,
) -> anyhow::Result<TaskResponse> {
    let media_root = state.current_media_root()?;
    validate_library_root(&media_root)?;

    let source_path = PathBuf::from(file_path);
    if !source_path.exists() {
        return Err(anyhow!("源视频文件不存在"));
    }

    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "mp4" | "mov" | "avi" | "mkv") {
        return Err(anyhow!("不支持的视频格式，仅支持 mp4/mov/avi/mkv"));
    }

    let ffprobe_path = resolve_ffprobe_path(&state.paths);
    let ffmpeg_path = resolve_ffmpeg_path(&state.paths);
    let info = run_ffprobe(&ffprobe_path, &source_path)?;
    let config_input = config.unwrap_or_else(default_task_config_input);
    let timeout_ratio = config_input
        .timeout_ratio
        .unwrap_or_else(|| "1:4".to_string());
    let enable_preprocessing = config_input.enable_preprocessing.unwrap_or(false);
    let preprocessing_strength = config_input
        .preprocessing_strength
        .unwrap_or_else(|| "moderate".to_string());
    let preprocessing_enhance_pool = if enable_preprocessing {
        config_input.preprocessing_enhance_pool.unwrap_or(false)
    } else {
        false
    };
    let enable_tracking_merge = config_input.enable_tracking_merge.unwrap_or(false);
    let tracking_merge_strategy = config_input
        .tracking_merge_strategy
        .unwrap_or_else(|| "auto".to_string());
    let timeout_threshold = parse_timeout_ratio(&timeout_ratio, info.duration_seconds)?;

    let mut conn = state.open_db()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO analysis_tasks (name, original_filename, original_video_rel_path, analysis_input_rel_path, result_video_rel_path,
                                     preprocessed_video_rel_path, tracking_rel_path, video_duration, status, timeout_threshold,
                                     is_timeout, created_at, queue_order)
         VALUES (?1, ?2, '', NULL, NULL, NULL, NULL, ?3, 'PENDING', ?4, 0, ?5, NULL)",
        params![
            name.unwrap_or_else(|| default_task_name(&source_path)),
            source_path.file_name().and_then(|value| value.to_str()).map(str::to_string),
            info.duration_seconds,
            timeout_threshold,
            now_string()
        ],
    )?;
    let task_id = tx.last_insert_rowid();

    tx.execute(
        "INSERT INTO task_configs (task_id, timeout_ratio, model_version, enable_preprocessing, preprocessing_strength,
                                   preprocessing_enhance_pool, enable_tracking_merge, tracking_merge_strategy, frame_rate)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            task_id,
            timeout_ratio,
            enable_preprocessing as i64,
            preprocessing_strength,
            preprocessing_enhance_pool as i64,
            enable_tracking_merge as i64,
            tracking_merge_strategy,
            info.frame_rate
        ],
    )?;

    let task_dir = task_root(&media_root, task_id);
    ensure_task_dirs(&task_dir)?;

    let original_rel = format!("input/original.{}", extension);
    let original_abs = task_dir.join(&original_rel);
    fs::copy(&source_path, &original_abs)?;

    let analysis_input_rel = if is_web_friendly_codec(&source_path, info.codec_name.as_deref()) {
        None
    } else {
        let rel = "work/analysis_input.mp4".to_string();
        let abs = task_dir.join(&rel);
        normalize_analysis_input(&ffmpeg_path, &original_abs, &abs)?;
        Some(rel)
    };

    tx.execute(
        "UPDATE analysis_tasks SET original_video_rel_path = ?1, analysis_input_rel_path = ?2 WHERE id = ?3",
        params![original_rel, analysis_input_rel, task_id],
    )?;

    tx.commit()?;
    state.load_task_response(task_id)
}

#[tauri::command]
fn import_video_task(
    request: ImportTaskRequest,
    state: State<DesktopState>,
) -> CommandResult<TaskResponse> {
    create_task_from_import(&state, &request.file_path, request.name, request.config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_video_tasks(
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
fn update_scheduler_settings(
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
    }

    state.save_settings().map_err(|error| error.to_string())?;
    state.emit_scheduler_state(&app);
    try_schedule_tasks(&state, &app).map_err(|error| error.to_string())?;
    get_app_state(state)
}

#[tauri::command]
fn get_queue_recovery_state(
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
fn resolve_queue_recovery(
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
        for task in &recovery_tasks {
            let task_id = task
                .task_id
                .parse::<i64>()
                .map_err(|error| error.to_string())?;
            conn.execute(
                "UPDATE analysis_tasks
                 SET status = 'PENDING',
                     queue_order = NULL
                 WHERE id = ?1 AND status = 'QUEUED'",
                params![task_id],
            )
            .map_err(|error| error.to_string())?;
        }
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

#[tauri::command]
fn list_tasks(
    request: ListTasksRequest,
    state: State<DesktopState>,
) -> CommandResult<PageResult<TaskResponse>> {
    let page = request.page.unwrap_or(0).max(0);
    let size = request.size.unwrap_or(20).clamp(1, 100);
    let offset = page * size;
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let order_clause = build_task_list_order_clause(
        request.status.as_deref(),
        request.sort_by.as_deref(),
        request.sort_direction.as_deref(),
    );

    let (total, ids) = if let Some(status) = request.status.clone() {
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM analysis_tasks WHERE status = ?1",
                params![status.clone()],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let query_sql = format!(
            "SELECT id
               FROM analysis_tasks
              WHERE status = ?1
              ORDER BY {order_clause}
              LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn
            .prepare(&query_sql)
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![status, size, offset], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())?;
        let ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        (total, ids)
    } else {
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM analysis_tasks", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        let query_sql = format!(
            "SELECT id
               FROM analysis_tasks
              ORDER BY {order_clause}
              LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn
            .prepare(&query_sql)
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![size, offset], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())?;
        let ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        (total, ids)
    };

    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        items.push(load_task_response(&mut conn, id).map_err(|error| error.to_string())?);
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

fn build_task_list_order_clause(
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

#[tauri::command]
fn get_task(task_id: String, state: State<DesktopState>) -> CommandResult<TaskResponse> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    state
        .load_task_response(task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_task_status(
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
fn get_task_result(
    task_id: String,
    state: State<DesktopState>,
) -> CommandResult<TaskResultResponse> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let task = load_task_response(&mut conn, task_id).map_err(|error| error.to_string())?;
    if !matches!(task.status.as_str(), "COMPLETED" | "COMPLETED_TIMEOUT") {
        return Err("任务尚未完成，无法获取结果".to_string());
    }

    let dynamic_metrics = {
        let mut stmt = conn
            .prepare(
                "SELECT frame_number, timestamp, brightness, pool_area, pool_perimeter
                   FROM dynamic_metrics WHERE task_id = ?1 ORDER BY frame_number ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![task_id], |row| {
                Ok(DynamicMetricData {
                    frame_number: row.get(0)?,
                    timestamp: row.get(1)?,
                    brightness: row.get(2)?,
                    pool_area: row.get(3)?,
                    pool_perimeter: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    let anomaly_events = {
        let mut stmt = conn
            .prepare(
                "SELECT id, event_type, start_frame, end_frame, object_id, metadata_json
                   FROM anomaly_events WHERE task_id = ?1 ORDER BY start_frame ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![task_id], |row| {
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
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    let tracking_objects = {
        let mut stmt = conn
            .prepare(
                "SELECT id, object_id, category, first_frame, last_frame, trajectory_json
                   FROM tracking_objects WHERE task_id = ?1 ORDER BY first_frame ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![task_id], |row| {
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
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
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
        let raw: Option<String> = conn
            .query_row(
                "SELECT global_analysis_json FROM analysis_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
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

#[tauri::command]
fn start_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    enqueue_tasks(&state, &app, &[task_id], &["PENDING"]).map_err(|error| error.to_string())?;
    Ok("任务已加入分析队列".to_string())
}

#[tauri::command]
fn dequeue_task(
    task_id: String,
    app: AppHandle,
    state: State<DesktopState>,
) -> CommandResult<String> {
    let task_id = task_id.parse::<i64>().map_err(|error| error.to_string())?;
    if has_pending_queue_recovery(&state) {
        return Err("存在待恢复的排队任务，请先处理恢复弹窗".to_string());
    }

    let mut conn = state.open_db().map_err(|error| error.to_string())?;
    let task = load_task_response(&mut conn, task_id).map_err(|error| error.to_string())?;
    if task.status != "QUEUED" {
        return Err(format!("当前任务状态不允许移出队列: {}", task.status));
    }

    let updated = conn
        .execute(
            "UPDATE analysis_tasks
             SET status = 'PENDING',
                 queue_order = NULL
             WHERE id = ?1 AND status = 'QUEUED'",
            params![task_id],
        )
        .map_err(|error| error.to_string())?;
    if updated == 0 {
        return Err("任务状态已变化，无法移出队列".to_string());
    }

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
fn reanalyze_task(
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

    conn.execute(
        "DELETE FROM dynamic_metrics WHERE task_id = ?1",
        params![task_id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM anomaly_events WHERE task_id = ?1",
        params![task_id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM tracking_objects WHERE task_id = ?1",
        params![task_id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE analysis_tasks
         SET status = 'PENDING',
             is_timeout = 0,
             global_analysis_json = NULL,
             result_video_rel_path = NULL,
             preprocessed_video_rel_path = NULL,
             tracking_rel_path = NULL,
             queue_order = NULL,
             started_at = NULL,
             preprocessing_completed_at = NULL,
             completed_at = NULL,
             failure_reason = NULL
         WHERE id = ?1",
        params![task_id],
    )
    .map_err(|error| error.to_string())?;

    let _ = fs::remove_file(task_dir.join("output").join("result.mp4"));
    let _ = fs::remove_file(task_dir.join("output").join("preprocessed.mp4"));
    let _ = fs::remove_file(task_dir.join("output").join("tracking.json"));

    state.runtime.progress.write().remove(&task_id);
    enqueue_tasks(&state, &app, &[task_id], &["PENDING"]).map_err(|error| error.to_string())?;
    Ok("任务已重新进入分析队列".to_string())
}

#[tauri::command]
fn delete_task(
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

    conn.execute("DELETE FROM analysis_tasks WHERE id = ?1", params![task_id])
        .map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(task_dir);
    state.runtime.progress.write().remove(&task_id);
    if task.status == "QUEUED" {
        emit_queue_updates(&state, &app).map_err(|error| error.to_string())?;
        state.emit_scheduler_state(&app);
    }
    Ok("任务删除成功".to_string())
}

#[tauri::command]
fn get_video_stream_url(
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

#[tauri::command]
fn export_report_file(request: ExportReportRequest) -> CommandResult<String> {
    let path = PathBuf::from(&request.path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if let Some(text_content) = request.text_content {
        fs::write(&path, text_content).map_err(|error| error.to_string())?;
    } else if let Some(base64_content) = request.base64_content {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_content)
            .map_err(|error| error.to_string())?;
        fs::write(&path, bytes).map_err(|error| error.to_string())?;
    } else {
        return Err("缺少导出内容".to_string());
    }

    Ok(path.to_string_lossy().to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = DesktopState::bootstrap(&app.handle()).map_err(
                |error| -> Box<dyn std::error::Error> {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        error.to_string(),
                    ))
                },
            )?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            initialize_media_library,
            select_existing_media_library,
            migrate_media_library,
            import_video_task,
            import_video_tasks,
            list_tasks,
            get_task,
            get_task_status,
            get_task_result,
            start_task,
            dequeue_task,
            reanalyze_task,
            delete_task,
            get_video_stream_url,
            export_report_file,
            update_scheduler_settings,
            get_queue_recovery_state,
            resolve_queue_recovery
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
