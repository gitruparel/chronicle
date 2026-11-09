use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use serde_json::Value;
use std::path::Path;
use crate::types::{ConfidenceLevel, DateResolution, DateSource};

/// Parses various timestamp formats into a validated YYYY-MM-DD string and components.
pub fn parse_timestamp_to_date(raw: &str) -> Option<(String, Option<String>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("0000:00:00") || trimmed.starts_with("0000-00-00") {
        return None;
    }

    // Check for timezone offset in the string (e.g. "+02:00", "-05:00", "Z")
    let mut tz_info = None;
    if trimmed.ends_with('Z') || trimmed.ends_with('z') {
        tz_info = Some("UTC (Z)".to_string());
    } else if let Some(plus_idx) = trimmed.rfind('+') {
        if plus_idx > 10 {
            tz_info = Some(trimmed[plus_idx..].to_string());
        }
    } else if let Some(minus_idx) = trimmed.rfind('-') {
        // Ensure this isn't part of the YYYY-MM-DD date separator
        if minus_idx > 10 {
            tz_info = Some(trimmed[minus_idx..].to_string());
        }
    }

    // Try standard EXIF format: "YYYY:MM:DD HH:MM:SS..."
    if trimmed.len() >= 10 {
        let date_part = &trimmed[0..10];
        let normalized = date_part.replace(':', "-");
        if let Ok(naive_date) = NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
            let year = naive_date.year();
            // Sanity check year bounds
            if (1970..=2100).contains(&year) {
                return Some((naive_date.format("%Y-%m-%d").to_string(), tz_info));
            }
        }
    }

    // Try RFC 3339 / ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        let year = dt.year();
        if (1970..=2100).contains(&year) {
            return Some((dt.format("%Y-%m-%d").to_string(), Some(dt.offset().to_string())));
        }
    }

    // Try NaiveDateTime formats
    let formats = [
        "%Y:%m:%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y:%m:%d",
        "%Y-%m-%d",
    ];

    for fmt in &formats {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            let year = ndt.year();
            if (1970..=2100).contains(&year) {
                return Some((ndt.format("%Y-%m-%d").to_string(), tz_info));
            }
        }
    }

    None
}

/// Helper to get a string value from an ExifTool JSON map using multiple candidate tag keys
fn find_tag_value<'a>(metadata: &'a Value, candidate_keys: &[&'static str]) -> Option<(&'static str, &'a str)> {
    if let Value::Object(map) = metadata {
        for &key in candidate_keys {
            if let Some(val) = map.get(key) {
                if let Some(s) = val.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("0000:00:00") {
                        return Some((key, trimmed));
                    }
                }
            }
        }
    }
    None
}

