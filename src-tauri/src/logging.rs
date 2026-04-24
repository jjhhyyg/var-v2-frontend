use crate::*;
use chrono::{Datelike, Local};
use std::panic;
use std::sync::{Once, OnceLock};

const LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
const LOG_RETENTION_DAYS: i64 = 30;
const LOG_RETENTION_FILES: usize = 200;

static LOG_ROOT: OnceLock<PathBuf> = OnceLock::new();
static PANIC_HOOK: Once = Once::new();

#[derive(Clone, Copy)]
pub(crate) enum LogNamespace {
    Backend,
    Worker,
}

impl LogNamespace {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Worker => "worker",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum LogLevel {
    Info,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Error => "ERROR",
        }
    }
}

pub(crate) fn init_logging(log_root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(log_root)?;
    let _ = LOG_ROOT.set(log_root.to_path_buf());
    cleanup_logs(LogNamespace::Backend)?;
    cleanup_logs(LogNamespace::Worker)?;
    install_panic_hook();
    backend_log_info("backend logger initialized");
    Ok(())
}

pub(crate) fn backend_log_info(message: impl AsRef<str>) {
    backend_log(LogLevel::Info, message.as_ref());
}

pub(crate) fn backend_log_error(message: impl AsRef<str>) {
    backend_log(LogLevel::Error, message.as_ref());
}

pub(crate) fn backend_log(level: LogLevel, message: &str) {
    append_log(LogNamespace::Backend, "backend", level, message);
}

pub(crate) fn worker_log_path(task_id: i64) -> PathBuf {
    dated_log_path(LogNamespace::Worker, &format!("task-{task_id}.log"))
}

pub(crate) fn append_worker_log_line(log_path: &Path, line: &str) {
    append_line_to_path(log_path, line);
}

pub(crate) fn cleanup_logs(namespace: LogNamespace) -> anyhow::Result<()> {
    let root = namespace_root(namespace);
    if !root.exists() {
        return Ok(());
    }

    let mut entries = collect_log_files(&root)?;
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(
            (LOG_RETENTION_DAYS as u64) * 24 * 60 * 60,
        ))
        .unwrap_or(UNIX_EPOCH);

    for entry in &entries {
        if entry.modified < cutoff {
            let _ = fs::remove_file(&entry.path);
        }
    }

    entries.retain(|entry| entry.modified >= cutoff && entry.path.exists());
    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    for entry in entries.into_iter().skip(LOG_RETENTION_FILES) {
        let _ = fs::remove_file(entry.path);
    }

    prune_empty_dirs(&root);
    Ok(())
}

fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            backend_log_error(format!("panic: {panic_info}"));
            previous_hook(panic_info);
        }));
    });
}

fn append_log(namespace: LogNamespace, stem: &str, level: LogLevel, message: &str) {
    let now = Local::now();
    let line = format!(
        "{} [{}] {}",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        level.as_str(),
        message
    );
    let path = dated_log_path(namespace, &format!("{stem}.log"));
    append_line_to_path(&path, &line);
}

fn append_line_to_path(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    rotate_if_needed(path);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn dated_log_path(namespace: LogNamespace, file_name: &str) -> PathBuf {
    let now = Local::now();
    namespace_root(namespace)
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()))
        .join(format!("{:02}", now.day()))
        .join(file_name)
}

fn namespace_root(namespace: LogNamespace) -> PathBuf {
    LOG_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("logs"))
        .join(namespace.as_str())
}

fn rotate_if_needed(path: &Path) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() < LOG_MAX_BYTES {
        return;
    }

    let Some(parent) = path.parent() else {
        return;
    };
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let Some((stem, extension)) = file_name.rsplit_once('.') else {
        return;
    };

    let mut rotated = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Some(name) = entry_path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let prefix = format!("{stem}.");
            let suffix = format!(".{extension}");
            if !name.starts_with(&prefix) || !name.ends_with(&suffix) {
                continue;
            }
            let ordinal = &name[prefix.len()..name.len() - suffix.len()];
            if let Ok(index) = ordinal.parse::<usize>() {
                rotated.push((index, entry_path));
            }
        }
    }

    rotated.sort_by(|a, b| b.0.cmp(&a.0));
    for (index, entry_path) in rotated {
        let target = parent.join(format!("{stem}.{}.{extension}", index + 1));
        let _ = fs::rename(entry_path, target);
    }

    let first_rotated = parent.join(format!("{stem}.1.{extension}"));
    let _ = fs::rename(path, first_rotated);
}

#[derive(Debug)]
struct LogFileEntry {
    path: PathBuf,
    modified: SystemTime,
}

fn collect_log_files(root: &Path) -> anyhow::Result<Vec<LogFileEntry>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path().to_path_buf();
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        files.push(LogFileEntry { path, modified });
    }
    Ok(files)
}

fn prune_empty_dirs(root: &Path) {
    let mut dirs: Vec<PathBuf> = WalkDir::new(root)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.path().to_path_buf())
        .collect();
    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        if dir != root {
            let _ = fs::remove_dir(&dir);
        }
    }
}
