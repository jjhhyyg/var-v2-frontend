use crate::*;

const DEFAULT_WINDOWS_GPU_LIMIT_PERCENT: f64 = 60.0;
const DEFAULT_WINDOWS_MIN_AVAILABLE_GPU_MEMORY_RATIO: f64 = 0.15;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) media_library_path: Option<String>,
    pub(crate) cleanup_preprocessed: bool,
    pub(crate) recent_library_migrations: Vec<MigrationRecord>,
    pub(crate) scheduler: SchedulerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchedulerSettings {
    pub(crate) max_concurrency: usize,
    pub(crate) mac_cpu_limit_percent: f64,
    pub(crate) mac_min_available_memory_ratio: f64,
    #[serde(default = "default_windows_gpu_limit_percent")]
    pub(crate) windows_gpu_limit_percent: f64,
    #[serde(default = "default_windows_min_available_gpu_memory_ratio")]
    pub(crate) windows_min_available_gpu_memory_ratio: f64,
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
            windows_gpu_limit_percent: DEFAULT_WINDOWS_GPU_LIMIT_PERCENT,
            windows_min_available_gpu_memory_ratio: DEFAULT_WINDOWS_MIN_AVAILABLE_GPU_MEMORY_RATIO,
        }
    }
}

fn default_windows_gpu_limit_percent() -> f64 {
    DEFAULT_WINDOWS_GPU_LIMIT_PERCENT
}

fn default_windows_min_available_gpu_memory_ratio() -> f64 {
    DEFAULT_WINDOWS_MIN_AVAILABLE_GPU_MEMORY_RATIO
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationRecord {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) migrated_at: String,
}
pub(crate) fn load_settings(settings_path: &Path) -> anyhow::Result<AppSettings> {
    if !settings_path.exists() {
        let settings = AppSettings::default();
        save_settings(settings_path, &settings)?;
        return Ok(settings);
    }

    let raw = fs::read_to_string(settings_path)?;
    let settings = serde_json::from_str(&raw).unwrap_or_default();
    Ok(settings)
}

pub(crate) fn save_settings(settings_path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(settings_path, raw)?;
    Ok(())
}
