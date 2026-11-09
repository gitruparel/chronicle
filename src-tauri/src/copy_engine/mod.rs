use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CopyOptions {
    pub verify_hash: bool,
    pub buffer_size: usize,
}

impl Default for CopyOptions {
    fn default() -> Self {
        Self {
            verify_hash: true,
            buffer_size: 64 * 1024, // 64 KB chunks
        }
    }
}

pub struct CopyFileResult {
    pub success: bool,
    pub bytes_copied: u64,
    pub sha256: Option<String>,
    pub error: Option<String>,
}

/// Helper to safely delete a temporary file if it exists
fn cleanup_tmp_file(tmp_path: &Path) {
    if tmp_path.exists() {
        if let Err(e) = fs::remove_file(tmp_path) {
            log::warn!("Failed to remove temporary file {}: {}", tmp_path.display(), e);
        }
    }
}

/// Copies a single file using streaming chunks, simultaneous SHA-256 calculation,
/// temporary staging file, size verification, and atomic rename.
/// Never alters or deletes source file.
pub fn copy_and_verify_file<F>(
    source_path: &Path,
    destination_path: &Path,
    options: &CopyOptions,
    cancellation_flag: Option<&Arc<AtomicBool>>,
    mut progress_callback: F,
) -> CopyFileResult
where
    F: FnMut(u64), // called with bytes written in this step
{
    // 1. Pre-flight source existence and metadata check
    let source_meta = match fs::metadata(source_path) {
        Ok(m) => m,
        Err(e) => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                error: Some(format!("Cannot access source file {}: {}", source_path.display(), e)),
            };
        }
    };

    let expected_len = source_meta.len();
    let initial_mtime = source_meta.modified().ok();

    // 2. Ensure destination folder exists
    let dest_dir = match destination_path.parent() {
        Some(d) => d,
        None => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                error: Some("Destination path has no parent directory".to_string()),
            };
        }
    };

    if let Err(e) = fs::create_dir_all(dest_dir) {
        return CopyFileResult {
            success: false,
            bytes_copied: 0,
            sha256: None,
            error: Some(format!("Failed to create destination directory {}: {}", dest_dir.display(), e)),
        };
    }

    // 3. Create temporary file in the destination folder
    let filename = destination_path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let tmp_filename = format!(".chronicle_tmp_{}_{}", Uuid::new_v4().simple(), filename);
    let tmp_path = dest_dir.join(&tmp_filename);

    // 4. Open source strictly read-only
    let mut src_file = match File::open(source_path) {
        Ok(f) => f,
        Err(e) => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                error: Some(format!("Failed to open source file for reading: {}", e)),
            };
        }
    };

    // Open temporary destination file with write + create_new
    let mut dst_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
    {
        Ok(f) => f,
        Err(e) => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                error: Some(format!("Failed to create temporary destination file: {}", e)),
            };
        }
    };

    // 5. Streaming copy with SHA-256 calculation
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; options.buffer_size];
    let mut total_copied: u64 = 0;

    loop {
        // Cooperative cancellation check
        if let Some(flag) = cancellation_flag {
            if flag.load(Ordering::SeqCst) {
                drop(dst_file);
                cleanup_tmp_file(&tmp_path);
                return CopyFileResult {
                    success: false,
                    bytes_copied: total_copied,
                    sha256: None,
                    error: Some("Operation cancelled by user".to_string()),
                };
            }
        }

        let read_bytes = match src_file.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => n,
            Err(e) => {
                drop(dst_file);
                cleanup_tmp_file(&tmp_path);
                return CopyFileResult {
                    success: false,
                    bytes_copied: total_copied,
                    sha256: None,
                    error: Some(format!("Read error from source file: {}", e)),
                };
            }
        };

        if let Err(e) = dst_file.write_all(&buffer[..read_bytes]) {
            drop(dst_file);
            cleanup_tmp_file(&tmp_path);
            return CopyFileResult {
                success: false,
                bytes_copied: total_copied,
                sha256: None,
                error: Some(format!("Write error to destination: {}", e)),
            };
        }

        if options.verify_hash {
            hasher.update(&buffer[..read_bytes]);
        }

        total_copied += read_bytes as u64;
        progress_callback(read_bytes as u64);
    }

    // Flush destination file to disk
    if let Err(e) = dst_file.flush() {
        drop(dst_file);
        cleanup_tmp_file(&tmp_path);
        return CopyFileResult {
            success: false,
            bytes_copied: total_copied,
            sha256: None,
            error: Some(format!("Failed to flush destination file: {}", e)),
        };
    }
    drop(dst_file);

    // 6. Verification: Length check
    if total_copied != expected_len {
        cleanup_tmp_file(&tmp_path);
        return CopyFileResult {
            success: false,
            bytes_copied: total_copied,
            sha256: None,
            error: Some(format!("Size mismatch: copied {} bytes, expected {}", total_copied, expected_len)),
        };
    }

    // 7. Verify source did not change during reading
    if let Ok(after_meta) = fs::metadata(source_path) {
        if after_meta.len() != expected_len || (initial_mtime.is_some() && after_meta.modified().ok() != initial_mtime) {
            cleanup_tmp_file(&tmp_path);
            return CopyFileResult {
                success: false,
                bytes_copied: total_copied,
                sha256: None,
                error: Some("Source file was modified concurrently during copy".to_string()),
            };
        }
    }

    // 8. Compute final hash
    let sha256_hex = if options.verify_hash {
        let hash_result = hasher.finalize();
        Some(format!("{:x}", hash_result))
    } else {
        None
    };

    // 9. Atomic rename to destination path
    if let Err(e) = fs::rename(&tmp_path, destination_path) {
        cleanup_tmp_file(&tmp_path);
        return CopyFileResult {
            success: false,
            bytes_copied: total_copied,
            sha256: None,
            error: Some(format!("Atomic rename failed: {}", e)),
        };
    }

    CopyFileResult {
        success: true,
        bytes_copied: total_copied,
        sha256: sha256_hex,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_streaming_copy_and_hash_verification() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("source.txt");
        let content = b"Chronicle test content for copy engine verification with SHA-256";
        fs::write(&src_file, content).unwrap();

        let dst_file = dst_dir.path().join("copied.txt");
        let opts = CopyOptions::default();

        let mut tracked_bytes = 0;
        let res = copy_and_verify_file(&src_file, &dst_file, &opts, None, |bytes| {
            tracked_bytes += bytes;
        });

        assert!(res.success);
        assert_eq!(res.bytes_copied, content.len() as u64);
        assert_eq!(tracked_bytes, content.len() as u64);
        assert!(res.sha256.is_some());

        // Verify source was untouched
        assert_eq!(fs::read(&src_file).unwrap(), content);
        // Verify destination matches
        assert_eq!(fs::read(&dst_file).unwrap(), content);
    }

    #[test]
    fn test_copy_zero_byte_file() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("zero.bin");
        fs::write(&src_file, b"").unwrap();

        let dst_file = dst_dir.path().join("zero_copied.bin");
        let opts = CopyOptions::default();

        let res = copy_and_verify_file(&src_file, &dst_file, &opts, None, |_| {});
        assert!(res.success);
        assert_eq!(res.bytes_copied, 0);
        assert!(dst_file.exists());
    }

    #[test]
    fn test_cancellation_cleans_up_tmp_file() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("large.dat");
        fs::write(&src_file, vec![0xAA; 100_000]).unwrap();

        let dst_file = dst_dir.path().join("cancelled.dat");
        let opts = CopyOptions { buffer_size: 1024, verify_hash: true };

        let cancel = Arc::new(AtomicBool::new(true)); // cancel immediately
        let res = copy_and_verify_file(&src_file, &dst_file, &opts, Some(&cancel), |_| {});

        assert!(!res.success);
        assert_eq!(res.error.unwrap(), "Operation cancelled by user");
        assert!(!dst_file.exists());

        // Ensure no lingering .chronicle_tmp_* files in destination dir
        let files: Vec<_> = fs::read_dir(dst_dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(files.is_empty());
    }
}
