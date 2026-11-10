use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use serde::{Deserialize, Serialize};

use crate::jobs::JobManager;
use crate::metadata::ExifToolManager;
use crate::reports::ReportGenerator;
use crate::scanner::check_overlap;
use crate::types::{JobOptions, JobRecord, JobSummary, OverlapCheckResult, PathValidationResult, PlannedFile};

pub struct AppState {
    pub job_manager: JobManager,
    pub exiftool: ExifToolManager,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJobResponse {
    pub job_id: String,
    pub summary: JobSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedFilesResponse {
    pub files: Vec<PlannedFile>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfoResponse {
    pub version: String,
    pub exiftool_available: bool,
    pub exiftool_path: Option<String>,
}

#[tauri::command]
pub fn validate_path(path: String) -> PathValidationResult {
    let p = Path::new(&path);
    if !p.exists() {
        return PathValidationResult {
            is_valid: false,
            canonical_path: None,
            exists: false,
            is_directory: false,
            available_bytes: None,
            error_message: Some("Path does not exist".to_string()),
        };
    }

    let is_dir = p.is_dir();
    let canonical = dunce::canonicalize(p)
        .map(|c| c.to_string_lossy().to_string())
        .ok();

    PathValidationResult {
        is_valid: is_dir,
        canonical_path: canonical,
        exists: true,
        is_directory: is_dir,
        available_bytes: None,
        error_message: if is_dir { None } else { Some("Path is not a directory".to_string()) },
    }
}

#[tauri::command]
pub fn check_folder_overlap_cmd(source: String, destination: String) -> OverlapCheckResult {
    check_overlap(Path::new(&source), Path::new(&destination))
}

#[tauri::command]
pub async fn create_and_scan_job(
    state: State<'_, AppState>,
    name: String,
    source_path: String,
    destination_path: String,
    options: JobOptions,
) -> Result<CreateJobResponse, String> {
    let src = PathBuf::from(&source_path);
    let dst = PathBuf::from(&destination_path);

    let job_id = state.job_manager.create_job(&name, &src, &dst, options.clone())?;

    let summary = state.job_manager.run_scan_and_plan(
        &job_id,
        &src,
        &dst,
        &options,
        &state.exiftool,
    ).await?;

    Ok(CreateJobResponse { job_id, summary })
}

#[tauri::command]
pub fn get_preview_files(
    state: State<'_, AppState>,
    job_id: String,
    page: usize,
    page_size: usize,
    filter: Option<String>,
    search: Option<String>,
) -> Result<PaginatedFilesResponse, String> {
    let offset = page * page_size;
    let (files, total) = state.job_manager.get_db().get_preview_files_filtered(
        &job_id,
        page_size,
        offset,
        filter.as_deref(),
        search.as_deref(),
    ).map_err(|e| e.to_string())?;

    Ok(PaginatedFilesResponse {
        files,
        total,
        page,
        page_size,
    })
}

#[tauri::command]
pub fn get_job_summary_cmd(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<JobSummary, String> {
    state.job_manager.get_db().get_job_summary(&job_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_jobs_cmd(
    state: State<'_, AppState>,
) -> Result<Vec<JobRecord>, String> {
    state.job_manager.get_db().get_all_jobs()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_copy_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    options: JobOptions,
) -> Result<(), String> {
    state.job_manager.start_execution(app, job_id, options).await
}

#[tauri::command]
pub async fn pause_copy_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<bool, String> {
    Ok(state.job_manager.pause_job(&job_id).await)
}

#[tauri::command]
pub async fn resume_copy_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<bool, String> {
    Ok(state.job_manager.resume_job(&job_id).await)
}

#[tauri::command]
pub async fn cancel_copy_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<bool, String> {
    Ok(state.job_manager.cancel_job(&job_id).await)
}

#[tauri::command]
pub async fn retry_failed_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    options: JobOptions,
) -> Result<usize, String> {
    let count = state.job_manager.get_db().reset_failed_files_for_retry(&job_id)
        .map_err(|e| e.to_string())?;

    // Restart execution for pending files
    state.job_manager.start_execution(app, job_id, options).await?;
    Ok(count)
}

#[tauri::command]
pub fn export_report(
    state: State<'_, AppState>,
    job_id: String,
    format: String,
    output_path: String,
) -> Result<(), String> {
    let out = Path::new(&output_path);
    if format.to_lowercase() == "csv" {
        ReportGenerator::export_csv(state.job_manager.get_db(), &job_id, out)
    } else {
        ReportGenerator::export_json(state.job_manager.get_db(), &job_id, out)
    }
}

#[tauri::command]
pub fn open_destination_folder(path: String) -> Result<(), String> {
    ReportGenerator::open_in_explorer(Path::new(&path))
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfoResponse {
    AppInfoResponse {
        version: "1.0.0".to_string(),
        exiftool_available: state.exiftool.is_available(),
        exiftool_path: state.exiftool.get_path().map(|p| p.to_string_lossy().to_string()),
    }
}
