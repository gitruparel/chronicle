pub mod types;
pub mod scanner;
pub mod metadata;
pub mod date_resolver;
pub mod organizer;
pub mod copy_engine;
pub mod database;
pub mod jobs;
pub mod reports;
pub mod commands;

use std::path::PathBuf;
use tauri::Manager;
use commands::AppState;
use database::Database;
use jobs::JobManager;
use metadata::ExifToolManager;

pub fn run() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();

    log::info!("Starting Chronicle desktop backend...");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Determine application data directory for SQLite database
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(local).join("Chronicle")
            });

            let db_path = app_data_dir.join("chronicle.db");
            log::info!("Chronicle SQLite database path: {}", db_path.display());

            let db = Database::open(&db_path)
                .expect("Failed to initialize SQLite database for Chronicle");

            let job_manager = JobManager::new(db);
            let exiftool = ExifToolManager::new();

            if exiftool.is_available() {
                log::info!("ExifTool available at: {:?}", exiftool.get_path());
            } else {
                log::warn!("ExifTool not found; will operate with filesystem modified time fallback");
            }

            app.manage(AppState {
                job_manager,
                exiftool,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::validate_path,
            commands::check_folder_overlap_cmd,
            commands::create_and_scan_job,
            commands::get_preview_files,
            commands::get_job_summary_cmd,
            commands::start_copy_job,
            commands::pause_copy_job,
            commands::resume_copy_job,
            commands::cancel_copy_job,
            commands::retry_failed_job,
            commands::export_report,
            commands::open_destination_folder,
            commands::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Chronicle application");
}
