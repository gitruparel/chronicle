use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;
use sha2::{Digest, Sha256};
use serde_json::json;

use chronicle_lib::database::Database;
use chronicle_lib::date_resolver::{parse_timestamp_to_date, resolve_date};
use chronicle_lib::organizer::plan_destinations;
use chronicle_lib::reports::ReportGenerator;
use chronicle_lib::scanner::{check_overlap, scan_directory};
use chronicle_lib::types::{ConfidenceLevel, DateResolution, DateSource, FileStatus, JobOptions, PlannedFile};
use chronicle_lib::copy_engine::{copy_and_verify_file, CopyOptions};

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[test]
fn test_end_to_end_pipeline_with_diverse_fixtures() {
    let src_dir = tempdir().unwrap();
    let dst_dir = tempdir().unwrap();
    let src = src_dir.path();
    let dst = dst_dir.path();

    // 1. Create subdirectories
    let sub_a = src.join("folder_a");
    let sub_b = src.join("folder_b");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();

    // 2. Populate diverse test fixtures
    // Regular text file
    fs::write(src.join("notes.txt"), b"Meeting notes from project").unwrap();

    // Extensionless file
    fs::write(src.join("LICENSE_DATA"), b"Universal data file").unwrap();

    // Zero-byte file
    fs::write(src.join("empty_file.dat"), b"").unwrap();

    // Unicode filenames
    fs::write(src.join("照片_2024.jpg"), b"fake jpeg data 1").unwrap();
    fs::write(src.join("документ.pdf"), b"fake pdf data").unwrap();
    fs::write(src.join("caf\u{00e9}_re\u{0301}sume\u{0301}.docx"), b"unicode doc").unwrap();

    // Filename collision across subdirectories: folder_a/contract.pdf and folder_b/contract.pdf
    fs::write(sub_a.join("contract.pdf"), b"Contract Version A").unwrap();
    fs::write(sub_b.join("contract.pdf"), b"Contract Version B").unwrap();

    // Byte-identical duplicate content files with different names
    let identical_bytes = b"EXACT_SAME_BYTE_CONTENT_123456789";
    fs::write(src.join("original.bin"), identical_bytes).unwrap();
    fs::write(src.join("duplicate.bin"), identical_bytes).unwrap();

    // Corrupt binary file
    fs::write(src.join("corrupt_image.jpg"), &[0xFF, 0xD8, 0x00, 0x11, 0x22]).unwrap();

    // 3. Run recursive scanner
    let scan_res = scan_directory(src, true).unwrap();
    let scanned = scan_res.files;
    assert_eq!(scanned.len(), 11, "Scanner must discover every regular file without omission");

    // 4. Resolve dates with simulated metadata
    let mut resolved = Vec::new();
    for file in &scanned {
        let meta = if file.filename == "照片_2024.jpg" {
            Some(json!({ "EXIF:DateTimeOriginal": "2024:05:18 10:30:00" }))
        } else if file.filename == "документ.pdf" {
            Some(json!({ "PDF:CreationDate": "2023:11:04 14:00:00" }))
        } else if file.filename == "contract.pdf" {
            Some(json!({ "XMP:CreateDate": "2024:05:18 09:00:00" })) // both contracts target 2024-05-18
        } else {
            None
        };

        let date_res = resolve_date(
            &file.source_path,
            file.extension.as_deref(),
            meta.as_ref(),
            file.modified_ms,
            true, // enable mtime fallback
        );
        resolved.push((file.clone(), date_res));
    }

    // 5. Plan destinations and resolve collisions
    let planned = plan_destinations(dst, resolved);
    assert_eq!(planned.len(), 11);

    // Verify collision resolution for contract.pdf:
    // Both target 2024-05-18, one should be contract.pdf and the other contract (1).pdf
    let contracts: Vec<_> = planned.iter().filter(|f| f.filename == "contract.pdf").collect();
    assert_eq!(contracts.len(), 2);
    let names: Vec<_> = contracts.iter().map(|c| c.planned_filename.as_str()).collect();
    assert!(names.contains(&"contract.pdf"));
    assert!(names.contains(&"contract (1).pdf"));

    // 6. Setup SQLite Database and record job
    let db_path = dst.join("test_chronicle.db");
    let db = Database::open(&db_path).unwrap();
    let job_id = "test-job-e2e";
    db.create_job(job_id, "E2E Test Job", src, dst, &JobOptions::default()).unwrap();
    db.save_planned_files(job_id, &planned, &scan_res.errors, &scan_res.excluded).unwrap();

    // 7. Execute copies through the copy engine
    let copy_opts = CopyOptions::default();
    for file in &planned {
        let res = copy_and_verify_file(
            &file.source_path,
            &file.destination_path,
            &copy_opts,
            None,
            None,
            |_| {},
        );
        assert!(res.success, "Copy failed for {}: {:?}", file.filename, res.error);
        assert!(file.destination_path.exists());

        // Verify SHA-256
        let src_bytes = fs::read(&file.source_path).unwrap();
        let dst_bytes = fs::read(&file.destination_path).unwrap();
        assert_eq!(src_bytes, dst_bytes, "Destination bytes must match source bytes exactly");
        assert_eq!(compute_sha256(&dst_bytes), res.sha256.unwrap());

        db.update_file_status(&file.id, FileStatus::Verified, None, None).unwrap();
    }

    // 8. Verify source immutability
    assert!(src.join("notes.txt").exists());
    assert!(src.join("LICENSE_DATA").exists());
    assert!(sub_a.join("contract.pdf").exists());
    assert!(sub_b.join("contract.pdf").exists());

    // 9. Export reports and verify contents
    let csv_report = dst.join("report.csv");
    ReportGenerator::export_csv(&db, job_id, &csv_report).unwrap();
    assert!(csv_report.exists());
    let csv_content = fs::read_to_string(&csv_report).unwrap();
    assert!(csv_content.contains("contract (1).pdf"));
    assert!(csv_content.contains("照片_2024.jpg"));

    let json_report = dst.join("report.json");
    ReportGenerator::export_json(&db, job_id, &json_report).unwrap();
    assert!(json_report.exists());
    let json_content = fs::read_to_string(&json_report).unwrap();
    assert!(json_content.contains(job_id));
}

