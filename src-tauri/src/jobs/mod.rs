use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::copy_engine::{copy_and_verify_file, CopyOptions};
use crate::database::Database;
use crate::date_resolver::resolve_date;
use crate::metadata::ExifToolManager;
use crate::organizer::plan_destinations;
use crate::scanner::{check_overlap, scan_directory};
use crate::types::{FileStatus, JobOptions, JobProgress, JobStatus, JobSummary};

pub struct JobManager {
    db: Database,
    cancellation_tokens: Arc<TokioMutex<HashMap<String, Arc<AtomicBool>>>>,
    pause_flags: Arc<TokioMutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl JobManager {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            cancellation_tokens: Arc::new(TokioMutex::new(HashMap::new())),
            pause_flags: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    pub fn get_db(&self) -> &Database {
        &self.db
    }

    /// Creates and persists a new job after verifying path safety
    pub fn create_job(
        &self,
        name: &str,
        source: &Path,
        destination: &Path,
        options: JobOptions,
    ) -> Result<String, String> {
        let overlap = check_overlap(source, destination);
        if !overlap.is_safe {
            return Err(overlap.error_message.unwrap_or_else(|| "Unsafe path overlap detected".to_string()));
        }

        let job_id = Uuid::new_v4().to_string();
        self.db.create_job(&job_id, name, source, destination, &options)
            .map_err(|e| format!("Failed to create job in database: {}", e))?;

        Ok(job_id)
    }

    /// Scans source directory, extracts metadata via ExifTool batches, resolves dates,
    /// and plans collision-safe destinations.
    pub async fn run_scan_and_plan(
        &self,
        job_id: &str,
        source: &Path,
        destination: &Path,
        options: &JobOptions,
        exiftool: &ExifToolManager,
    ) -> Result<JobSummary, String> {
        self.db.update_job_status(job_id, JobStatus::Scanning)
            .map_err(|e| e.to_string())?;

        // 1. Recursive scan
        let scanned_files = scan_directory(source, options.recursive)?;

        self.db.update_job_status(job_id, JobStatus::Analysing)
            .map_err(|e| e.to_string())?;

        // 2. Batched ExifTool extraction
        let batch_size = 50;
        let mut resolved_files = Vec::with_capacity(scanned_files.len());

        for chunk in scanned_files.chunks(batch_size) {
            let paths: Vec<&Path> = chunk.iter().map(|f| f.source_path.as_path()).collect();
            let metadata_map = if exiftool.is_available() {
                match exiftool.extract_batch(&paths) {
                    Ok(arr) => {
                        let mut map = HashMap::new();
                        for item in arr {
                            if let Some(src) = item.get("SourceFile").and_then(|v| v.as_str()) {
                                map.insert(src.to_string(), item);
                            }
                        }
                        map
                    }
                    Err(e) => {
                        log::warn!("Batch metadata extraction warning: {}", e);
                        HashMap::new()
                    }
                }
            } else {
                HashMap::new()
            };

            for file in chunk {
                let path_str = file.source_path.to_string_lossy().to_string();
                let meta = metadata_map.get(&path_str);
                let date_res = resolve_date(
                    &file.source_path,
                    file.extension.as_deref(),
                    meta,
                    file.modified_ms,
                    options.mtime_fallback,
                );
                resolved_files.push((file.clone(), date_res));
            }
        }

        // 3. Plan destinations and resolve collisions
        let planned_files = plan_destinations(destination, resolved_files);

        // 4. Save to database
        self.db.save_planned_files(job_id, &planned_files)
            .map_err(|e| format!("Database save error: {}", e))?;

        self.db.update_job_status(job_id, JobStatus::Analysed)
            .map_err(|e| e.to_string())?;

        // 5. Compute summary
        let summary = self.db.get_job_summary(job_id)
            .map_err(|e| e.to_string())?;

        Ok(summary)
    }

    /// Starts streaming copy execution for a job
    pub async fn start_execution(
        &self,
        app_handle: AppHandle,
        job_id: String,
        options: JobOptions,
    ) -> Result<(), String> {
        let cancel_token = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));

        {
            let mut tokens = self.cancellation_tokens.lock().await;
            tokens.insert(job_id.clone(), cancel_token.clone());
            let mut pauses = self.pause_flags.lock().await;
            pauses.insert(job_id.clone(), pause_flag.clone());
        }

        self.db.update_job_status(&job_id, JobStatus::Running)
            .map_err(|e| e.to_string())?;

        let db = self.db.clone();
        let copy_opts = CopyOptions {
            verify_hash: options.verify_hash,
            buffer_size: 64 * 1024,
        };

        let files = db.get_planned_files(&job_id, None, 0)
            .map_err(|e| e.to_string())?;

        let total_files = files.len() as u64;
        let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();

