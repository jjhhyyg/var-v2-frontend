use crate::*;

pub(crate) fn default_task_config_input() -> TaskConfigInput {
    TaskConfigInput {
        timeout_ratio: Some("1:4".to_string()),
        enable_preprocessing: Some(false),
        preprocessing_strength: Some("moderate".to_string()),
        preprocessing_enhance_pool: Some(false),
        enable_tracking_merge: Some(false),
        tracking_merge_strategy: Some("auto".to_string()),
    }
}

pub(crate) fn default_task_name(source_path: &Path) -> String {
    source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名任务")
        .to_string()
}

pub(crate) fn create_task_from_import(
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
