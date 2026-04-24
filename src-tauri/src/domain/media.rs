use crate::*;

#[derive(Clone)]
pub(crate) struct MediaToken {
    pub(crate) path: PathBuf,
    pub(crate) expires_at: Instant,
}
#[derive(Debug, Clone)]
pub(crate) struct VideoInfo {
    pub(crate) duration_seconds: i64,
    pub(crate) frame_rate: f64,
    pub(crate) codec_name: Option<String>,
}
