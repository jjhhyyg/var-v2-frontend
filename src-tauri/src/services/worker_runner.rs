use crate::*;

pub(crate) fn run_task_worker(
    state: &DesktopState,
    app: &AppHandle,
    task_id: i64,
) -> anyhow::Result<()> {
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
    let log_path = worker_log_path(task_id);
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
        },
    };
    cleanup_logs(LogNamespace::Worker)?;
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
            command.current_dir(&python_path);
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
        .env("STORAGE_DETECTION_RESULTS_SUBDIR", "output")
        .env("DETECTION_RESULTS_FILENAME_TEMPLATE", "detections.json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    suppress_command_window(&mut command);
    let mut child = command.spawn().with_context(|| {
        format!(
            "启动桌面 worker 失败，请检查 Python/worker 可执行文件是否可用: {:?}",
            command.get_program()
        )
    })?;
    let stdout = child.stdout.take().context("worker stdout 不可用")?;
    let stderr = child.stderr.take().context("worker stderr 不可用")?;

    let stderr_log_path = log_path.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            append_worker_log_line(&stderr_log_path, &line);
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
            Err(error) => append_worker_log_line(
                &log_path,
                &format!("invalid worker stdout event: {error}; raw={line}"),
            ),
        }
    }

    let status = child.wait()?;
    if !status.success() && !final_received {
        return Err(anyhow!("worker 非正常退出: {status}"));
    }

    Ok(())
}
