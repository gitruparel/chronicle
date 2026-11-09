use std::fs::File;
use std::path::Path;
use std::process::Command;
use crate::database::Database;

pub struct ReportGenerator;

impl ReportGenerator {
    /// Generates a CSV report file for a job
    pub fn export_csv(db: &Database, job_id: &str, output_path: &Path) -> Result<(), String> {
        let files = db.get_planned_files(job_id, None, 0)
            .map_err(|e| format!("Failed to query job files: {}", e))?;

        let mut wtr = csv::Writer::from_path(output_path)
            .map_err(|e| format!("Failed to create CSV writer: {}", e))?;

        wtr.write_record(&[
            "File ID",
            "Filename",
            "Source Path",
            "Relative Path",
            "File Size (Bytes)",
            "Resolved Date",
            "Date Source",
            "Raw Timestamp",
            "Timezone Info",
            "Destination Folder",
            "Planned Filename",
            "Destination Path",
            "Status",
            "SHA-256",
            "Error Message",
        ]).map_err(|e| format!("Failed to write CSV header: {}", e))?;

        for f in &files {
            let status_str = format!("{:?}", f.status);
            let src_str = f.source_path.to_string_lossy();
            let rel_str = f.relative_path.to_string_lossy();
            let dst_str = f.destination_path.to_string_lossy();
            let size_str = f.size_bytes.to_string();

            wtr.write_record(&[
                f.id.as_str(),
                f.filename.as_str(),
                src_str.as_ref(),
                rel_str.as_ref(),
                size_str.as_str(),
                f.date_resolution.resolved_date.as_deref().unwrap_or("No Date"),
                f.date_resolution.date_source.to_string().as_str(),
                f.date_resolution.raw_timestamp.as_deref().unwrap_or(""),
                f.date_resolution.timezone_info.as_deref().unwrap_or(""),
                f.destination_folder.as_str(),
                f.planned_filename.as_str(),
                dst_str.as_ref(),
                status_str.as_str(),
                f.sha256.as_deref().unwrap_or(""),
                f.error_message.as_deref().unwrap_or(""),
            ]).map_err(|e| format!("Failed to write CSV record: {}", e))?;
        }

        wtr.flush().map_err(|e| format!("Failed to flush CSV: {}", e))?;
        Ok(())
    }

    /// Generates a JSON report file for a job
    pub fn export_json(db: &Database, job_id: &str, output_path: &Path) -> Result<(), String> {
        let files = db.get_planned_files(job_id, None, 0)
            .map_err(|e| format!("Failed to query job files: {}", e))?;

        let summary = db.get_job_summary(job_id)
            .map_err(|e| format!("Failed to query job summary: {}", e))?;

        let report = serde_json::json!({
            "job_id": job_id,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "summary": summary,
            "files": files,
        });

        let file = File::create(output_path)
            .map_err(|e| format!("Failed to create JSON report file: {}", e))?;

        serde_json::to_writer_pretty(file, &report)
            .map_err(|e| format!("Failed to write JSON report: {}", e))?;

        Ok(())
    }

    /// Opens the specified folder path in Windows File Explorer
    pub fn open_in_explorer(path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("explorer")
                .arg(path.as_os_str())
                .spawn()
                .map_err(|e| format!("Failed to launch Windows Explorer: {}", e))?;
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            Command::new("xdg-open")
                .arg(path.as_os_str())
                .spawn()
                .map_err(|e| format!("Failed to open directory: {}", e))?;
            Ok(())
        }
    }
}