        tokio::spawn(async move {
            let mut copied_files: u64 = 0;
            let mut copied_bytes: u64 = 0;
            let mut failed_files: u64 = 0;

            for file in files {
                // If already verified, count and skip
                if file.status == FileStatus::Verified {
                    copied_files += 1;
                    copied_bytes += file.size_bytes;
                    continue;
                }

                // Check cancellation
                if cancel_token.load(Ordering::SeqCst) {
                    let _ = db.update_job_status(&job_id, JobStatus::Cancelled);
                    let _ = app_handle.emit("job-progress", JobProgress {
                        job_id: job_id.clone(),
                        status: JobStatus::Cancelled,
                        total_files,
                        total_bytes,
                        copied_files,
                        copied_bytes,
                        failed_files,
                        current_file: None,
                        current_operation: "Job cancelled".to_string(),
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                    });
                    return;
                }

                // Check pause
                while pause_flag.load(Ordering::SeqCst) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    if cancel_token.load(Ordering::SeqCst) {
                        let _ = db.update_job_status(&job_id, JobStatus::Cancelled);
                        return;
                    }
                }

                let _ = db.update_file_status(&file.id, FileStatus::Copying, None, None);

                // Emit progress event
                let _ = app_handle.emit("job-progress", JobProgress {
                    job_id: job_id.clone(),
                    status: JobStatus::Running,
                    total_files,
                    total_bytes,
                    copied_files,
                    copied_bytes,
                    failed_files,
                    current_file: Some(file.filename.clone()),
                    current_operation: format!("Copying {} to {}", file.filename, file.destination_folder),
                    speed_bytes_per_sec: 0,
                    eta_seconds: None,
                });

                let src = file.source_path.clone();
                let dst = file.destination_path.clone();
                let opts = copy_opts.clone();
                let c_flag = cancel_token.clone();

                // Run copy in blocking thread pool
                let res = tokio::task::spawn_blocking(move || {
                    copy_and_verify_file(&src, &dst, &opts, Some(&c_flag), |_| {})
                }).await;

                match res {
                    Ok(copy_res) if copy_res.success => {
                        let _ = db.update_file_status(
                            &file.id,
                            FileStatus::Verified,
                            copy_res.sha256.as_deref(),
                            None,
                        );
                        let _ = db.increment_job_progress(&job_id, copy_res.bytes_copied, true);
                        copied_files += 1;
                        copied_bytes += copy_res.bytes_copied;
                    }
                    Ok(copy_res) => {
                        let err_msg = copy_res.error.unwrap_or_else(|| "Unknown copy error".to_string());
                        let _ = db.update_file_status(&file.id, FileStatus::Failed, None, Some(&err_msg));
                        let _ = db.increment_job_progress(&job_id, 0, false);
                        failed_files += 1;
                    }
                    Err(join_err) => {
                        let err_msg = format!("Task execution error: {}", join_err);
                        let _ = db.update_file_status(&file.id, FileStatus::Failed, None, Some(&err_msg));
                        let _ = db.increment_job_progress(&job_id, 0, false);
                        failed_files += 1;
                    }
                }
            }

            // Terminal status check
            let final_status = if cancel_token.load(Ordering::SeqCst) {
                JobStatus::Cancelled
            } else if failed_files > 0 {
                JobStatus::Failed
            } else {
                JobStatus::Completed
            };

            let _ = db.update_job_status(&job_id, final_status);

            let _ = app_handle.emit("job-progress", JobProgress {
                job_id: job_id.clone(),
                status: final_status,
                total_files,
                total_bytes,
                copied_files,
                copied_bytes,
                failed_files,
                current_file: None,
                current_operation: format!("Job finished with status: {:?}", final_status),
                speed_bytes_per_sec: 0,
                eta_seconds: Some(0),
            });
        });

        Ok(())
    }

    /// Signals cancellation of a running job
    pub async fn cancel_job(&self, job_id: &str) -> bool {
        let tokens = self.cancellation_tokens.lock().await;
        if let Some(token) = tokens.get(job_id) {
            token.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// Pauses a running job
    pub async fn pause_job(&self, job_id: &str) -> bool {
        let pauses = self.pause_flags.lock().await;
        if let Some(pause) = pauses.get(job_id) {
            pause.store(true, Ordering::SeqCst);
            let _ = self.db.update_job_status(job_id, JobStatus::Paused);
            true
        } else {
            false
        }
    }

    /// Resumes a paused job
    pub async fn resume_job(&self, job_id: &str) -> bool {
        let pauses = self.pause_flags.lock().await;
        if let Some(pause) = pauses.get(job_id) {
            pause.store(false, Ordering::SeqCst);
            let _ = self.db.update_job_status(job_id, JobStatus::Running);
            true
        } else {
            false
        }
    }
}