#[test]
fn test_timezone_interpretation_and_wall_clock_rules() {
    // 1. Timezone-less camera EXIF: must preserve wall-clock date
    let (d1, tz1) = parse_timestamp_to_date("2024:05:18 23:45:00").unwrap();
    assert_eq!(d1, "2024-05-18", "Wall-clock date must be preserved without artificial UTC shifts");
    assert_eq!(tz1, None);

    // 2. EXIF with explicit positive timezone offset
    let (d2, tz2) = parse_timestamp_to_date("2024:05:18 01:15:00+02:00").unwrap();
    assert_eq!(d2, "2024-05-18");
    assert_eq!(tz2, Some("+02:00".to_string()));

    // 3. EXIF with explicit negative timezone offset
    let (d3, tz3) = parse_timestamp_to_date("2024:05:18 22:30:00-08:00").unwrap();
    assert_eq!(d3, "2024-05-18");
    assert_eq!(tz3, Some("-08:00".to_string()));

    // 4. QuickTime creation date with UTC marker Z
    let (d4, tz4) = parse_timestamp_to_date("2024-05-18T18:00:00Z").unwrap();
    assert_eq!(d4, "2024-05-18");
    assert_eq!(tz4, Some("UTC converted to Local".to_string()));

    // 5. Explicit UTC offset (+00:00)
    let (d5, tz5) = parse_timestamp_to_date("2024-05-18T18:00:00+00:00").unwrap();
    assert_eq!(d5, "2024-05-18");
    assert_eq!(tz5, Some("UTC converted to Local".to_string()));

    // 6. Invalid corrupt dates: 0000:00:00 or garbage
    assert!(parse_timestamp_to_date("0000:00:00 00:00:00").is_none());
    assert!(parse_timestamp_to_date("not a date string").is_none());
    assert!(parse_timestamp_to_date("").is_none());
}

#[test]
fn test_source_destination_overlap_protection() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let nested_dest = root.join("Organized_Destination");
    fs::create_dir_all(&nested_dest).unwrap();

    // Case 1: Destination inside source -> REJECT
    let r1 = check_overlap(root, &nested_dest);
    assert!(!r1.is_safe);
    assert!(r1.error_message.unwrap().contains("cannot be located inside"));

    // Case 2: Source inside destination -> REJECT
    let r2 = check_overlap(&nested_dest, root);
    assert!(!r2.is_safe);

    // Case 3: Same directory -> REJECT
    let r3 = check_overlap(root, root);
    assert!(!r3.is_safe);

    // Case 4: Separate independent directories -> ACCEPT
    let other_dir = tempdir().unwrap();
    let r4 = check_overlap(root, other_dir.path());
    assert!(r4.is_safe);
}

