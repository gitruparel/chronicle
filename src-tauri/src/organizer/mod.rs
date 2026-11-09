use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crate::types::{DateResolution, FileStatus, PlannedFile, ScannedFile};

/// Splits a filename into base stem and extension.
/// Handles:
/// - "photo.jpg" -> ("photo", Some("jpg"))
/// - "archive.tar.gz" -> ("archive.tar", Some("gz"))
/// - ".gitignore" -> (".gitignore", None)
/// - "unknown" -> ("unknown", None)
pub fn split_stem_and_ext(filename: &str) -> (String, Option<String>) {
    let path = Path::new(filename);
    let stem = match path.file_stem() {
        Some(s) => s.to_string_lossy().to_string(),
        None => filename.to_string(),
    };

    let ext = path.extension().map(|e| e.to_string_lossy().to_string());

    if filename.starts_with('.') && ext.is_none() {
        return (filename.to_string(), None);
    }

    (stem, ext)
}

/// Generates a suffixed filename when collision occurs:
/// - "photo.jpg" with suffix 1 -> "photo (1).jpg"
/// - "unknown" with suffix 2 -> "unknown (2)"
pub fn make_suffixed_filename(stem: &str, ext: Option<&str>, suffix: u32) -> String {
    if suffix == 0 {
        match ext {
            Some(e) => format!("{}.{}", stem, e),
            None => stem.to_string(),
        }
    } else {
        match ext {
            Some(e) => format!("{} ({}).{}", stem, suffix, e),
            None => format!("{} ({})", stem, suffix),
        }
    }
}

/// Tracks assigned filenames in each destination folder to ensure no two files
/// get the same target path, and checks if target file already exists on disk.
pub struct CollisionTracker {
    destination_root: PathBuf,
    /// Map of folder_name -> Set of lowercased filenames already planned or existing
    planned_names_case_insensitive: HashMap<String, HashSet<String>>,
}

impl CollisionTracker {
    pub fn new(destination_root: PathBuf) -> Self {
        Self {
            destination_root,
            planned_names_case_insensitive: HashMap::new(),
        }
    }

    /// Resolves a collision-free filename in the given destination folder.
    /// Checks both the in-memory planned set and the physical disk.
    pub fn allocate_filename(&mut self, folder_name: &str, original_filename: &str) -> (String, bool, Option<u32>) {
        let folder_set = self.planned_names_case_insensitive
            .entry(folder_name.to_string())
            .or_default();

        let (stem, ext) = split_stem_and_ext(original_filename);
        let target_dir = self.destination_root.join(folder_name);

        let mut suffix: u32 = 0;
        let mut candidate = original_filename.to_string();
        let mut is_collision = false;

        loop {
            let candidate_lower = candidate.to_lowercase();
            let in_memory_taken = folder_set.contains(&candidate_lower);
            let on_disk_taken = target_dir.join(&candidate).exists();

            if !in_memory_taken && !on_disk_taken {
                folder_set.insert(candidate_lower);
                let collision_suffix = if is_collision { Some(suffix) } else { None };
                return (candidate, is_collision, collision_suffix);
            }

            is_collision = true;
            suffix += 1;
            candidate = make_suffixed_filename(&stem, ext.as_deref(), suffix);
        }
    }
}

/// Plans destination folders and collision-safe filenames for a list of scanned files.
pub fn plan_destinations(
    destination_root: &Path,
    scanned_files: Vec<(ScannedFile, DateResolution)>,
) -> Vec<PlannedFile> {
    let mut tracker = CollisionTracker::new(destination_root.to_path_buf());
    let mut planned = Vec::with_capacity(scanned_files.len());

    for (file, date_res) in scanned_files {
        let folder_name = match &date_res.resolved_date {
            Some(d) => d.clone(),
            None => "No Date".to_string(),
        };

        let (planned_filename, is_collision, collision_suffix) =
            tracker.allocate_filename(&folder_name, &file.filename);

        let destination_path = destination_root
            .join(&folder_name)
            .join(&planned_filename);

        planned.push(PlannedFile {
            id: file.id.clone(),
            source_path: file.source_path.clone(),
            relative_path: file.relative_path.clone(),
            filename: file.filename.clone(),
            extension: file.extension.clone(),
            size_bytes: file.size_bytes,
            modified_ms: file.modified_ms,
            date_resolution: date_res,
            destination_folder: folder_name,
            planned_filename,
            destination_path,
            is_collision,
            collision_suffix,
            status: FileStatus::Pending,
            sha256: None,
            error_message: None,
        });
    }

    planned
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_split_stem_and_ext() {
        assert_eq!(split_stem_and_ext("photo.jpg"), ("photo".to_string(), Some("jpg".to_string())));
        assert_eq!(split_stem_and_ext("archive.tar.gz"), ("archive.tar".to_string(), Some("gz".to_string())));
        assert_eq!(split_stem_and_ext(".gitignore"), (".gitignore".to_string(), None));
        assert_eq!(split_stem_and_ext("README"), ("README".to_string(), None));
    }

    #[test]
    fn test_collision_resolution_multiple_duplicates() {
        let dir = tempdir().unwrap();
        let mut tracker = CollisionTracker::new(dir.path().to_path_buf());

        let (f1, c1, s1) = tracker.allocate_filename("2024-05-18", "IMG_0001.JPG");
        assert_eq!(f1, "IMG_0001.JPG");
        assert!(!c1);
        assert_eq!(s1, None);

        let (f2, c2, s2) = tracker.allocate_filename("2024-05-18", "IMG_0001.JPG");
        assert_eq!(f2, "IMG_0001 (1).JPG");
        assert!(c2);
        assert_eq!(s2, Some(1));

        let (f3, c3, s3) = tracker.allocate_filename("2024-05-18", "IMG_0001.JPG");
        assert_eq!(f3, "IMG_0001 (2).JPG");
        assert!(c3);
        assert_eq!(s3, Some(2));

        let (f4, c4, s4) = tracker.allocate_filename("2024-05-19", "IMG_0001.JPG");
        assert_eq!(f4, "IMG_0001.JPG");
        assert!(!c4);
        assert_eq!(s4, None);
    }

    #[test]
    fn test_collision_resolution_against_existing_disk_file() {
        let dir = tempdir().unwrap();
        let target_folder = dir.path().join("2024-05-18");
        fs::create_dir_all(&target_folder).unwrap();
        fs::write(target_folder.join("document.pdf"), "already here").unwrap();

        let mut tracker = CollisionTracker::new(dir.path().to_path_buf());
        let (f, coll, suff) = tracker.allocate_filename("2024-05-18", "document.pdf");
        assert_eq!(f, "document (1).pdf");
        assert!(coll);
        assert_eq!(suff, Some(1));
    }
}
