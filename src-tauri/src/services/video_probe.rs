use crate::*;

pub(crate) fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn parse_timeout_ratio(timeout_ratio: &str, video_duration: i64) -> anyhow::Result<i64> {
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

pub(crate) fn run_ffprobe(ffprobe_path: &Path, file_path: &Path) -> anyhow::Result<VideoInfo> {
    let mut command = Command::new(ffprobe_path);
    command
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
        .arg(file_path);
    let output = suppress_command_window(&mut command)
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

pub(crate) fn parse_ratio(raw: &str) -> anyhow::Result<f64> {
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

pub(crate) fn is_web_friendly_codec(path: &Path, codec_name: Option<&str>) -> bool {
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

pub(crate) fn normalize_analysis_input(
    ffmpeg_path: &Path,
    source: &Path,
    target: &Path,
) -> anyhow::Result<()> {
    let mut command = Command::new(ffmpeg_path);
    command.args([
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
    ]);
    let status = suppress_command_window(&mut command)
        .status()
        .context("执行 ffmpeg 标准化失败")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg 标准化视频失败"));
    }
    Ok(())
}
