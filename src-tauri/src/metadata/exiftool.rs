use std::path::{Path, PathBuf};
use std::process::Command;
use serde_json::Value;

pub struct ExifToolManager {
    executable_path: Option<PathBuf>,
}

impl ExifToolManager {
    pub fn new() -> Self {
        let path = Self::locate_exiftool();
        Self { executable_path: path }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            executable_path: if path.exists() { Some(path) } else { None },
        }
    }

    pub fn is_available(&self) -> bool {
        self.executable_path.is_some()
    }

    pub fn get_path(&self) -> Option<&PathBuf> {
        self.executable_path.as_ref()
    }

    /// Finds exiftool in known locations:
    /// 1. Application resource directory
    /// 2. Next to executable
    /// 3. Known install path in user LocalAppData
    /// 4. System PATH
    pub fn locate_exiftool() -> Option<PathBuf> {
        // 1. Current working directory / workspace resources
        let candidate1 = PathBuf::from("src-tauri/resources/exiftool.exe");
        if candidate1.exists() {
            return Some(candidate1);
        }
        let candidate1_alt = PathBuf::from("resources/exiftool.exe");
        if candidate1_alt.exists() {
            return Some(candidate1_alt);
        }

        // 2. Next to current executable
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let candidate2 = parent.join("resources").join("exiftool.exe");
                if candidate2.exists() {
                    return Some(candidate2);
                }
                let candidate2_flat = parent.join("exiftool.exe");
                if candidate2_flat.exists() {
                    return Some(candidate2_flat);
                }
            }
        }

        // 3. User LocalAppData Programs
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate3 = PathBuf::from(local_app_data).join("Programs").join("ExifTool").join("ExifTool.exe");
            if candidate3.exists() {
                return Some(candidate3);
            }
        }

        // 4. PATH lookup
        if let Ok(output) = Command::new("exiftool").arg("-ver").output() {
            if output.status.success() {
                return Some(PathBuf::from("exiftool"));
            }
        }

        None
    }

    /// Extracts metadata for a batch of file paths.
    /// Returns a list of JSON objects corresponding to each file.
    /// Exiftool is invoked with:
    /// `-j` (JSON output)
    /// `-charset filename=utf8` (UTF-8 path handling)
    /// `-fast2` (avoid reading trailing media data where not needed)
    /// `-G` (group names like EXIF, QuickTime, XMP, etc.)
    pub fn extract_batch(&self, paths: &[&Path]) -> Result<Vec<Value>, String> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let exe = match &self.executable_path {
            Some(p) => p,
            None => return Err("ExifTool binary not found or unavailable".to_string()),
        };

        let mut cmd = Command::new(exe);
        cmd.arg("-j")
           .arg("-charset")
           .arg("filename=utf8")
           .arg("-G") // Show tag group names: EXIF:DateTimeOriginal, QuickTime:CreateDate, etc.
           .arg("-fast2");

        for p in paths {
            cmd.arg(p.as_os_str());
        }

        let output = cmd.output().map_err(|e| format!("Failed to execute ExifTool: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("ExifTool returned non-zero status: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let json: Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse ExifTool JSON output: {}. Raw: {}", e, stdout))?;

        if let Value::Array(items) = json {
            Ok(items)
        } else {
            Err("Expected JSON array from ExifTool output".to_string())
        }
    }
}
