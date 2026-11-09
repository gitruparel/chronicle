use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::organizer::{make_suffixed_filename, split_stem_and_ext};

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
    pub final_destination_path: Option<PathBuf>,
    pub error: Option<String>,
}

/// Safely removes a temporary file if it still exists
pub fn cleanup_tmp_file(tmp_path: &Path) {
    if tmp_path.exists() {
        if let Err(e) = fs::remove_file(tmp_path) {
            log::warn!("Failed to clean up temporary file {}: {}", tmp_path.display(), e);
        }
    }
}

/// Independently computes the SHA-256 hash of a file on disk
pub fn hash_file_on_disk(path: &Path, buffer_size: usize) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; buffer_size];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Atomically commits a temporary file to a target destination without ever overwriting.
/// If target already exists on disk, allocates a collision suffix deterministically.
pub fn commit_non_overwriting(tmp_path: &Path, planned_dest: &Path) -> Result<PathBuf, String> {
    let parent_dir = planned_dest.parent()
        .ok_or_else(|| "Destination has no parent directory".to_string())?;

    let filename = planned_dest.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let (stem, ext) = split_stem_and_ext(&filename);

    let mut candidate_path = planned_dest.to_path_buf();
    let mut suffix = 0;

    // Loop until we find a target that does not exist, ensuring zero overwrite
    while candidate_path.exists() {
        suffix += 1;
        let suffixed_name = make_suffixed_filename(&stem, ext.as_deref(), suffix);
        candidate_path = parent_dir.join(&suffixed_name);
    }

    // Atomic rename to confirmed non-existing path
    fs::rename(tmp_path, &candidate_path)
        .map_err(|e| format!("Failed to commit file from {} to {}: {}", tmp_path.display(), candidate_path.display(), e))?;

    Ok(candidate_path)
}

