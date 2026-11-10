use std::path::{Path, PathBuf};
use std::process::Command;
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Creates a Command configured on Windows to suppress console window popup (CREATE_NO_WINDOW = 0x08000000).
/// This prevents CMD/PowerShell windows from flickering open and closing during batch metadata extraction.
fn silent_command<P: AsRef<std::ffi::OsStr>>(program: P) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

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

    /// Checks whether an ExifTool executable actually runs and responds to -ver
    fn is_working_binary(path: &Path) -> bool {
        if !path.exists() {
            return false;
        }
        match silent_command(path).arg("-ver").output() {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// Finds exiftool in known locations:
    /// 1. Official Tauri 2 runtime resource directory (for packaged production installer/NSIS)
    /// 2. Next to executable or sibling resources directory
    /// 3. Development workspace resources folder (src-tauri/resources/exiftool.exe)
    /// 4. Known install path in user LocalAppData
    /// 5. System PATH
    pub fn locate_exiftool_with_app(app: Option<&AppHandle>) -> Option<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Official Tauri 2 resource resolver
        if let Some(app_handle) = app {
            if let Ok(resolved) = app_handle.path().resolve("resources/exiftool.exe", tauri::path::BaseDirectory::Resource) {
                candidates.push(resolved);
            }
            if let Ok(res_dir) = app_handle.path().resource_dir() {
                candidates.push(res_dir.join("resources").join("exiftool.exe"));
                candidates.push(res_dir.join("exiftool.exe"));
            }
        }

        // 2. Next to current executable (packaged binary sibling)
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                candidates.push(parent.join("resources").join("exiftool.exe"));
                candidates.push(parent.join("exiftool.exe"));
            }
        }

        // 3. Current working directory / workspace resources (dev mode)
        candidates.push(PathBuf::from("src-tauri/resources/exiftool.exe"));
        candidates.push(PathBuf::from("resources/exiftool.exe"));

        // 4. User LocalAppData Programs
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local_app_data).join("Programs").join("ExifTool").join("ExifTool.exe"));
        }

        // First pass: find candidate that successfully executes -ver
        for candidate in &candidates {
            if Self::is_working_binary(candidate) {
                log::info!("Discovered working ExifTool at: {}", candidate.display());
                return Some(candidate.clone());
            }
        }

        // 5. System PATH lookup
        if let Ok(output) = silent_command("exiftool").arg("-ver").output() {
            if output.status.success() {
                log::info!("Discovered system PATH exiftool");
                return Some(PathBuf::from("exiftool"));
            }
        }

        // Fallback: return first candidate that exists on disk even if execution verification failed
        for candidate in candidates {
            if candidate.exists() {
                log::warn!("Using unverified ExifTool at: {}", candidate.display());
                return Some(candidate);
            }
        }

        None
    }

    /// Extracts metadata for a batch of file paths.
    /// Returns a list of JSON objects corresponding to each file.
    /// Exiftool is invoked with:
    /// `-j` (JSON output)
    /// `-charset filename=utf8` (UTF-8 path handling)
    /// `-fast` (avoid parsing maker notes, but allow reading trailer moov atoms for MP4/MOV)
    /// `-G` (group names like EXIF, QuickTime, XMP, etc.)
    pub fn extract_batch(&self, paths: &[&Path]) -> Result<Vec<Value>, String> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let exe = match &self.executable_path {
            Some(p) => p,
            None => return Err("ExifTool binary not found or unavailable".to_string()),
        };

        let mut cmd = silent_command(exe);
        cmd.arg("-j")
           .arg("-charset")
           .arg("filename=utf8")
           .arg("-G") // Show tag group names: EXIF:DateTimeOriginal, QuickTime:CreateDate, etc.
           .arg("-fast"); // Fast tag extraction without skipping video trailer atoms

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
