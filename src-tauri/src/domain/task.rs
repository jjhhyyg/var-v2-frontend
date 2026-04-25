use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskResponse {
    pub(crate) task_id: String,
    pub(crate) name: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) video_duration: i64,
    pub(crate) result_video_path: Option<String>,
    pub(crate) preprocessed_video_path: Option<String>,
    pub(crate) status: String,
    pub(crate) timeout_threshold: i64,
    pub(crate) is_timeout: bool,
    pub(crate) config: Option<TaskConfigData>,
    pub(crate) created_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) preprocessing_completed_at: Option<String>,
    pub(crate) completed_at: Option<String>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) queue_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskConfigData {
    pub(crate) timeout_ratio: String,
    pub(crate) model_version: Option<String>,
    pub(crate) enable_preprocessing: bool,
    pub(crate) preprocessing_strength: String,
    pub(crate) preprocessing_enhance_pool: bool,
    pub(crate) frame_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskStatusResponse {
    pub(crate) task_id: String,
    pub(crate) status: String,
    pub(crate) phase: Option<String>,
    pub(crate) progress: Option<f64>,
    pub(crate) current_frame: Option<i64>,
    pub(crate) total_frames: Option<i64>,
    pub(crate) preprocessing_duration: Option<i64>,
    pub(crate) analyzing_elapsed_time: Option<i64>,
    pub(crate) is_timeout: Option<bool>,
    pub(crate) timeout_warning: Option<bool>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) queue_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskResultResponse {
    pub(crate) task_id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) is_timeout: bool,
    pub(crate) video_info: VideoInfoData,
    pub(crate) performance: PerformanceData,
    pub(crate) dynamic_metrics: Vec<DynamicMetricData>,
    pub(crate) global_analysis: Option<Value>,
    pub(crate) anomaly_events: Vec<AnomalyEventData>,
    pub(crate) event_statistics: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoInfoData {
    pub(crate) source_video_fps: f64,
    pub(crate) total_frames: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerformanceData {
    pub(crate) preprocessing_average_fps: Option<f64>,
    pub(crate) defect_detection_average_fps: Option<f64>,
    pub(crate) preprocessing_duration_seconds: i64,
    pub(crate) defect_detection_duration_seconds: i64,
    pub(crate) detection_backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DynamicMetricData {
    pub(crate) frame_number: i64,
    pub(crate) timestamp: f64,
    pub(crate) brightness: Option<f64>,
    pub(crate) pool_area: Option<i64>,
    pub(crate) pool_perimeter: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnomalyEventData {
    pub(crate) event_id: String,
    pub(crate) event_type: String,
    pub(crate) start_frame: i64,
    pub(crate) end_frame: i64,
    pub(crate) start_time: Option<f64>,
    pub(crate) end_time: Option<f64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageResult<T> {
    pub(crate) items: Vec<T>,
    pub(crate) total: i64,
    pub(crate) total_pages: i64,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
    pub(crate) has_next: bool,
    pub(crate) has_previous: bool,
}
#[derive(Debug)]
pub(crate) struct LoadedTask {
    pub(crate) response: TaskResponse,
    pub(crate) original_rel: String,
    pub(crate) analysis_input_rel: Option<String>,
}
