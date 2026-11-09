use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Created,
    Scanning,
    Scanned,
    Analysing,
    Analysed,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Discovered,
    Analysed,
    Pending,
    Copying,
    Verified,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSource {
    ExifOriginal,
    QuickTimeCreation,
    EmbeddedDocAudio,
    EmbeddedOther,
    FilesystemModified,
    None,
}

impl std::fmt::Display for DateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DateSource::ExifOriginal => write!(f, "EXIF Original Capture"),
            DateSource::QuickTimeCreation => write!(f, "QuickTime Creation Time"),
            DateSource::EmbeddedDocAudio => write!(f, "Embedded Media Metadata"),
            DateSource::EmbeddedOther => write!(f, "Secondary Embedded Metadata"),
            DateSource::FilesystemModified => write!(f, "Filesystem Modified Time"),
            DateSource::None => write!(f, "No Date (Fallback)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateResolution {
    /// Resolved calendar date in YYYY-MM-DD or None for No Date
    pub resolved_date: Option<String>,
    pub date_source: DateSource,
    pub raw_timestamp: Option<String>,
    pub confidence: ConfidenceLevel,
    pub reason: String,
    pub timezone_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub id: String,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub filename: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub modified_ms: Option<i64>,
    pub is_readonly: bool,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanErrorItem {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub files: Vec<ScannedFile>,
    pub errors: Vec<ScanErrorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedFile {
    pub id: String,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub filename: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub modified_ms: Option<i64>,
    pub date_resolution: DateResolution,
    pub destination_folder: String, // "2024-05-18" or "No Date"
    pub planned_filename: String,   // e.g. "IMG_0001 (1).JPG"
    pub destination_path: PathBuf,
    pub is_collision: bool,
    pub collision_suffix: Option<u32>,
    pub status: FileStatus,
    pub sha256: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOptions {
    pub recursive: bool,
    pub copy_duplicates: bool,
    pub mtime_fallback: bool,
    pub verify_hash: bool,
    pub worker_count: usize,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            copy_duplicates: true,
            mtime_fallback: true,
            verify_hash: true,
            worker_count: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub total_files: u64,
    pub total_bytes: u64,
    pub embedded_date_count: u64,
    pub mtime_date_count: u64,
    pub no_date_count: u64,
    pub metadata_error_count: u64,
    pub duplicate_content_count: u64,
    pub unique_folders_count: u64,
    pub scan_error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub job_id: String,
    pub status: JobStatus,
    pub total_files: u64,
    pub total_bytes: u64,
    pub copied_files: u64,
    pub copied_bytes: u64,
    pub failed_files: u64,
    pub current_file: Option<String>,
    pub current_operation: String,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub destination_path: String,
    pub status: JobStatus,
    pub total_files: u64,
    pub total_bytes: u64,
    pub copied_files: u64,
    pub copied_bytes: u64,
    pub failed_files: u64,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationResult {
    pub is_valid: bool,
    pub canonical_path: Option<String>,
    pub exists: bool,
    pub is_directory: bool,
    pub available_bytes: Option<u64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlapCheckResult {
    pub is_safe: bool,
    pub error_message: Option<String>,
}
