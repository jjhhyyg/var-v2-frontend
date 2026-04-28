use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerJob {
    pub(crate) task_id: i64,
    pub(crate) video_path: String,
    pub(crate) video_duration: i64,
    pub(crate) timeout_threshold: i64,
    pub(crate) model_path: String,
    pub(crate) device: String,
    pub(crate) log_path: String,
    pub(crate) preprocessed_output_path: String,
    pub(crate) config: WorkerJobConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerJobConfig {
    pub(crate) confidence_threshold: f64,
    pub(crate) iou_threshold: f64,
    pub(crate) timeout_ratio: String,
    pub(crate) frame_rate: f64,
    pub(crate) enable_preprocessing: bool,
    pub(crate) preprocessing_strength: String,
    pub(crate) preprocessing_enhance_pool: bool,
    pub(crate) enable_dynamic_metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerEvent {
    #[serde(rename = "type")]
    pub(crate) event_type: String,
    pub(crate) payload: Value,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResultPayload {
    pub(crate) status: String,
    pub(crate) is_timeout: Option<bool>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) video_info: Option<VideoInfoPayload>,
    pub(crate) performance: Option<PerformancePayload>,
    pub(crate) dynamic_metrics: Option<Vec<DynamicMetricPayload>>,
    pub(crate) global_analysis: Option<Value>,
    pub(crate) anomaly_events: Option<Vec<AnomalyEventPayload>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoInfoPayload {
    pub(crate) source_video_fps: f64,
    pub(crate) total_frames: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerformancePayload {
    pub(crate) preprocessing_average_fps: Option<f64>,
    pub(crate) defect_detection_average_fps: Option<f64>,
    pub(crate) preprocessing_duration_seconds: i64,
    pub(crate) defect_detection_duration_seconds: i64,
    pub(crate) detection_backend: String,
    pub(crate) preprocessing_benchmark: Option<PreprocessingBenchmarkData>,
    pub(crate) timing_summary: Option<TimingSummaryData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerformanceTracePayload {
    pub(crate) timing_summary: TimingSummaryData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DynamicMetricPayload {
    pub(crate) frame_number: i64,
    pub(crate) timestamp: f64,
    pub(crate) brightness: Option<f64>,
    pub(crate) pool_area: Option<i64>,
    pub(crate) pool_perimeter: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnomalyEventPayload {
    pub(crate) event_type: String,
    pub(crate) start_frame: i64,
    pub(crate) end_frame: i64,
    pub(crate) start_time: Option<f64>,
    pub(crate) end_time: Option<f64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProgressPayload {
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
}