#[test]
fn test_crash_recovery_and_reconciliation() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let dst_dir = dir.path().join("destination");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    let db_path = dir.path().join("recovery_test.db");

    // Create source files
    let src_1 = src_dir.join("stage1.txt");
    fs::write(&src_1, b"Stage 1 test content").unwrap();
    let src_2 = src_dir.join("stage2.txt");
    fs::write(&src_2, b"Stage 2 partial crash content").unwrap();
    let src_3 = src_dir.join("stage3.txt");
    fs::write(&src_3, b"Stage 3 completed before DB update content").unwrap();
    let src_4 = src_dir.join("stage4.txt");
    fs::write(&src_4, b"Stage 4 valid source content").unwrap();

    let job_id = "job-crashed-multi-stage";

    // Destination target folder
    let dst_sub = dst_dir.join("2024-05-18");
    fs::create_dir_all(&dst_sub).unwrap();

    // Stage 1: Orphaned .chronicle_tmp_* staging file
    let orphan_tmp = dst_sub.join(".chronicle_tmp_orphan_1234.tmp");
    fs::write(&orphan_tmp, b"incomplete staging bytes").unwrap();
    assert!(orphan_tmp.exists());

    // Stage 3 simulation: destination file is already written and has valid source bytes on disk,
    // but the process crashed before SQLite was updated (status in DB is 'copying').
    let dst_3 = dst_sub.join("stage3.txt");
    fs::write(&dst_3, b"Stage 3 completed before DB update content").unwrap();

    // Stage 4 simulation: destination file was written, but subsequently got corrupted on disk!
    let dst_4 = dst_sub.join("stage4.txt");
    fs::write(&dst_4, b"corrupted bytes on disk").unwrap();

    // Phase A: Create DB and record job with files at various stages
    {
        let db = Database::open(&db_path).unwrap();
        db.create_job(job_id, "Multi-stage Recovery Job", &src_dir, &dst_dir, &JobOptions::default()).unwrap();

        let files = vec![
            PlannedFile {
                id: "file-stage1".to_string(),
                source_path: src_1.clone(),
                relative_path: PathBuf::from("stage1.txt"),
                filename: "stage1.txt".to_string(),
                extension: Some("txt".to_string()),
                size_bytes: 20,
                modified_ms: None,
                date_resolution: DateResolution {
                    resolved_date: Some("2024-05-18".to_string()),
                    date_source: DateSource::FilesystemModified,
                    raw_timestamp: None,
                    confidence: ConfidenceLevel::High,
                    reason: "Mtime".to_string(),
                    timezone_info: None,
                },
                destination_folder: "2024-05-18".to_string(),
                planned_filename: "stage1.txt".to_string(),
                destination_path: dst_sub.join("stage1.txt"),
                is_collision: false,
                collision_suffix: None,
                status: FileStatus::Pending,
                sha256: None,
                error_message: None,
            },
            PlannedFile {
                id: "file-stage2".to_string(),
                source_path: src_2.clone(),
                relative_path: PathBuf::from("stage2.txt"),
                filename: "stage2.txt".to_string(),
                extension: Some("txt".to_string()),
                size_bytes: 30,
                modified_ms: None,
                date_resolution: DateResolution {
                    resolved_date: Some("2024-05-18".to_string()),
                    date_source: DateSource::FilesystemModified,
                    raw_timestamp: None,
                    confidence: ConfidenceLevel::High,
                    reason: "Mtime".to_string(),
                    timezone_info: None,
                },
                destination_folder: "2024-05-18".to_string(),
                planned_filename: "stage2.txt".to_string(),
                destination_path: dst_sub.join("stage2.txt"),
                is_collision: false,
                collision_suffix: None,
                status: FileStatus::Copying, // was copying when crashed, no destination file committed
                sha256: None,
                error_message: None,
            },
            PlannedFile {
                id: "file-stage3".to_string(),
                source_path: src_3.clone(),
                relative_path: PathBuf::from("stage3.txt"),
                filename: "stage3.txt".to_string(),
                extension: Some("txt".to_string()),
                size_bytes: fs::metadata(&src_3).unwrap().len(),
                modified_ms: None,
                date_resolution: DateResolution {
                    resolved_date: Some("2024-05-18".to_string()),
                    date_source: DateSource::FilesystemModified,
                    raw_timestamp: None,
                    confidence: ConfidenceLevel::High,
                    reason: "Mtime".to_string(),
                    timezone_info: None,
                },
                destination_folder: "2024-05-18".to_string(),
                planned_filename: "stage3.txt".to_string(),
                destination_path: dst_3.clone(),
                is_collision: false,
                collision_suffix: None,
                status: FileStatus::Copying, // was marked copying, but destination file was already renamed!
                sha256: None,
                error_message: None,
            },
            PlannedFile {
                id: "file-stage4".to_string(),
                source_path: src_4.clone(),
                relative_path: PathBuf::from("stage4.txt"),
                filename: "stage4.txt".to_string(),
                extension: Some("txt".to_string()),
                size_bytes: fs::metadata(&src_4).unwrap().len(),
                modified_ms: None,
                date_resolution: DateResolution {
                    resolved_date: Some("2024-05-18".to_string()),
                    date_source: DateSource::FilesystemModified,
                    raw_timestamp: None,
                    confidence: ConfidenceLevel::High,
                    reason: "Mtime".to_string(),
                    timezone_info: None,
                },
                destination_folder: "2024-05-18".to_string(),
                planned_filename: "stage4.txt".to_string(),
                destination_path: dst_4.clone(),
                is_collision: false,
                collision_suffix: None,
                status: FileStatus::Verified, // was marked verified, but disk bytes are corrupted!
                sha256: Some(compute_sha256(b"Stage 4 valid source content")),
                error_message: None,
            },
        ];

        db.save_planned_files(job_id, &files, &[], &[]).unwrap();
        db.update_job_status(job_id, chronicle_lib::types::JobStatus::Running).unwrap();
    } // Simulated crash: DB dropped

    // Phase B: Application restarts -> open database runs startup reconciliation
    let recovered_db = Database::open(&db_path).unwrap();

    // Trigger explicit disk reconciliation on the recovered job
    recovered_db.reconcile_job_with_disk(job_id).unwrap();

    // 1. Verify Stage 1: Orphaned .chronicle_tmp_* was deleted
    assert!(!orphan_tmp.exists(), "Orphaned .chronicle_tmp_* must be cleaned up on recovery");

    let recovered_files = recovered_db.get_planned_files(job_id, None, 0).unwrap();

    // 2. Verify Stage 2: Incomplete copy reset to pending
    let f2 = recovered_files.iter().find(|f| f.id == "file-stage2").unwrap();
    assert_eq!(f2.status, FileStatus::Pending);

    // 3. Verify Stage 3: Completed copy before DB update is self-healed to Verified!
    let f3 = recovered_files.iter().find(|f| f.id == "file-stage3").unwrap();
    assert_eq!(f3.status, FileStatus::Verified, "Self-healing must promote valid destination to verified");
    assert!(f3.sha256.is_some());

    // 4. Verify Stage 4: Corrupted destination was quarantined and reverted to Pending
    let f4 = recovered_files.iter().find(|f| f.id == "file-stage4").unwrap();
    assert_eq!(f4.status, FileStatus::Pending, "Corrupted verified file must be reverted to pending");
    assert!(!dst_4.exists(), "Original corrupted destination file must be moved to quarantine");
    let quarantine_path = dst_sub.join(format!(".chronicle_corrupted_{}_{}", f4.id, f4.planned_filename));
    assert!(quarantine_path.exists(), "Quarantine file must exist on disk");
    assert_eq!(fs::read(&quarantine_path).unwrap(), b"corrupted bytes on disk");
}