/// Deterministically resolves the most appropriate date for a file based on:
/// 1. Primary semantic capture/creation tags (EXIF, QuickTime, Audio, Doc)
/// 2. Secondary embedded creation timestamps
/// 3. Filesystem last-modified fallback (if enabled)
/// 4. No Date fallback
pub fn resolve_date(
    _file_path: &Path,
    extension: Option<&str>,
    metadata_json: Option<&Value>,
    modified_ms: Option<i64>,
    mtime_fallback_enabled: bool,
) -> DateResolution {
    let ext = extension.map(|e| e.to_lowercase()).unwrap_or_default();

    if let Some(meta) = metadata_json {
        // TIER 1: Primary capture/creation timestamps
        // Photos / Images
        let photo_primary_tags = [
            "EXIF:SubSecDateTimeOriginal",
            "EXIF:DateTimeOriginal",
            "XMP:DateTimeOriginal",
            "XMP:DateCreated",
            "DateTimeOriginal",
        ];

        // Videos
        let video_primary_tags = [
            "QuickTime:CreationDate",
            "QuickTime:CreateDate",
            "QuickTime:TrackCreateDate",
            "Matroska:DateUTC",
            "CreationDate",
        ];

        // Audio
        let audio_primary_tags = [
            "ID3:RecordingTime",
            "ID3:Year",
            "FLAC:Date",
            "Vorbis:Date",
            "RecordingTime",
        ];

        // Documents
        let doc_primary_tags = [
            "PDF:CreationDate",
            "XMP:CreateDate",
            "Office:CreationDate",
            "CreationDate",
        ];

        // Determine file category to prioritize semantic tags
        let is_video = matches!(ext.as_str(), "mov" | "mp4" | "m4v" | "mkv" | "avi" | "wmv" | "flv" | "webm" | "3gp");
        let is_audio = matches!(ext.as_str(), "mp3" | "flac" | "wav" | "m4a" | "aac" | "ogg" | "wma");
        let is_doc = matches!(ext.as_str(), "pdf" | "docx" | "xlsx" | "pptx" | "doc" | "xls" | "ppt" | "odt");
        let is_image = matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "heic" | "heif" | "webp" | "tiff" | "tif" | "dng" | "cr2" | "cr3" | "nef" | "arw");

        // 1. Try category-specific primary tags first
        if is_image {
            if let Some((tag, val)) = find_tag_value(meta, &photo_primary_tags) {
                if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                    return DateResolution {
                        resolved_date: Some(date_str),
                        date_source: DateSource::ExifOriginal,
                        raw_timestamp: Some(val.to_string()),
                        confidence: ConfidenceLevel::High,
                        reason: format!("Resolved from primary image capture tag [{}]", tag),
                        timezone_info: tz,
                    };
                }
            }
        } else if is_video {
            if let Some((tag, val)) = find_tag_value(meta, &video_primary_tags) {
                if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                    return DateResolution {
                        resolved_date: Some(date_str),
                        date_source: DateSource::QuickTimeCreation,
                        raw_timestamp: Some(val.to_string()),
                        confidence: ConfidenceLevel::High,
                        reason: format!("Resolved from video creation tag [{}]", tag),
                        timezone_info: tz,
                    };
                }
            }
        } else if is_audio {
            if let Some((tag, val)) = find_tag_value(meta, &audio_primary_tags) {
                if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                    return DateResolution {
                        resolved_date: Some(date_str),
                        date_source: DateSource::EmbeddedDocAudio,
                        raw_timestamp: Some(val.to_string()),
                        confidence: ConfidenceLevel::High,
                        reason: format!("Resolved from audio recording tag [{}]", tag),
                        timezone_info: tz,
                    };
                }
            }
        } else if is_doc {
            if let Some((tag, val)) = find_tag_value(meta, &doc_primary_tags) {
                if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                    return DateResolution {
                        resolved_date: Some(date_str),
                        date_source: DateSource::EmbeddedDocAudio,
                        raw_timestamp: Some(val.to_string()),
                        confidence: ConfidenceLevel::High,
                        reason: format!("Resolved from document creation tag [{}]", tag),
                        timezone_info: tz,
                    };
                }
            }
        }

        // Try general primary tags across any file type if not matched above
        let general_primary_tags = [
            "EXIF:SubSecDateTimeOriginal",
            "EXIF:DateTimeOriginal",
            "QuickTime:CreationDate",
            "QuickTime:CreateDate",
            "XMP:DateTimeOriginal",
            "XMP:DateCreated",
            "PDF:CreationDate",
            "ID3:RecordingTime",
        ];
        if let Some((tag, val)) = find_tag_value(meta, &general_primary_tags) {
            if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                return DateResolution {
                    resolved_date: Some(date_str),
                    date_source: DateSource::ExifOriginal,
                    raw_timestamp: Some(val.to_string()),
                    confidence: ConfidenceLevel::Medium,
                    reason: format!("Resolved from embedded primary timestamp [{}]", tag),
                    timezone_info: tz,
                };
            }
        }

        // TIER 2: Secondary embedded creation timestamps
        let secondary_tags = [
            "Composite:SubSecCreateDate",
            "EXIF:CreateDate",
            "QuickTime:TrackCreateDate",
            "MediaCreateDate",
            "ContentCreateDate",
            "CreateDate",
        ];
        if let Some((tag, val)) = find_tag_value(meta, &secondary_tags) {
            if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                return DateResolution {
                    resolved_date: Some(date_str),
                    date_source: DateSource::EmbeddedOther,
                    raw_timestamp: Some(val.to_string()),
                    confidence: ConfidenceLevel::Medium,
                    reason: format!("Resolved from secondary embedded timestamp [{}]", tag),
                    timezone_info: tz,
                };
            }
        }
    }

    // TIER 3: Filesystem Modified Timestamp Fallback
    if mtime_fallback_enabled {
        if let Some(ms) = modified_ms {
            let secs = ms / 1000;
            if let Some(dt) = DateTime::from_timestamp(secs, 0) {
                let date_str = dt.format("%Y-%m-%d").to_string();
                let year = dt.year();
                if (1970..=2100).contains(&year) {
                    return DateResolution {
                        resolved_date: Some(date_str),
                        date_source: DateSource::FilesystemModified,
                        raw_timestamp: Some(dt.to_rfc3339()),
                        confidence: ConfidenceLevel::Fallback,
                        reason: "Resolved from filesystem last-modified timestamp (no embedded date)".to_string(),
                        timezone_info: Some("UTC/Filesystem".to_string()),
                    };
                }
            }
        }
    }

    // TIER 4: No Date
    DateResolution {
        resolved_date: None,
        date_source: DateSource::None,
        raw_timestamp: None,
        confidence: ConfidenceLevel::Fallback,
        reason: if mtime_fallback_enabled {
            "No valid embedded metadata or filesystem modified date found".to_string()
        } else {
            "No embedded date found (filesystem fallback disabled)".to_string()
        },
        timezone_info: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_parse_timestamp_formats() {
        let (d1, tz1) = parse_timestamp_to_date("2024:05:18 14:30:00").unwrap();
        assert_eq!(d1, "2024-05-18");
        assert_eq!(tz1, None);

        let (d2, tz2) = parse_timestamp_to_date("2023:11:04 09:15:22+02:00").unwrap();
        assert_eq!(d2, "2023-11-04");
        assert_eq!(tz2, Some("+02:00".to_string()));

        let (d3, tz3) = parse_timestamp_to_date("2022-08-15T12:00:00Z").unwrap();
        assert_eq!(d3, "2022-08-15");
        assert_eq!(tz3, Some("UTC (Z)".to_string()));

        assert!(parse_timestamp_to_date("0000:00:00 00:00:00").is_none());
        assert!(parse_timestamp_to_date("1905:01:01 00:00:00").is_none());
    }

    #[test]
    fn test_resolve_photo_priority() {
        let path = PathBuf::from("photo.jpg");
        let meta = json!({
            "EXIF:DateTimeOriginal": "2024:05:18 14:20:00",
            "EXIF:CreateDate": "2024:05:19 10:00:00",
            "File:FileModifyDate": "2024:06:01 08:00:00"
        });

        let res = resolve_date(&path, Some("jpg"), Some(&meta), None, true);
        assert_eq!(res.resolved_date, Some("2024-05-18".to_string()));
        assert_eq!(res.date_source, DateSource::ExifOriginal);
        assert_eq!(res.confidence, ConfidenceLevel::High);
    }

    #[test]
    fn test_resolve_video_priority() {
        let path = PathBuf::from("video.mov");
        let meta = json!({
            "QuickTime:CreationDate": "2023:12:25 18:00:00+01:00",
            "QuickTime:CreateDate": "2023:12:25 17:00:00"
        });

        let res = resolve_date(&path, Some("mov"), Some(&meta), None, true);
        assert_eq!(res.resolved_date, Some("2023-12-25".to_string()));
        assert_eq!(res.date_source, DateSource::QuickTimeCreation);
        assert_eq!(res.timezone_info, Some("+01:00".to_string()));
    }

    #[test]
    fn test_resolve_fallback_to_mtime() {
        let path = PathBuf::from("archive.zip");
        let res = resolve_date(&path, Some("zip"), None, Some(1715000000000), true);
        assert_eq!(res.resolved_date, Some("2024-05-06".to_string()));
        assert_eq!(res.date_source, DateSource::FilesystemModified);
        assert_eq!(res.confidence, ConfidenceLevel::Fallback);
    }

    #[test]
    fn test_resolve_no_date_when_fallback_disabled() {
        let path = PathBuf::from("unknown.bin");
        let res = resolve_date(&path, Some("bin"), None, Some(1715000000000), false);
        assert_eq!(res.resolved_date, None);
        assert_eq!(res.date_source, DateSource::None);
    }
}
