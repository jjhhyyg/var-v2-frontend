use crate::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppStateResponse {
    pub(crate) initialized: bool,
    pub(crate) media_library_available: bool,
    pub(crate) media_library_path: Option<String>,
    pub(crate) recommended_media_library_path: String,
    pub(crate) max_concurrency: usize,
    pub(crate) mac_cpu_limit_percent: f64,
    pub(crate) mac_min_available_memory_ratio: f64,
    pub(crate) windows_gpu_limit_percent: f64,
    pub(crate) windows_min_available_gpu_memory_ratio: f64,
    pub(crate) active_task_count: usize,
    pub(crate) queued_task_count: usize,
    pub(crate) platform: String,
    pub(crate) version: String,
    pub(crate) runtime_required: bool,
    pub(crate) runtime_ready: bool,
    pub(crate) runtime_build_id: Option<String>,
    pub(crate) required_runtime_build_id: String,
    pub(crate) runtime_platform: String,
    pub(crate) runtime_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStateResponse {
    pub(crate) runtime_required: bool,
    pub(crate) runtime_ready: bool,
    pub(crate) runtime_build_id: Option<String>,
    pub(crate) required_runtime_build_id: String,
    pub(crate) runtime_platform: String,
    pub(crate) runtime_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceStateResponse {
    pub(crate) cpu_percent: f64,
    pub(crate) memory_used_percent: f64,
    pub(crate) gpu_percent: Option<f64>,
    pub(crate) gpu_memory_used_percent: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRuntimeRequest {
    pub(crate) path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeManifest {
    pub(crate) platform: String,
    pub(crate) runtime_build_id: String,
    pub(crate) app_version: String,
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) files: Vec<RuntimeManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeManifestFile {
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimePackageLock {
    pub(crate) platform: String,
    pub(crate) runtime_build_id: String,
    pub(crate) app_version: String,
    pub(crate) package_name: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchedulerStateResponse {
    pub(crate) max_concurrency: usize,
    pub(crate) active_task_count: usize,
    pub(crate) queued_task_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueRecoveryTask {
    pub(crate) task_id: String,
    pub(crate) name: String,
    pub(crate) queue_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueRecoveryStateResponse {
    pub(crate) has_pending_recovery: bool,
    pub(crate) tasks: Vec<QueueRecoveryTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskConfigInput {
    pub(crate) timeout_ratio: Option<String>,
    pub(crate) enable_preprocessing: Option<bool>,
    pub(crate) preprocessing_strength: Option<String>,
    pub(crate) preprocessing_enhance_pool: Option<bool>,
    pub(crate) enable_dynamic_metrics: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTaskRequest {
    pub(crate) file_path: String,
    pub(crate) name: Option<String>,
    pub(crate) config: Option<TaskConfigInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTaskItemRequest {
    pub(crate) file_path: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTasksRequest {
    pub(crate) items: Vec<ImportTaskItemRequest>,
    pub(crate) config: Option<TaskConfigInput>,
    pub(crate) auto_start: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTasksFailure {
    pub(crate) file_path: String,
    pub(crate) file_name: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTasksResponse {
    pub(crate) import_id: String,
    pub(crate) message: String,
    pub(crate) total_files: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTasksProgress {
    pub(crate) import_id: String,
    pub(crate) status: String,
    pub(crate) total_files: usize,
    pub(crate) processed_files: usize,
    pub(crate) file_path: Option<String>,
    pub(crate) file_name: Option<String>,
    pub(crate) created_task: Option<TaskResponse>,
    pub(crate) failed_file: Option<ImportTasksFailure>,
    pub(crate) queued_task_ids: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchedulerSettingsInput {
    pub(crate) max_concurrency: Option<usize>,
    pub(crate) mac_cpu_limit_percent: Option<f64>,
    pub(crate) mac_min_available_memory_ratio: Option<f64>,
    pub(crate) windows_gpu_limit_percent: Option<f64>,
    pub(crate) windows_min_available_gpu_memory_ratio: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolveQueueRecoveryRequest {
    pub(crate) continue_analysis: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListTasksRequest {
    pub(crate) page: Option<i64>,
    pub(crate) size: Option<i64>,
    pub(crate) status: Option<String>,
    pub(crate) sort_by: Option<String>,
    pub(crate) sort_direction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteTasksRequest {
    pub(crate) task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteTasksResponse {
    pub(crate) deleted_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportReportRequest {
    pub(crate) path: String,
    pub(crate) text_content: Option<String>,
    pub(crate) base64_content: Option<String>,
}