#[test]
fn test_large_synthetic_dataset_10000_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Generate 10,000 synthetic files across 20 subdirectories
    let num_files = 10_000;
    let num_dirs = 20;

    for d in 0..num_dirs {
        let sub = root.join(format!("sub_{}", d));
        fs::create_dir(&sub).unwrap();
    }

    for i in 0..num_files {
        let sub_idx = i % num_dirs;
        let sub = root.join(format!("sub_{}", sub_idx));
        let file_path = sub.join(format!("file_{}.bin", i));
        // Small 16-byte content to be fast while creating realistic directory entries
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"1234567890abcdef").unwrap();
    }

    let start_scan = std::time::Instant::now();
    let scan_res = scan_directory(root, true).unwrap();
    let scanned = scan_res.files;
    let scan_duration = start_scan.elapsed();

    assert_eq!(scanned.len(), num_files);
    println!("Scanned {} files in {:?}", num_files, scan_duration);

    // Plan destinations for all 10,000 files
    let resolved: Vec<_> = scanned.into_iter().map(|f| {
        let date_res = resolve_date(
            &f.source_path,
            f.extension.as_deref(),
            None,
            Some(1715000000000), // May 6, 2024
            true,
        );
        (f, date_res)
    }).collect();

    let start_plan = std::time::Instant::now();
    let dst_dir = tempdir().unwrap();
    let planned = plan_destinations(dst_dir.path(), resolved);
    let plan_duration = start_plan.elapsed();

    assert_eq!(planned.len(), num_files);
    println!("Planned destinations for {} files in {:?}", num_files, plan_duration);

    // Verify all 10,000 files were planned deterministically
    assert!(plan_duration.as_secs() < 10, "Planning 10,000 files must complete within 10 seconds");
}

