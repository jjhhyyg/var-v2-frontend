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
    pub(crate) result_output_path: String,
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
    pub(crate) enable_tracking_merge: bool,
    pub(crate) tracking_merge_strategy: String,
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
    pub(crate) dynamic_metrics: Option<Vec<DynamicMetricPayload>>,
    pub(crate) global_analysis: Option<Value>,
    pub(crate) anomaly_events: Option<Vec<AnomalyEventPayload>>,
    pub(crate) tracking_objects: Option<Vec<TrackingObjectPayload>>,
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
    pub(crate) object_id: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackingObjectPayload {
    pub(crate) object_id: i64,
    pub(crate) category: String,
    pub(crate) first_frame: i64,
    pub(crate) last_frame: i64,
    pub(crate) trajectory: Option<Value>,
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
