use crate::*;

#[tauri::command]
pub(crate) fn export_report_file(request: ExportReportRequest) -> CommandResult<String> {
    let path = PathBuf::from(&request.path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if let Some(text_content) = request.text_content {
        fs::write(&path, text_content).map_err(|error| error.to_string())?;
    } else if let Some(base64_content) = request.base64_content {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_content)
            .map_err(|error| error.to_string())?;
        fs::write(&path, bytes).map_err(|error| error.to_string())?;
    } else {
        return Err("缺少导出内容".to_string());
    }

    Ok(path.to_string_lossy().to_string())
}
