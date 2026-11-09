use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use uuid::Uuid;
use crate::types::{ExcludedItem, OverlapCheckResult, ScanErrorItem, ScanResult, ScannedFile};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x00000002;
const FILE_ATTRIBUTE_READONLY: u32 = 0x00000001;

/// Checks if source and destination paths have dangerous overlap:
/// 1. source == destination
/// 2. destination is inside source (e.g., source: C:\Data, dest: C:\Data\Organized)
/// 3. source is inside destination
pub fn check_overlap(source: &Path, destination: &Path) -> OverlapCheckResult {
    let canonical_src = match dunce::canonicalize(source) {
        Ok(p) => p,
        Err(_) => source.to_path_buf(),
    };
    let canonical_dst = match dunce::canonicalize(destination) {
        Ok(p) => p,
        Err(_) => destination.to_path_buf(),
    };

    if canonical_src == canonical_dst {
        return OverlapCheckResult {
            is_safe: false,
            error_message: Some("Destination folder cannot be the same as the source folder.".to_string()),
        };
    }

    if canonical_dst.starts_with(&canonical_src) {
        return OverlapCheckResult {
            is_safe: false,
            error_message: Some("Destination folder cannot be located inside the source folder. This prevents recursive copy loops.".to_string()),
        };
    }

    if canonical_src.starts_with(&canonical_dst) {
        return OverlapCheckResult {
            is_safe: false,
            error_message: Some("Source folder cannot be located inside the destination folder.".to_string()),
        };
    }

    OverlapCheckResult {
        is_safe: true,
        error_message: None,
    }
}

/// Checks whether a directory entry or metadata represents a reparse point / junction / symlink
pub fn is_reparse_point_or_symlink(entry: &walkdir::DirEntry) -> bool {
    if entry.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        if let Ok(metadata) = entry.metadata() {
            let attrs = metadata.file_attributes();
            if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return true;
            }
        }
    }

    false
}

/// Recursively scans the source directory for all regular files.
/// Never silently omits files. Traversal and metadata errors are explicitly collected and reported.
pub fn scan_directory(source_root: &Path, recursive: bool) -> Result<ScanResult, String> {
    if !source_root.exists() {
        return Err(format!("Source path does not exist: {}", source_root.display()));
    }
    if !source_root.is_dir() {
        return Err(format!("Source path is not a directory: {}", source_root.display()));
    }

    let canonical_root = dunce::canonicalize(source_root)
        .map_err(|e| format!("Failed to canonicalize source path: {}", e))?;

    let max_depth = if recursive { usize::MAX } else { 1 };
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut excluded = Vec::new();

    let walker = WalkDir::new(&canonical_root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter();

    for entry_result in walker {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                let bad_path = e.path().unwrap_or(source_root).to_path_buf();
                let rel = bad_path.strip_prefix(&canonical_root).unwrap_or(&bad_path).to_path_buf();
                log::warn!("Scan traversal error at {}: {}", bad_path.display(), e);
                errors.push(ScanErrorItem {
                    path: bad_path,
                    relative_path: rel,
                    error_message: format!("Directory traversal error: {}", e),
                });
                continue;
            }
        };

        let path = entry.path().to_path_buf();
        let relative_path = path.strip_prefix(&canonical_root)
            .unwrap_or(&path)
            .to_path_buf();

        // 1. Directory symlink / junction loop exclusion
        if entry.file_type().is_dir() {
            if entry.path() != canonical_root && is_reparse_point_or_symlink(&entry) {
                log::info!("Excluding directory junction/symlink: {}", path.display());
                excluded.push(ExcludedItem {
                    path,
                    relative_path,
                    reason: "Directory junction / symlink excluded by policy to prevent recursion loops".to_string(),
                });
            }
            continue;
        }

        // 2. File-level symlink or reparse point exclusion
        if is_reparse_point_or_symlink(&entry) {
            log::info!("Excluding reparse point/symlink file: {}", path.display());
            excluded.push(ExcludedItem {
                path,
                relative_path,
                reason: "File-level symlink / reparse point excluded by policy".to_string(),
            });
            continue;
        }

        // 3. Non-regular filesystem entry exclusion (e.g. pipes, sockets, devices)
        if !entry.file_type().is_file() {
            log::info!("Excluding non-regular special entry: {}", path.display());
            excluded.push(ExcludedItem {
                path,
                relative_path,
                reason: "Non-regular entry / special device excluded by policy".to_string(),
            });
            continue;
        }

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("Metadata read error for {}: {}", path.display(), e);
                errors.push(ScanErrorItem {
                    path,
                    relative_path,
                    error_message: format!("Failed to read filesystem metadata: {}", e),
                });
                continue;
            }
        };

        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let extension = path.extension()
            .map(|e| e.to_string_lossy().to_string().to_lowercase());

        let size_bytes = metadata.len();

        let modified_ms = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);

        let is_readonly = metadata.permissions().readonly();
        #[cfg(windows)]
        let is_readonly = is_readonly || (metadata.file_attributes() & FILE_ATTRIBUTE_READONLY) != 0;
        #[cfg(windows)]
        let is_hidden = (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0;
        #[cfg(not(windows))]
        let is_hidden = false;

        files.push(ScannedFile {
            id: Uuid::new_v4().to_string(),
            source_path: path,
            relative_path,
            filename,
            extension,
            size_bytes,
            modified_ms,
            is_readonly,
            is_hidden,
        });
    }

    Ok(ScanResult { files, errors, excluded })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_overlap_checks() {
        let dir1 = tempdir().unwrap();
        let p1 = dir1.path();
        let sub = p1.join("nested");
        fs::create_dir_all(&sub).unwrap();

        // Same folder
        let r1 = check_overlap(p1, p1);
        assert!(!r1.is_safe);

        // Sub folder
        let r2 = check_overlap(p1, &sub);
        assert!(!r2.is_safe);

        // Reverse
        let r3 = check_overlap(&sub, p1);
        assert!(!r3.is_safe);

        // Independent
        let dir2 = tempdir().unwrap();
        let p2 = dir2.path();
        let r4 = check_overlap(p1, p2);
        assert!(r4.is_safe);
    }

    #[test]
    fn test_scan_finds_all_file_types_and_reports_errors() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("test.txt"), "hello").unwrap();
        fs::write(root.join("no_extension"), "raw data").unwrap();
        fs::write(root.join("empty.bin"), "").unwrap(); // 0 byte
        fs::write(root.join("weird.tar.gz"), "binary").unwrap();

        let sub = root.join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.jpg"), "photo").unwrap();

        let res = scan_directory(root, true).unwrap();
        assert_eq!(res.files.len(), 5);
        assert_eq!(res.errors.len(), 0);
        assert_eq!(res.excluded.len(), 0);

        let zero_byte = res.files.iter().find(|f| f.filename == "empty.bin").unwrap();
        assert_eq!(zero_byte.size_bytes, 0);

        let no_ext = res.files.iter().find(|f| f.filename == "no_extension").unwrap();
        assert_eq!(no_ext.extension, None);
    }
}