/// Copies a single file using:
/// 1. Pre-flight source verification.
/// 2. Chunked streaming into a temporary staging file `.chronicle_tmp_*`.
/// 3. Mid-stream cooperative cancellation and pause checks.
/// 4. Source SHA-256 calculation during streaming.
/// 5. Physical disk synchronization via sync_all().
/// 6. Independent destination SHA-256 verification.
/// 7. Non-overwriting atomic commit.
pub fn copy_and_verify_file<F>(
    source_path: &Path,
    destination_path: &Path,
    options: &CopyOptions,
    cancellation_flag: Option<&Arc<AtomicBool>>,
    pause_flag: Option<&Arc<AtomicBool>>,
    mut progress_callback: F,
) -> CopyFileResult
where
    F: FnMut(u64),
{
    // 1. Source existence and metadata check
    let source_meta = match fs::metadata(source_path) {
        Ok(m) => m,
        Err(e) => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                final_destination_path: None,
                error: Some(format!("Cannot access source file {}: {}", source_path.display(), e)),
            };
        }
    };

    let expected_len = source_meta.len();
    let initial_mtime = source_meta.modified().ok();

    // 2. Ensure parent destination directory exists
    let dest_dir = match destination_path.parent() {
        Some(d) => d,
        None => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                final_destination_path: None,
                error: Some("Destination path has no parent directory".to_string()),
            };
        }
    };

    if let Err(e) = fs::create_dir_all(dest_dir) {
        return CopyFileResult {
            success: false,
            bytes_copied: 0,
            sha256: None,
            final_destination_path: None,
            error: Some(format!("Failed to create destination directory {}: {}", dest_dir.display(), e)),
        };
    }

    // 3. Create temporary file
    let filename = destination_path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let tmp_filename = format!(".chronicle_tmp_{}_{}", Uuid::new_v4().simple(), filename);
    let tmp_path = dest_dir.join(&tmp_filename);

    let mut src_file = match File::open(source_path) {
        Ok(f) => f,
        Err(e) => {
            return CopyFileResult {
                success: false,
                bytes_copied: 0,
                sha256: None,
                final_destination_path: None,
                error: Some(format!("Failed to open source file: {}", e)),
            };
        }
    };

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
                final_destination_path: None,
                error: Some(format!("Failed to create temporary file: {}", e)),
            };
        }
    };

    // 4. Streaming copy with mid-chunk pause and cancellation
    let mut source_hasher = Sha256::new();
    let mut buffer = vec![0u8; options.buffer_size];
    let mut total_copied: u64 = 0;

    loop {
        // Cooperative cancellation check
        if let Some(c_flag) = cancellation_flag {
            if c_flag.load(Ordering::SeqCst) {
                drop(dst_file);
                cleanup_tmp_file(&tmp_path);
                return CopyFileResult {
                    success: false,
                    bytes_copied: total_copied,
                    sha256: None,
                    final_destination_path: None,
                    error: Some("Operation cancelled by user".to_string()),
                };
            }
        }

        // Mid-copy pause check: pauses large files immediately
        if let Some(p_flag) = pause_flag {
            while p_flag.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Some(c_flag) = cancellation_flag {
                    if c_flag.load(Ordering::SeqCst) {
                        drop(dst_file);
                        cleanup_tmp_file(&tmp_path);
                        return CopyFileResult {
                            success: false,
                            bytes_copied: total_copied,
                            sha256: None,
                            final_destination_path: None,
                            error: Some("Operation cancelled by user during pause".to_string()),
                        };
                    }
                }
            }
        }

        let read_bytes = match src_file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(dst_file);
                cleanup_tmp_file(&tmp_path);
                return CopyFileResult {
                    success: false,
                    bytes_copied: total_copied,
                    sha256: None,
                    final_destination_path: None,
                    error: Some(format!("Read error from source: {}", e)),
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
                final_destination_path: None,
                error: Some(format!("Write error to destination: {}", e)),
            };
        }

        if options.verify_hash {
            source_hasher.update(&buffer[..read_bytes]);
        }

        total_copied += read_bytes as u64;
        progress_callback(read_bytes as u64);
    }

    // 5. Flush and sync physically to disk storage
    if let Err(e) = dst_file.flush() {
        drop(dst_file);
        cleanup_tmp_file(&tmp_path);
        return CopyFileResult {
            success: false,
            bytes_copied: total_copied,
            sha256: None,
            final_destination_path: None,
            error: Some(format!("Failed to flush temporary file: {}", e)),
        };
    }
    if let Err(e) = dst_file.sync_all() {
        drop(dst_file);
        cleanup_tmp_file(&tmp_path);
        return CopyFileResult {
            success: false,
            bytes_copied: total_copied,
            sha256: None,
            final_destination_path: None,
            error: Some(format!("Failed to sync temporary file to physical disk: {}", e)),
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
            final_destination_path: None,
            error: Some(format!("Size mismatch: copied {} bytes, expected {}", total_copied, expected_len)),
        };
    }

    // 7. Verify source was not concurrently modified during stream
    if let Ok(after_meta) = fs::metadata(source_path) {
        if after_meta.len() != expected_len || (initial_mtime.is_some() && after_meta.modified().ok() != initial_mtime) {
            cleanup_tmp_file(&tmp_path);
            return CopyFileResult {
                success: false,
                bytes_copied: total_copied,
                sha256: None,
                final_destination_path: None,
                error: Some("Source file changed concurrently during copy".to_string()),
            };
        }
    }

    // 8. Independent destination hash calculation & comparison
    let final_sha256 = if options.verify_hash {
        let src_sha256 = format!("{:x}", source_hasher.finalize());

        let dst_sha256 = match hash_file_on_disk(&tmp_path, options.buffer_size) {
            Ok(h) => h,
            Err(e) => {
                cleanup_tmp_file(&tmp_path);
                return CopyFileResult {
                    success: false,
                    bytes_copied: total_copied,
                    sha256: None,
                    final_destination_path: None,
                    error: Some(format!("Failed to independently hash destination file: {}", e)),
                };
            }
        };

        if src_sha256 != dst_sha256 {
            cleanup_tmp_file(&tmp_path);
            return CopyFileResult {
                success: false,
                bytes_copied: total_copied,
                sha256: None,
                final_destination_path: None,
                error: Some(format!("SHA-256 verification mismatch! Source: {}, Dest: {}", src_sha256, dst_sha256)),
            };
        }

        Some(dst_sha256)
    } else {
        None
    };

    // 9. Non-overwriting atomic commit
    let committed_path = match commit_non_overwriting(&tmp_path, destination_path) {
        Ok(p) => p,
        Err(e) => {
            cleanup_tmp_file(&tmp_path);
            return CopyFileResult {
                success: false,
                bytes_copied: total_copied,
                sha256: None,
                final_destination_path: None,
                error: Some(e),
            };
        }
    };

    CopyFileResult {
        success: true,
        bytes_copied: total_copied,
        sha256: final_sha256,
        final_destination_path: Some(committed_path),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_streaming_copy_and_independent_hash_verification() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("source.txt");
        let content = b"Chronicle test content for copy engine verification with SHA-256";
        fs::write(&src_file, content).unwrap();

        let dst_file = dst_dir.path().join("copied.txt");
        let opts = CopyOptions::default();

        let mut tracked_bytes = 0;
        let res = copy_and_verify_file(&src_file, &dst_file, &opts, None, None, |bytes| {
            tracked_bytes += bytes;
        });

        assert!(res.success);
        assert_eq!(res.bytes_copied, content.len() as u64);
        assert_eq!(tracked_bytes, content.len() as u64);
        assert!(res.sha256.is_some());

        // Independent hash on disk matches
        let disk_hash = hash_file_on_disk(&dst_file, 1024).unwrap();
        assert_eq!(res.sha256.unwrap(), disk_hash);

        // Verify source was untouched
        assert_eq!(fs::read(&src_file).unwrap(), content);
        assert_eq!(fs::read(&dst_file).unwrap(), content);
    }

    #[test]
    fn test_non_overwriting_commit_when_file_exists() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("doc.txt");
        fs::write(&src_file, b"new version").unwrap();

        let dst_file = dst_dir.path().join("doc.txt");
        fs::write(&dst_file, b"existing version that must NOT be overwritten").unwrap();

        let opts = CopyOptions::default();
        let res = copy_and_verify_file(&src_file, &dst_file, &opts, None, None, |_| {});

        assert!(res.success);
        assert_eq!(fs::read(&dst_file).unwrap(), b"existing version that must NOT be overwritten");

        // Suffixed file was created
        let suffixed = dst_dir.path().join("doc (1).txt");
        assert!(suffixed.exists());
        assert_eq!(fs::read(&suffixed).unwrap(), b"new version");
    }

    #[test]
    fn test_mid_copy_pause() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let src_file = src_dir.path().join("medium.bin");
        fs::write(&src_file, vec![0x42; 50_000]).unwrap();
        let dst_file = dst_dir.path().join("medium_copied.bin");

        let opts = CopyOptions { buffer_size: 1024, verify_hash: true };
        let pause_flag = Arc::new(AtomicBool::new(true));

        // Spawn copy in background thread
        let p_flag = pause_flag.clone();
        let handle = std::thread::spawn(move || {
            copy_and_verify_file(&src_file, &dst_file, &opts, None, Some(&p_flag), |_| {})
        });

        // Let it enter pause
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(pause_flag.load(Ordering::SeqCst));

        // Resume
        pause_flag.store(false, Ordering::SeqCst);
        let res = handle.join().unwrap();
        assert!(res.success);
        assert_eq!(res.bytes_copied, 50_000);
    }
}
