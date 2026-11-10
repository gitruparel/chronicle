use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::copy_engine::{copy_and_verify_file, CopyOptions};
use crate::database::Database;
use crate::date_resolver::resolve_date;
use crate::metadata::ExifToolManager;
use crate::organizer::plan_destinations;
use crate::scanner::{check_overlap, scan_directory};
use crate::types::{FileStatus, JobOptions, JobProgress, JobStatus, JobSummary};

/// Normalizes path representations so that ExifTool's forward-slashed SourceFile paths
/// match Rust PathBuf string representations across Windows and Unix platforms.
pub fn normalize_path_key(path_str: &str) -> String {
    let trimmed = path_str.trim();
    // Strip Windows extended length prefixes
    let stripped = if let Some(unc) = trimmed.strip_prefix(r"\\?\UNC\") {
        format!("//{}", unc)
    } else if let Some(regular) = trimmed.strip_prefix(r"\\?\") {
        regular.to_string()
    } else {
        trimmed.to_string()
    };
    let with_forward_slashes = stripped.replace('\\', "/");
    #[cfg(windows)]
    {
        with_forward_slashes.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        with_forward_slashes
    }
}

pub struct JobManager {
    db: Database,
    cancellation_tokens: Arc<TokioMutex<HashMap<String, Arc<AtomicBool>>>>,
    pause_flags: Arc<TokioMutex<HashMap<String, Arc<AtomicBool>>>>,
    active_jobs: Arc<TokioMutex<HashSet<String>>>,
}

impl JobManager {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            cancellation_tokens: Arc::new(TokioMutex::new(HashMap::new())),
            pause_flags: Arc::new(TokioMutex::new(HashMap::new())),
            active_jobs: Arc::new(TokioMutex::new(HashSet::new())),
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
    /// and plans collision-safe destinations. Captures all scan errors without omission.
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

        // 1. Recursive scan (captures files AND traversal errors)
        let scan_result = scan_directory(source, options.recursive)?;

        self.db.update_job_status(job_id, JobStatus::Analysing)
            .map_err(|e| e.to_string())?;

        // 2. Batched ExifTool extraction
        let batch_size = 50;
        let mut resolved_files = Vec::with_capacity(scan_result.files.len());

