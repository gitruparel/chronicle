use rusqlite::{params, Connection, Result as SqlResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use chrono::Utc;
use crate::copy_engine::hash_file_on_disk;
use crate::types::{
    ConfidenceLevel, DateResolution, DateSource, ExcludedItem, FileStatus, JobOptions, JobRecord, JobStatus,
    JobSummary, PlannedFile, ScanErrorItem,
};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    _db_path: PathBuf,
}

impl Database {
    /// Opens or creates the SQLite database at the given path, enabling WAL mode and applying migrations.
    pub fn open(path: &Path) -> SqlResult<Self> {
        let (conn, actual_path) = match (|| -> SqlResult<(Connection, PathBuf)> {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let c = Connection::open(path)?;
            Ok((c, path.to_path_buf()))
        })() {
            Ok(pair) => pair,
            Err(e) => {
                log::warn!("Failed to open DB at {}: {}. Falling back to local chronicle.db", path.display(), e);
                let fallback = PathBuf::from("chronicle.db");
                let c = Connection::open(&fallback)?;
                (c, fallback)
            }
        };

        // Performance & concurrency pragmas
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            _db_path: actual_path,
        };

        db.migrate()?;
        db.reconcile_on_startup()?;

        Ok(db)
    }

    /// Creates an in-memory database (useful for unit tests)
    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            _db_path: PathBuf::from(":memory:"),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                name TEXT,
                source_path TEXT NOT NULL,
                destination_path TEXT NOT NULL,
                status TEXT NOT NULL,
                total_files INTEGER DEFAULT 0,
                total_bytes INTEGER DEFAULT 0,
                copied_files INTEGER DEFAULT 0,
                copied_bytes INTEGER DEFAULT 0,
                failed_files INTEGER DEFAULT 0,
                excluded_files INTEGER DEFAULT 0,
                options_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS job_files (
                id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                source_path TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                filename TEXT NOT NULL,
                extension TEXT,
                file_size INTEGER NOT NULL,
                source_mtime INTEGER,
                status TEXT NOT NULL,
                resolved_date TEXT,
                date_source TEXT,
                raw_timestamp TEXT,
                confidence TEXT,
                reason TEXT,
                timezone_info TEXT,
                destination_folder TEXT,
                planned_filename TEXT,
                destination_path TEXT,
                is_collision INTEGER DEFAULT 0,
                collision_suffix INTEGER,
                sha256 TEXT,
                error_message TEXT,
                attempt_count INTEGER DEFAULT 0,
                updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_job_files_status ON job_files(job_id, status);
            CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at DESC);
            "#,
        )?;

        // Ensure backward compatibility if database already existed
        let _ = conn.execute("ALTER TABLE jobs ADD COLUMN excluded_files INTEGER DEFAULT 0", []);

        Ok(())
    }

    /// Reconciles database records on startup to handle unexpected shutdowns:
    /// Resets any running/scanning/analysing jobs to 'paused'
    fn reconcile_on_startup(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE jobs SET status = 'paused' WHERE status IN ('running', 'scanning', 'analysing')",
            [],
        )?;

        Ok(())
    }

    /// Rigorous disk reconciliation for a specific job:
    /// - Cleans up orphaned .chronicle_tmp_* files in destination directories.
    /// - For 'copying' files, self-heals files completed before the process died, or reverts to pending.
    /// - Never trusts a 'verified' database record without verifying actual destination on disk.
    /// - Quarantines corrupted destination files to ensure they never masquerade as valid and frees the planned path.
    pub fn reconcile_job_with_disk(&self, job_id: &str) -> SqlResult<()> {
        let (dst_root_str,): (String,) = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT destination_path FROM jobs WHERE id = ?1",
                params![job_id],
                |row| Ok((row.get(0)?,)),
            )?
        };
        let dst_root = PathBuf::from(dst_root_str);

        // 1. Clean up any lingering .chronicle_tmp_* files in destination root and date subdirectories
        if dst_root.exists() {
            if let Ok(entries) = fs::read_dir(&dst_root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                            if fname.starts_with(".chronicle_tmp_") {
                                log::info!("Recovery: Cleaning orphaned staging file: {}", path.display());
                                let _ = fs::remove_file(&path);
                            }
                        }
                    } else if path.is_dir() {
                        if let Ok(sub_entries) = fs::read_dir(&path) {
                            for sub in sub_entries.flatten() {
                                let sub_path = sub.path();
                                if let Some(fname) = sub_path.file_name().and_then(|n| n.to_str()) {
                                    if fname.starts_with(".chronicle_tmp_") {
                                        log::info!("Recovery: Cleaning orphaned staging file: {}", sub_path.display());
                                        let _ = fs::remove_file(&sub_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let files = self.get_planned_files(job_id, None, 0)?;

        for file in files {
            if file.status == FileStatus::Verified {
                let mut is_valid = false;
                if file.destination_path.exists() {
                    if let Ok(meta) = fs::metadata(&file.destination_path) {
                        if meta.len() == file.size_bytes {
                            if let Some(expected_hash) = &file.sha256 {
                                if let Ok(actual_hash) = hash_file_on_disk(&file.destination_path, 64 * 1024) {
                                    if &actual_hash == expected_hash {
                                        is_valid = true;
                                    }
                                }
                            } else {
                                is_valid = true;
                            }
                        }
                    }
                }

                if !is_valid {
                    log::warn!(
                        "Reconciliation: File {} failed destination disk check. Quarantining corrupted output and reverting to pending.",
                        file.destination_path.display()
                    );
                    if file.destination_path.exists() {
                        let quarantine_name = format!(".chronicle_corrupted_{}_{}", file.id, file.planned_filename);
                        if let Some(parent) = file.destination_path.parent() {
                            let quarantine_path = parent.join(quarantine_name);
                            let _ = fs::rename(&file.destination_path, &quarantine_path);
                        }
                    }
                    self.update_file_status(
                        &file.id,
                        FileStatus::Pending,
                        None,
                        Some("Destination file missing or corrupted after interruption; quarantined and reverted to pending"),
                    )?;
                }
            } else if file.status == FileStatus::Copying {
                // Check if file copy completed atomically before process died (Stage 3 self-healing)
                let mut self_healed = false;
                if file.destination_path.exists() {
                    if let Ok(meta) = fs::metadata(&file.destination_path) {
                        if meta.len() == file.size_bytes {
                            if let Ok(dst_hash) = hash_file_on_disk(&file.destination_path, 64 * 1024) {
                                if let Ok(src_hash) = hash_file_on_disk(&file.source_path, 64 * 1024) {
                                    if dst_hash == src_hash {
                                        log::info!(
                                            "Reconciliation self-healing: File {} was completed before crash. Promoting to verified.",
                                            file.destination_path.display()
                                        );
                                        self.update_file_status(
                                            &file.id,
                                            FileStatus::Verified,
                                            Some(&dst_hash),
                                            None,
                                        )?;
                                        self_healed = true;
                                    }
                                }
                            }
                        }
                    }
                    if !self_healed {
                        let quarantine_name = format!(".chronicle_corrupted_{}_{}", file.id, file.planned_filename);
                        if let Some(parent) = file.destination_path.parent() {
                            let quarantine_path = parent.join(quarantine_name);
                            let _ = fs::rename(&file.destination_path, &quarantine_path);
                        }
                    }
                }

                if !self_healed {
                    self.update_file_status(&file.id, FileStatus::Pending, None, None)?;
                }
            }
        }

        // Recompute job aggregate counters from reconciled file states
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE jobs
            SET copied_files = (SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND status = 'verified'),
                copied_bytes = (SELECT COALESCE(SUM(file_size), 0) FROM job_files WHERE job_id = ?1 AND status = 'verified'),
                failed_files = (SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND status = 'failed'),
                excluded_files = (SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND status = 'excluded')
            WHERE id = ?1
            "#,
            params![job_id],
        )?;

        Ok(())
    }

    /// Creates a new job record in the database
    pub fn create_job(
        &self,
        job_id: &str,
        name: &str,
        source_path: &Path,
        destination_path: &Path,
        options: &JobOptions,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let options_json = serde_json::to_string(options).unwrap_or_default();

        conn.execute(
            r#"
            INSERT INTO jobs (id, name, source_path, destination_path, status, options_json, created_at)
            VALUES (?1, ?2, ?3, ?4, 'created', ?5, ?6)
            "#,
            params![
                job_id,
                name,
                source_path.to_string_lossy(),
                destination_path.to_string_lossy(),
                options_json,
                now
            ],
        )?;

        Ok(())
    }

    /// Saves the planned files, scan errors, AND policy-excluded items for a job inside a single transaction.
    /// Guarantees that every discovered entry is permanently recorded and accounted for.
    pub fn save_planned_files(
        &self,
        job_id: &str,
        files: &[PlannedFile],
        scan_errors: &[ScanErrorItem],
        excluded: &[ExcludedItem],
    ) -> SqlResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().timestamp_millis();

        let mut total_bytes: u64 = 0;
        let mut total_files = files.len() as u64;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO job_files (
                    id, job_id, source_path, relative_path, filename, extension,
                    file_size, source_mtime, status, resolved_date, date_source,
                    raw_timestamp, confidence, reason, timezone_info, destination_folder,
                    planned_filename, destination_path, is_collision, collision_suffix,
                    sha256, error_message, attempt_count, updated_at
                )
                VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24
                )
                "#,
            )?;

            // 1. Insert planned files
            for f in files {
                total_bytes += f.size_bytes;

                let status_str = match f.status {
                    FileStatus::Discovered => "discovered",
                    FileStatus::Analysed => "analysed",
                    FileStatus::Pending => "pending",
                    FileStatus::Copying => "copying",
                    FileStatus::Verified => "verified",
                    FileStatus::Failed => "failed",
                    FileStatus::Skipped => "skipped",
                    FileStatus::Excluded => "excluded",
                };

                let date_source_str = match f.date_resolution.date_source {
                    DateSource::ExifOriginal => "exif_original",
                    DateSource::QuickTimeCreation => "quicktime_creation",
                    DateSource::EmbeddedDocAudio => "embedded_doc_audio",
                    DateSource::EmbeddedOther => "embedded_other",
                    DateSource::FilesystemModified => "filesystem_modified",
                    DateSource::None => "none",
                };

                let conf_str = match f.date_resolution.confidence {
                    ConfidenceLevel::High => "high",
                    ConfidenceLevel::Medium => "medium",
                    ConfidenceLevel::Low => "low",
                    ConfidenceLevel::Fallback => "fallback",
                };

                stmt.execute(params![
                    f.id,
                    job_id,
                    f.source_path.to_string_lossy(),
                    f.relative_path.to_string_lossy(),
                    f.filename,
                    f.extension,
                    f.size_bytes as i64,
                    f.modified_ms,
                    status_str,
                    f.date_resolution.resolved_date,
                    date_source_str,
                    f.date_resolution.raw_timestamp,
                    conf_str,
                    f.date_resolution.reason,
                    f.date_resolution.timezone_info,
                    f.destination_folder,
                    f.planned_filename,
                    f.destination_path.to_string_lossy(),
                    if f.is_collision { 1 } else { 0 },
                    f.collision_suffix,
                    f.sha256,
                    f.error_message,
                    0,
                    now,
                ])?;
            }

            // 2. Insert scan errors so no file or traversal error is silently dropped
            for err in scan_errors {
                total_files += 1;
                let err_id = uuid::Uuid::new_v4().to_string();
                let fname = err.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unreadable_entry".to_string());

                stmt.execute(params![
                    err_id,
                    job_id,
                    err.path.to_string_lossy(),
                    err.relative_path.to_string_lossy(),
                    fname,
                    Option::<String>::None,
                    0i64,
                    Option::<i64>::None,
                    "failed",
                    Option::<String>::None,
                    "none",
                    Option::<String>::None,
                    "fallback",
                    "Failed during directory scan traversal",
                    Option::<String>::None,
                    "Scan Errors",
                    fname,
                    Option::<String>::None,
                    0,
                    Option::<u32>::None,
                    Option::<String>::None,
                    Some(err.error_message.clone()),
                    1,
                    now,
                ])?;
            }

            // 3. Insert policy-excluded items (symlinks, junctions, devices) so accounting is 100% complete
            for exc in excluded {
                total_files += 1;
                let exc_id = uuid::Uuid::new_v4().to_string();
                let fname = exc.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "excluded_entry".to_string());

                stmt.execute(params![
                    exc_id,
                    job_id,
                    exc.path.to_string_lossy(),
                    exc.relative_path.to_string_lossy(),
                    fname,
                    Option::<String>::None,
                    0i64,
                    Option::<i64>::None,
                    "excluded",
                    Option::<String>::None,
                    "none",
                    Option::<String>::None,
                    "fallback",
                    &exc.reason,
                    Option::<String>::None,
                    "Excluded",
                    fname,
                    Option::<String>::None,
                    0,
                    Option::<u32>::None,
                    Option::<String>::None,
                    Some(exc.reason.clone()),
                    0,
                    now,
                ])?;
            }
        }

        let failed_count = scan_errors.len() as i64;
        let excluded_count = excluded.len() as i64;
        tx.execute(
            "UPDATE jobs SET total_files = ?1, total_bytes = ?2, failed_files = ?3, excluded_files = ?4, status = 'analysed' WHERE id = ?5",
            params![total_files as i64, total_bytes as i64, failed_count, excluded_count, job_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Retrieves all planned files for a job, e.g. for preview or execution
    pub fn get_planned_files(&self, job_id: &str, limit: Option<usize>, offset: usize) -> SqlResult<Vec<PlannedFile>> {
        let conn = self.conn.lock().unwrap();
        let query = if let Some(lim) = limit {
            format!(
                "SELECT id, source_path, relative_path, filename, extension, file_size, source_mtime,
                        status, resolved_date, date_source, raw_timestamp, confidence, reason,
                        timezone_info, destination_folder, planned_filename, destination_path,
                        is_collision, collision_suffix, sha256, error_message
                 FROM job_files WHERE job_id = ?1 ORDER BY rowid LIMIT {} OFFSET {}",
                lim, offset
            )
        } else {
            "SELECT id, source_path, relative_path, filename, extension, file_size, source_mtime,
                    status, resolved_date, date_source, raw_timestamp, confidence, reason,
                    timezone_info, destination_folder, planned_filename, destination_path,
                    is_collision, collision_suffix, sha256, error_message
             FROM job_files WHERE job_id = ?1 ORDER BY rowid".to_string()
        };

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(params![job_id], |row| {
            let status_str: String = row.get(7)?;
            let date_source_str: Option<String> = row.get(9)?;
            let conf_str: Option<String> = row.get(11)?;

            let status = match status_str.as_str() {
                "verified" => FileStatus::Verified,
                "failed" => FileStatus::Failed,
                "copying" => FileStatus::Copying,
                "analysed" => FileStatus::Analysed,
                "skipped" => FileStatus::Skipped,
                "excluded" => FileStatus::Excluded,
                "discovered" => FileStatus::Discovered,
                _ => FileStatus::Pending,
            };

            let date_source = match date_source_str.as_deref() {
                Some("exif_original") => DateSource::ExifOriginal,
                Some("quicktime_creation") => DateSource::QuickTimeCreation,
                Some("embedded_doc_audio") => DateSource::EmbeddedDocAudio,
                Some("embedded_other") => DateSource::EmbeddedOther,
                Some("filesystem_modified") => DateSource::FilesystemModified,
                _ => DateSource::None,
            };

            let confidence = match conf_str.as_deref() {
                Some("high") => ConfidenceLevel::High,
                Some("medium") => ConfidenceLevel::Medium,
                Some("low") => ConfidenceLevel::Low,
                _ => ConfidenceLevel::Fallback,
            };

            let date_resolution = DateResolution {
                resolved_date: row.get(8)?,
                date_source,
                raw_timestamp: row.get(10)?,
                confidence,
                reason: row.get(12).unwrap_or_default(),
                timezone_info: row.get(13)?,
            };

            let is_collision_int: i32 = row.get(17)?;
            let dst_str: Option<String> = row.get(16)?;

            Ok(PlannedFile {
                id: row.get(0)?,
                source_path: PathBuf::from(row.get::<_, String>(1)?),
                relative_path: PathBuf::from(row.get::<_, String>(2)?),
                filename: row.get(3)?,
                extension: row.get(4)?,
                size_bytes: row.get::<_, i64>(5)? as u64,
                modified_ms: row.get(6)?,
                date_resolution,
                destination_folder: row.get(14)?,
                planned_filename: row.get(15)?,
                destination_path: dst_str.map(PathBuf::from).unwrap_or_default(),
                is_collision: is_collision_int != 0,
                collision_suffix: row.get(18)?,
                status,
                sha256: row.get(19)?,
                error_message: row.get(20)?,
            })
        })?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    /// Updates status of a file during/after copy
    pub fn update_file_status(
        &self,
        file_id: &str,
        status: FileStatus,
        sha256: Option<&str>,
        error_message: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();

        let status_str = match status {
            FileStatus::Discovered => "discovered",
            FileStatus::Analysed => "analysed",
            FileStatus::Pending => "pending",
            FileStatus::Copying => "copying",
            FileStatus::Verified => "verified",
            FileStatus::Failed => "failed",
            FileStatus::Skipped => "skipped",
            FileStatus::Excluded => "excluded",
        };

        conn.execute(
            r#"
            UPDATE job_files
            SET status = ?1, sha256 = COALESCE(?2, sha256), error_message = ?3,
                attempt_count = attempt_count + 1, updated_at = ?4
            WHERE id = ?5
            "#,
            params![status_str, sha256, error_message, now, file_id],
        )?;

        Ok(())
    }

    /// Updates overall job status and progress counters
    pub fn update_job_status(&self, job_id: &str, status: JobStatus) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();

        let status_str = match status {
            JobStatus::Created => "created",
            JobStatus::Scanning => "scanning",
            JobStatus::Scanned => "scanned",
            JobStatus::Analysing => "analysing",
            JobStatus::Analysed => "analysed",
            JobStatus::Running => "running",
            JobStatus::Paused => "paused",
            JobStatus::Completed => "completed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Failed => "failed",
        };

        let started_update = if status == JobStatus::Running {
            ", started_at = COALESCE(started_at, ?2)"
        } else {
            ""
        };

        let completed_update = if matches!(status, JobStatus::Completed | JobStatus::Cancelled | JobStatus::Failed) {
            ", completed_at = ?2"
        } else {
            ""
        };

        let sql = format!(
            "UPDATE jobs SET status = ?1 {}{} WHERE id = ?3",
            started_update, completed_update
        );

        conn.execute(&sql, params![status_str, now, job_id])?;
        Ok(())
    }

    /// Resets all failed files in a job back to 'pending' for retry
    pub fn reset_failed_files_for_retry(&self, job_id: &str) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE job_files SET status = 'pending', error_message = NULL WHERE job_id = ?1 AND status = 'failed'",
            params![job_id],
        )?;

        // Re-sync failed_files counter
        conn.execute(
            "UPDATE jobs SET failed_files = 0, status = 'running' WHERE id = ?1",
            params![job_id],
        )?;

        Ok(count)
    }

    /// Returns persistent history of all past jobs
    pub fn get_all_jobs(&self) -> SqlResult<Vec<JobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, source_path, destination_path, status,
                   total_files, total_bytes, copied_files, copied_bytes, failed_files,
                   COALESCE(excluded_files, 0), created_at, completed_at
            FROM jobs
            ORDER BY created_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let status_str: String = row.get(4)?;
            let status = match status_str.as_str() {
                "created" => JobStatus::Created,
                "scanning" => JobStatus::Scanning,
                "scanned" => JobStatus::Scanned,
                "analysing" => JobStatus::Analysing,
                "analysed" => JobStatus::Analysed,
                "running" => JobStatus::Running,
                "paused" => JobStatus::Paused,
                "completed" => JobStatus::Completed,
                "cancelled" => JobStatus::Cancelled,
                _ => JobStatus::Failed,
            };

            Ok(JobRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                source_path: row.get(2)?,
                destination_path: row.get(3)?,
                status,
                total_files: row.get::<_, i64>(5)? as u64,
                total_bytes: row.get::<_, i64>(6)? as u64,
                copied_files: row.get::<_, i64>(7)? as u64,
                copied_bytes: row.get::<_, i64>(8)? as u64,
                failed_files: row.get::<_, i64>(9)? as u64,
                excluded_files: row.get::<_, i64>(10)? as u64,
                created_at: row.get(11)?,
                completed_at: row.get(12)?,
            })
        })?;

        let mut jobs = Vec::new();
        for r in rows {
            jobs.push(r?);
        }
        Ok(jobs)
    }

    /// Generates summary metrics for a job
    pub fn get_job_summary(&self, job_id: &str) -> SqlResult<JobSummary> {
        let conn = self.conn.lock().unwrap();

        let total_files: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let total_bytes: u64 = conn.query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM job_files WHERE job_id = ?1",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let embedded_date_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND date_source IN ('exif_original', 'quicktime_creation', 'embedded_doc_audio', 'embedded_other')",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let mtime_date_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND date_source = 'filesystem_modified'",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let no_date_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND date_source = 'none' AND status NOT IN ('failed', 'excluded')",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let metadata_error_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND error_message IS NOT NULL",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let scan_error_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND destination_folder = 'Scan Errors'",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let excluded_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM job_files WHERE job_id = ?1 AND status = 'excluded'",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let unique_folders_count: u64 = conn.query_row(
            "SELECT COUNT(DISTINCT destination_folder) FROM job_files WHERE job_id = ?1 AND destination_folder NOT IN ('Scan Errors', 'Excluded')",
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        let duplicate_content_count: u64 = conn.query_row(
            r#"
            SELECT COUNT(*) FROM job_files
            WHERE job_id = ?1 AND sha256 IS NOT NULL AND sha256 IN (
                SELECT sha256 FROM job_files WHERE job_id = ?1 AND sha256 IS NOT NULL GROUP BY sha256 HAVING COUNT(*) > 1
            )
            "#,
            params![job_id],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(0) as u64;

        Ok(JobSummary {
            total_files,
            total_bytes,
            embedded_date_count,
            mtime_date_count,
            no_date_count,
            metadata_error_count,
            duplicate_content_count,
            unique_folders_count,
            scan_error_count,
            excluded_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_crud_and_summary() {
        let db = Database::in_memory().unwrap();
        let job_id = "job-test-1";

        db.create_job(
            job_id,
            "Test Job",
            Path::new("C:/Source"),
            Path::new("C:/Dest"),
            &JobOptions::default(),
        ).unwrap();

        let files = vec![
            PlannedFile {
                id: "f1".to_string(),
                source_path: PathBuf::from("C:/Source/img1.jpg"),
                relative_path: PathBuf::from("img1.jpg"),
                filename: "img1.jpg".to_string(),
                extension: Some("jpg".to_string()),
                size_bytes: 1024,
                modified_ms: Some(1700000000),
                date_resolution: DateResolution {
                    resolved_date: Some("2024-05-18".to_string()),
                    date_source: DateSource::ExifOriginal,
                    raw_timestamp: Some("2024:05:18 10:00:00".to_string()),
                    confidence: ConfidenceLevel::High,
                    reason: "EXIF".to_string(),
                    timezone_info: None,
                },
                destination_folder: "2024-05-18".to_string(),
                planned_filename: "img1.jpg".to_string(),
                destination_path: PathBuf::from("C:/Dest/2024-05-18/img1.jpg"),
                is_collision: false,
                collision_suffix: None,
                status: FileStatus::Pending,
                sha256: None,
                error_message: None,
            },
        ];

        db.save_planned_files(job_id, &files, &[], &[]).unwrap();

        let retrieved = db.get_planned_files(job_id, None, 0).unwrap();
        assert_eq!(retrieved.len(), 1);

        let summary = db.get_job_summary(job_id).unwrap();
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.total_bytes, 1024);
        assert_eq!(summary.embedded_date_count, 1);
    }
}
