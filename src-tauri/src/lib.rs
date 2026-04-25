pub(crate) use std::collections::{HashMap, HashSet};
pub(crate) use std::fs::{self, OpenOptions};
pub(crate) use std::io::{BufRead, BufReader, Write};
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::{Command, Stdio};
pub(crate) use std::sync::Arc;
pub(crate) use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) use anyhow::{Context, anyhow};
pub(crate) use axum::Router;
pub(crate) use axum::body::Body;
pub(crate) use axum::extract::{Path as AxumPath, Request, State as AxumState};
pub(crate) use axum::http::StatusCode;
pub(crate) use axum::response::{IntoResponse, Response};
pub(crate) use axum::routing::any;
pub(crate) use base64::Engine;
pub(crate) use chrono::Local;
pub(crate) use parking_lot::{Mutex, RwLock};
pub(crate) use rusqlite::{Connection, OptionalExtension, params};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::{Value, json};
pub(crate) use sha2::{Digest, Sha256};
pub(crate) use sysinfo::System;
pub(crate) use tauri::{AppHandle, Emitter, Manager, State};
pub(crate) use tower::ServiceExt;
pub(crate) use tower_http::services::ServeFile;
pub(crate) use uuid::Uuid;
pub(crate) use walkdir::WalkDir;

#[cfg(unix)]
pub(crate) use std::os::unix::fs::PermissionsExt;

mod commands;
mod db;
mod domain;
mod logging;
mod media_server;
mod services;
mod settings;
mod state;

pub(crate) use db::*;
pub(crate) use domain::*;
pub(crate) use logging::*;
pub(crate) use media_server::*;
pub(crate) use services::*;
pub(crate) use settings::*;
pub(crate) use state::*;

pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const LIBRARY_MARKER_FILENAME: &str = "library.json";
pub(crate) const MEDIA_EVENT_TTL_MINUTES: u64 = 10;

pub(crate) type CommandResult<T> = Result<T, String>;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn suppress_command_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = DesktopState::bootstrap(&app.handle()).map_err(
                |error| -> Box<dyn std::error::Error> {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        error.to_string(),
                    ))
                },
            )?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_state,
            commands::app::get_resource_state,
            commands::app::request_app_exit,
            commands::library::initialize_media_library,
            commands::library::select_existing_media_library,
            commands::library::migrate_media_library,
            commands::tasks::import_video_task,
            commands::tasks::import_video_tasks,
            commands::tasks::list_tasks,
            commands::tasks::get_task,
            commands::tasks::get_task_status,
            commands::tasks::get_task_result,
            commands::tasks::start_task,
            commands::tasks::dequeue_task,
            commands::tasks::reanalyze_task,
            commands::tasks::delete_task,
            commands::tasks::delete_tasks,
            commands::tasks::get_video_stream_url,
            commands::report::export_report_file,
            commands::scheduler::update_scheduler_settings,
            commands::scheduler::get_queue_recovery_state,
            commands::scheduler::resolve_queue_recovery,
            commands::runtime::get_runtime_state,
            commands::runtime::import_runtime_zip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