        for chunk in scan_result.files.chunks(batch_size) {
            let paths: Vec<&Path> = chunk.iter().map(|f| f.source_path.as_path()).collect();
            let (metadata_map, raw_arr) = if exiftool.is_available() {
                match exiftool.extract_batch(&paths) {
                    Ok(arr) => {
                        let mut map = HashMap::new();
                        for item in &arr {
                            if let Some(src) = item.get("SourceFile").and_then(|v| v.as_str()) {
                                map.insert(normalize_path_key(src), item.clone());
                            }
                        }
                        (map, arr)
                    }
                    Err(e) => {
                        log::warn!("Batch metadata extraction warning: {}", e);
                        (HashMap::new(), Vec::new())
                    }
                }
            } else {
                (HashMap::new(), Vec::new())
            };

            for (idx, file) in chunk.iter().enumerate() {
                let path_str = file.source_path.to_string_lossy().to_string();
                let key = normalize_path_key(&path_str);
                // 1. Direct normalized lookup
                // 2. Aligned positional fallback if batch counts match
                let meta = metadata_map.get(&key)
                    .or_else(|| {
                        if raw_arr.len() == chunk.len() {
                            raw_arr.get(idx)
                        } else {
                            None
                        }
                    });

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

        // 4. Save to database including any traversal/scan errors and policy-excluded items
        self.db.save_planned_files(job_id, &planned_files, &scan_result.errors, &scan_result.excluded)
            .map_err(|e| format!("Database save error: {}", e))?;

        self.db.update_job_status(job_id, JobStatus::Analysed)
            .map_err(|e| e.to_string())?;

        // 5. Compute summary
        let summary = self.db.get_job_summary(job_id)
            .map_err(|e| e.to_string())?;

        Ok(summary)
    }

    /// Starts streaming copy execution using a bounded worker pool.
    /// Idempotent: rejects duplicate execution attempts for the same job.
    pub async fn start_execution(
        &self,
        app_handle: AppHandle,
        job_id: String,
        options: JobOptions,
    ) -> Result<(), String> {
        // Enforce idempotency: prevent starting the same job twice concurrently
        {
            let mut active = self.active_jobs.lock().await;
            if active.contains(&job_id) {
                log::warn!("Job {} is already running; ignoring duplicate start request.", job_id);
                return Ok(());
            }
            active.insert(job_id.clone());
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));

        {
            let mut tokens = self.cancellation_tokens.lock().await;
            tokens.insert(job_id.clone(), cancel_token.clone());
            let mut pauses = self.pause_flags.lock().await;
            pauses.insert(job_id.clone(), pause_flag.clone());
        }

        // Reconcile disk state before resuming/starting
        let _ = self.db.reconcile_job_with_disk(&job_id);

        self.db.update_job_status(&job_id, JobStatus::Running)
            .map_err(|e| e.to_string())?;

        let db = self.db.clone();
        let active_set = self.active_jobs.clone();
        let copy_opts = CopyOptions {
            verify_hash: options.verify_hash,
            buffer_size: 64 * 1024,
        };

        let files = db.get_planned_files(&job_id, None, 0)
            .map_err(|e| e.to_string())?;

        let total_files = files.len() as u64;
        let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
        let excluded_count = files.iter().filter(|f| f.status == FileStatus::Excluded).count() as u64;

        // Concurrency limiter: bounded worker pool
        let worker_limit = options.worker_count.clamp(1, 16);
        let semaphore = Arc::new(Semaphore::new(worker_limit));

        tokio::spawn(async move {
            let copied_files = Arc::new(AtomicU64::new(0));
            let copied_bytes = Arc::new(AtomicU64::new(0));
            let failed_files = Arc::new(AtomicU64::new(0));

            // Populate already verified files from database
            for f in &files {
                if f.status == FileStatus::Verified {
                    copied_files.fetch_add(1, Ordering::SeqCst);
                    copied_bytes.fetch_add(f.size_bytes, Ordering::SeqCst);
                } else if f.status == FileStatus::Failed {
                    failed_files.fetch_add(1, Ordering::SeqCst);
                }
            }

            let mut tasks = Vec::new();

            for file in files {
                // If file is already verified or policy-excluded, do not copy
                if file.status == FileStatus::Verified || file.status == FileStatus::Excluded {
                    continue;
                }

                // Check cancellation
                if cancel_token.load(Ordering::SeqCst) {
                    break;
                }

                // Check pause
                while pause_flag.load(Ordering::SeqCst) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                    if cancel_token.load(Ordering::SeqCst) {
                        break;
                    }
                }

                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let c_db = db.clone();
                let c_job_id = job_id.clone();
                let c_opts = copy_opts.clone();
                let c_cancel = cancel_token.clone();
                let c_pause = pause_flag.clone();
                let c_app = app_handle.clone();
                let c_copied_f = copied_files.clone();
                let c_copied_b = copied_bytes.clone();
                let c_failed_f = failed_files.clone();

                let task = tokio::spawn(async move {
                    let _permit = permit; // holds permit until task finishes

                    if c_cancel.load(Ordering::SeqCst) {
                        return;
                    }

                    let _ = c_db.update_file_status(&file.id, FileStatus::Copying, None, None);

                    let _ = c_app.emit("job-progress", JobProgress {
                        job_id: c_job_id.clone(),
                        status: JobStatus::Running,
                        total_files,
                        total_bytes,
                        copied_files: c_copied_f.load(Ordering::SeqCst),
                        copied_bytes: c_copied_b.load(Ordering::SeqCst),
                        failed_files: c_failed_f.load(Ordering::SeqCst),
                        excluded_files: excluded_count,
                        current_file: Some(file.filename.clone()),
                        current_operation: format!("Copying {} to {}", file.filename, file.destination_folder),
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                    });

                    let src = file.source_path.clone();
                    let dst = file.destination_path.clone();
                    let opts = c_opts.clone();
                    let c_flag = c_cancel.clone();
                    let p_flag = c_pause.clone();

                    let res = tokio::task::spawn_blocking(move || {
                        copy_and_verify_file(&src, &dst, &opts, Some(&c_flag), Some(&p_flag), |_| {})
                    }).await;

                    match res {
                        Ok(copy_res) if copy_res.success => {
                            let _ = c_db.update_file_status(
                                &file.id,
                                FileStatus::Verified,
                                copy_res.sha256.as_deref(),
                                None,
                            );
                            c_copied_f.fetch_add(1, Ordering::SeqCst);
                            c_copied_b.fetch_add(copy_res.bytes_copied, Ordering::SeqCst);
                        }
                        Ok(copy_res) => {
                            let err_msg = copy_res.error.unwrap_or_else(|| "Copy error".to_string());
                            let _ = c_db.update_file_status(&file.id, FileStatus::Failed, None, Some(&err_msg));
                            c_failed_f.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(join_err) => {
                            let err_msg = format!("Task failed: {}", join_err);
                            let _ = c_db.update_file_status(&file.id, FileStatus::Failed, None, Some(&err_msg));
                            c_failed_f.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                });

                tasks.push(task);
            }

            // Wait for all worker tasks to complete
            for t in tasks {
                let _ = t.await;
            }

            // Terminal status reconciliation
            let final_failed = failed_files.load(Ordering::SeqCst);
            let final_copied = copied_files.load(Ordering::SeqCst);
            let final_status = if cancel_token.load(Ordering::SeqCst) {
                JobStatus::Cancelled
            } else if final_failed > 0 {
                JobStatus::Failed
            } else {
                JobStatus::Completed
            };

            let _ = db.update_job_status(&job_id, final_status);

            // Re-sync aggregate counters
            let _ = db.reconcile_job_with_disk(&job_id);

            // Clear from active set
            {
                let mut active = active_set.lock().await;
                active.remove(&job_id);
            }

            let _ = app_handle.emit("job-progress", JobProgress {
                job_id: job_id.clone(),
                status: final_status,
                total_files,
                total_bytes,
                copied_files: final_copied,
                copied_bytes: copied_bytes.load(Ordering::SeqCst),
                failed_files: final_failed,
                excluded_files: excluded_count,
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
