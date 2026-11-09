use std::path::{Path, PathBuf};
use std::process::Command;
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub struct ExifToolManager {
    executable_path: Option<PathBuf>,
}

impl ExifToolManager {
    pub fn new() -> Self {
        let path = Self::locate_exiftool_with_app(None);
        Self { executable_path: path }
    }

    /// Primary production constructor using Tauri 2 runtime resource resolver
    pub fn from_app_handle(app: &AppHandle) -> Self {
        let path = Self::locate_exiftool_with_app(Some(app));
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

    pub fn locate_exiftool() -> Option<PathBuf> {
        Self::locate_exiftool_with_app(None)
    }

    /// Finds exiftool in known locations:
    /// 1. Official Tauri 2 runtime resource directory (for packaged production installer/NSIS)
    /// 2. Next to executable or sibling resources directory
    /// 3. Development workspace resources folder (src-tauri/resources/exiftool.exe)
    /// 4. Known install path in user LocalAppData
    /// 5. System PATH
    pub fn locate_exiftool_with_app(app: Option<&AppHandle>) -> Option<PathBuf> {
        // 1. Official Tauri 2 resource resolver
        if let Some(app_handle) = app {
            if let Ok(resolved) = app_handle.path().resolve("resources/exiftool.exe", tauri::path::BaseDirectory::Resource) {
                if resolved.exists() {
                    return Some(resolved);
                }
            }
            if let Ok(res_dir) = app_handle.path().resource_dir() {
                let candidate_sub = res_dir.join("resources").join("exiftool.exe");
                if candidate_sub.exists() {
                    return Some(candidate_sub);
                }
                let candidate_flat = res_dir.join("exiftool.exe");
                if candidate_flat.exists() {
                    return Some(candidate_flat);
                }
            }
        }

        // 2. Next to current executable (packaged binary sibling)
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

        // 3. Current working directory / workspace resources (dev mode)
        let candidate1 = PathBuf::from("src-tauri/resources/exiftool.exe");
        if candidate1.exists() {
            return Some(candidate1);
        }
        let candidate1_alt = PathBuf::from("resources/exiftool.exe");
        if candidate1_alt.exists() {
            return Some(candidate1_alt);
        }

        // 4. User LocalAppData Programs
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate3 = PathBuf::from(local_app_data).join("Programs").join("ExifTool").join("ExifTool.exe");
            if candidate3.exists() {
                return Some(candidate3);
            }
        }

        // 5. System PATH lookup
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
