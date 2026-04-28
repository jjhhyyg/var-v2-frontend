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
    let log_path = worker_log_path(task_id);
    let job_path = task_dir.join("work").join("task-job.json");

    let model_path = resolve_model_path(&state.paths)?;
    let job = WorkerJob {
        task_id,
        video_path: video_abs.to_string_lossy().to_string(),
        video_duration: loaded.response.video_duration,
        timeout_threshold: loaded.response.timeout_threshold,
        model_path: model_path.to_string_lossy().to_string(),
        device: String::new(),
        log_path: log_path.to_string_lossy().to_string(),
        preprocessed_output_path: preprocessed_output_path.to_string_lossy().to_string(),
        config: WorkerJobConfig {
            confidence_threshold: 0.5,
            iou_threshold: 0.45,
            timeout_ratio: config.timeout_ratio,
            frame_rate: config.frame_rate,
            enable_preprocessing: config.enable_preprocessing,
            preprocessing_strength: config.preprocessing_strength,
            preprocessing_enhance_pool: config.preprocessing_enhance_pool,
            enable_dynamic_metrics: config.enable_dynamic_metrics,
        },
    };
    cleanup_logs(LogNamespace::Worker)?;
    fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;

    let ffmpeg_path = resolve_ffmpeg_path(&state.paths);
    let ffprobe_path = resolve_ffprobe_path(&state.paths);
    let var_gpu_preprocessor_path = resolve_var_gpu_preprocessor_path(&state.paths);
    let var_video_analyzer_path = resolve_var_video_analyzer_path(&state.paths);
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
        .env("YOLO_MODEL_PATH", &model_path)
        .env(
            "ONNX_REQUIRE_CUDA",
            if cfg!(target_os = "windows") {
                "1"
            } else {
                "0"
            },
        )
        .env("ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env("PYTHONUNBUFFERED", "1")
        .env("FFMPEG_BIN", ffmpeg_path)
        .env("FFPROBE_BIN", ffprobe_path)
        .env("GPU_PREPROCESSOR_BIN", var_gpu_preprocessor_path)
        .env("VAR_VIDEO_ANALYZER_BIN", var_video_analyzer_path)
        .env("STORAGE_BASE_PATH", &task_dir)
        .env("STORAGE_PREPROCESSED_VIDEOS_SUBDIR", "output")
        .env("STORAGE_DETECTION_RESULTS_SUBDIR", "output")
        .env("DETECTION_RESULTS_FILENAME_TEMPLATE", "detections.json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let worker_program = command.get_program().to_string_lossy().to_string();
    backend_log_info(format!(
        "task {task_id} worker starting program={worker_program} job={}",
        job_path.display()
    ));

    suppress_command_window(&mut command);
    let mut child = command.spawn().with_context(|| {
        format!(
            "启动桌面 worker 失败，请检查 Python/worker 可执行文件是否可用: {:?}",
            command.get_program()
        )
    })?;
    let _worker_process_job = attach_worker_process_job(&child, task_id, &log_path);
    let stdout = child.stdout.take().context("worker stdout 不可用")?;
    let stderr = child.stderr.take().context("worker stderr 不可用")?;

    let stderr_log_path = log_path.clone();
    std::thread::spawn(move || {
        copy_worker_text_stream_to_log(stderr, stderr_log_path);
    });

    let mut final_received = false;
    let mut stdout_reader = BufReader::new(stdout);
    let mut line_bytes = Vec::new();
    loop {
        line_bytes.clear();
        let bytes_read = match stdout_reader.read_until(b'\n', &mut line_bytes) {
            Ok(bytes_read) => bytes_read,
            Err(error) => {
                append_worker_log_line(&log_path, &format!("worker stdout read error: {error:#}"));
                break;
            }
        };
        if bytes_read == 0 {
            break;
        }
        let line = decode_worker_line(&line_bytes);
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<WorkerEvent>(&line) {
            Ok(event) => {
                let event_type = event.event_type.clone();
                match handle_worker_event(state, app, task_id, event) {
                    Ok(received_final) => {
                        final_received |= received_final;
                    }
                    Err(error) => {
                        append_worker_log_line(
                            &log_path,
                            &format!(
                                "worker event handler error: {error:#}; type={event_type}; raw={line}"
                            ),
                        );
                        if matches!(event_type.as_str(), "result" | "failed") {
                            return Err(error).with_context(|| {
                                format!("处理 worker 最终事件失败: {event_type}")
                            });
                        }
                    }
                }
            }
            Err(error) => append_worker_log_line(
                &log_path,
                &format!("invalid worker stdout event: {error}; raw={line}"),
            ),
        }
    }

    let status = child.wait()?;
    backend_log_info(format!(
        "task {task_id} worker exited status={status} final_received={final_received}"
    ));
    if !final_received {
        return Err(anyhow!("worker 未输出最终结果事件: {status}"));
    }
    if !status.success() {
        return Err(anyhow!("worker 非正常退出: {status}"));
    }

    Ok(())
}

fn copy_worker_text_stream_to_log(stream: impl std::io::Read, log_path: PathBuf) {
    let mut reader = BufReader::new(stream);
    let mut line_bytes = Vec::new();
    loop {
        line_bytes.clear();
        match reader.read_until(b'\n', &mut line_bytes) {
            Ok(0) => break,
            Ok(_) => append_worker_log_line(&log_path, &decode_worker_line(&line_bytes)),
            Err(error) => {
                append_worker_log_line(&log_path, &format!("worker stderr read error: {error:#}"));
                break;
            }
        }
    }
}

fn decode_worker_line(bytes: &[u8]) -> String {
    let trimmed = bytes
        .strip_suffix(b"\n")
        .unwrap_or(bytes)
        .strip_suffix(b"\r")
        .unwrap_or_else(|| bytes.strip_suffix(b"\r\n").unwrap_or(bytes));
    String::from_utf8_lossy(trimmed).into_owned()
}

fn attach_worker_process_job(
    child: &std::process::Child,
    task_id: i64,
    log_path: &Path,
) -> Option<WorkerProcessJob> {
    match WorkerProcessJob::attach(child) {
        Ok(job) => {
            backend_log_info(format!(
                "task {task_id} worker process attached to kill-on-close job"
            ));
            Some(job)
        }
        Err(error) => {
            let message = format!("task {task_id} failed to attach worker process job: {error:#}");
            backend_log_error(&message);
            append_worker_log_line(log_path, &message);
            None
        }
    }
}

#[cfg(windows)]
struct WorkerProcessJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl WorkerProcessJob {
    fn attach(child: &std::process::Child) -> anyhow::Result<Self> {
        use std::mem::{size_of, zeroed};
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        unsafe {
            let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if handle == std::ptr::null_mut() {
                return Err(std::io::Error::last_os_error()).context("CreateJobObjectW failed");
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set_ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set_ok == 0 {
                let error = std::io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error).context("SetInformationJobObject failed");
            }

            let process_handle = child.as_raw_handle() as HANDLE;
            let assign_ok = AssignProcessToJobObject(handle, process_handle);
            if assign_ok == 0 {
                let error = std::io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error).context("AssignProcessToJobObject failed");
            }

            Ok(Self { handle })
        }
    }
}

#[cfg(windows)]
impl Drop for WorkerProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
struct WorkerProcessJob;

#[cfg(not(windows))]
impl WorkerProcessJob {
    fn attach(_child: &std::process::Child) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