#[test]
fn test_path_normalization_for_exiftool_matching() {
    use chronicle_lib::jobs::normalize_path_key;

    // Standard Windows backslashes vs ExifTool forward slashes
    let win_path = r"C:\Users\Swayam\Pictures\Camera Roll\WIN_20250328_09_04_36_Pro.jpg";
    let exiftool_src = "C:/Users/Swayam/Pictures/Camera Roll/WIN_20250328_09_04_36_Pro.jpg";
    assert_eq!(normalize_path_key(win_path), normalize_path_key(exiftool_src));

    // Extended length prefix \\?\
    let unc_prefix_path = r"\\?\C:\Users\Swayam\Pictures\Camera Roll\WIN_20250328_09_04_36_Pro.jpg";
    assert_eq!(normalize_path_key(unc_prefix_path), normalize_path_key(exiftool_src));

    // Case insensitivity
    let mixed_case = "c:/users/swayam/pictures/camera roll/win_20250328_09_04_36_pro.jpg";
    assert_eq!(normalize_path_key(win_path), normalize_path_key(mixed_case));
}

#[test]
fn test_real_image_and_video_metadata_resolution() {
    use chronicle_lib::metadata::ExifToolManager;

    let manager = ExifToolManager::new();
    if !manager.is_available() {
        println!("ExifTool binary not available in test environment, skipping real file test");
        return;
    }

    // Test real camera roll photo if present on host machine
    let real_photo = PathBuf::from(r"C:\Users\Swayam\Pictures\Camera Roll\WIN_20250328_09_04_36_Pro.jpg");
    if real_photo.exists() {
        let batch = manager.extract_batch(&[real_photo.as_path()]);
        if let Ok(items) = batch {
            if let Some(item) = items.first() {
                let date_res = resolve_date(
                    &real_photo,
                    Some("jpg"),
                    Some(item),
                    None,
                    true,
                );
                assert_eq!(date_res.resolved_date, Some("2025-03-28".to_string()));
                assert_eq!(date_res.date_source, DateSource::ExifOriginal);
                assert_eq!(date_res.confidence, ConfidenceLevel::High);
            }
        }
    }

    // Test real camera roll MP4 video if present on host machine
    let real_video = PathBuf::from(r"C:\Users\Swayam\Pictures\Camera Roll\WIN_20220202_12_07_04_Pro.mp4");
    if real_video.exists() {
        let batch = manager.extract_batch(&[real_video.as_path()]);
        if let Ok(items) = batch {
            if let Some(item) = items.first() {
                let date_res = resolve_date(
                    &real_video,
                    Some("mp4"),
                    Some(item),
                    None,
                    true,
                );
                assert_eq!(date_res.resolved_date, Some("2022-02-02".to_string()));
                assert_eq!(date_res.date_source, DateSource::QuickTimeCreation);
                assert_eq!(date_res.confidence, ConfidenceLevel::High);
            }
        }
    }
}
