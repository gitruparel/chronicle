use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime};
use serde_json::Value;
use std::path::Path;
use crate::types::{ConfidenceLevel, DateResolution, DateSource};

/// Parses various timestamp formats into a validated YYYY-MM-DD string and timezone description.
/// Handles:
/// - EXIF local wall-clock: "YYYY:MM:DD HH:MM:SS" (preserved directly without UTC distortion)
/// - Explicit timezone offset: "YYYY:MM:DD HH:MM:SS+HH:MM" or ISO 8601
/// - UTC timestamps with Z: converted to local calendar day
pub fn parse_timestamp_to_date(raw: &str) -> Option<(String, Option<String>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("0000:00:00") || trimmed.starts_with("0000-00-00") {
        return None;
    }

    // 1. If timestamp has UTC indicator 'Z', parse as UTC and convert to local calendar date
    if trimmed.ends_with('Z') || trimmed.ends_with('z') {
        if let Ok(utc_dt) = DateTime::parse_from_rfc3339(trimmed) {
            let local_dt = utc_dt.with_timezone(&Local);
            let year = local_dt.year();
            if (1970..=2100).contains(&year) {
                return Some((local_dt.format("%Y-%m-%d").to_string(), Some("UTC converted to Local".to_string())));
            }
        }
    }

    // 2. Check for explicit timezone offset in string (e.g. "+05:30", "-08:00")
    let mut tz_info = None;
    if let Some(plus_idx) = trimmed.rfind('+') {
        if plus_idx > 10 {
            tz_info = Some(trimmed[plus_idx..].to_string());
        }
    } else if let Some(minus_idx) = trimmed.rfind('-') {
        if minus_idx > 10 {
            tz_info = Some(trimmed[minus_idx..].to_string());
        }
    }

    // 3. Try RFC 3339 / ISO 8601 with offset
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        let year = dt.year();
        if (1970..=2100).contains(&year) {
            return Some((dt.format("%Y-%m-%d").to_string(), Some(dt.offset().to_string())));
        }
    }

    // 4. Standard EXIF format: "YYYY:MM:DD HH:MM:SS"
    // Preserves local wall-clock date recorded by camera
    if trimmed.len() >= 10 {
        let date_part = &trimmed[0..10];
        let normalized = date_part.replace(':', "-");
        if let Ok(naive_date) = NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
            let year = naive_date.year();
            if (1970..=2100).contains(&year) {
                return Some((naive_date.format("%Y-%m-%d").to_string(), tz_info));
            }
        }
    }

    // 5. Try NaiveDateTime formats
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

/// Converts a UTC timestamp into the user's local calendar date (YYYY-MM-DD).
/// This prevents midnight boundary errors where files created shortly after midnight
/// would be assigned to the previous day if evaluated in UTC.
pub fn format_timestamp_to_local_date(secs: i64) -> Option<String> {
    DateTime::from_timestamp(secs, 0)
        .map(|utc_dt| {
            let local_dt = utc_dt.with_timezone(&Local);
            local_dt.format("%Y-%m-%d").to_string()
        })
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
/// 3. Filesystem last-modified fallback (converted to local calendar date)
/// 4. No Date fallback
///
/// Universal: Does NOT skip metadata tags if a file lacks an extension.
pub fn resolve_date(
    _file_path: &Path,
    _extension: Option<&str>,
    metadata_json: Option<&Value>,
    modified_ms: Option<i64>,
    mtime_fallback_enabled: bool,
) -> DateResolution {
    if let Some(meta) = metadata_json {
        // TIER 1: Primary capture/creation timestamps
        // Ordered strictly by specificity of original capture
        let primary_capture_tags = [
            // EXIF Camera Capture (photos, raw, any image format)
            "EXIF:SubSecDateTimeOriginal",
            "EXIF:DateTimeOriginal",
            "XMP:DateTimeOriginal",
            "XMP:DateCreated",
            "DateTimeOriginal",
            // QuickTime / Video Creation
            "QuickTime:CreationDate", // Has timezone offset
            "QuickTime:CreateDate",   // Standard QuickTime UTC
            "QuickTime:TrackCreateDate",
            "Matroska:DateUTC",
            // Audio Recording / Release
            "ID3:RecordingTime",
            "ID3:Year",
            "FLAC:Date",
            "Vorbis:Date",
            "RecordingTime",
            // Documents
            "PDF:CreationDate",
            "XMP:CreateDate",
            "Office:CreationDate",
            "CreationDate",
        ];

        if let Some((tag, val)) = find_tag_value(meta, &primary_capture_tags) {
            if let Some((date_str, tz)) = parse_timestamp_to_date(val) {
                let date_source = if tag.contains("QuickTime") || tag.contains("Matroska") {
                    DateSource::QuickTimeCreation
                } else if tag.contains("EXIF") || tag.contains("DateTimeOriginal") {
                    DateSource::ExifOriginal
                } else if tag.contains("ID3") || tag.contains("FLAC") || tag.contains("PDF") || tag.contains("Office") {
                    DateSource::EmbeddedDocAudio
                } else {
                    DateSource::EmbeddedOther
                };

                return DateResolution {
                    resolved_date: Some(date_str),
                    date_source,
                    raw_timestamp: Some(val.to_string()),
                    confidence: ConfidenceLevel::High,
                    reason: format!("Resolved from primary capture tag [{}]", tag),
                    timezone_info: tz,
                };
            }
        }

        // TIER 2: Secondary embedded creation timestamps
        let secondary_tags = [
            "Composite:SubSecCreateDate",
            "EXIF:CreateDate",
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

    // TIER 3: Filesystem Modified Timestamp Fallback (in LOCAL calendar time)
    if mtime_fallback_enabled {
        if let Some(ms) = modified_ms {
            let secs = ms / 1000;
            if let Some(date_str) = format_timestamp_to_local_date(secs) {
                return DateResolution {
                    resolved_date: Some(date_str),
                    date_source: DateSource::FilesystemModified,
                    raw_timestamp: Some(format!("Unix mtime: {} ms", ms)),
                    confidence: ConfidenceLevel::Fallback,
                    reason: "Resolved from filesystem last-modified timestamp in local time".to_string(),
                    timezone_info: Some(Local::now().offset().to_string()),
                };
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

        assert!(parse_timestamp_to_date("0000:00:00 00:00:00").is_none());
        assert!(parse_timestamp_to_date("1905:01:01 00:00:00").is_none());
    }

    #[test]
    fn test_extensionless_file_metadata_inspection() {
        // Universal file handling: Extensionless file with embedded EXIF or QuickTime metadata
        let path = PathBuf::from("unnamed_binary_blob");
        let meta = json!({
            "EXIF:DateTimeOriginal": "2024:05:18 11:22:33"
        });

        let res = resolve_date(&path, None, Some(&meta), None, true);
        assert_eq!(res.resolved_date, Some("2024-05-18".to_string()));
        assert_eq!(res.date_source, DateSource::ExifOriginal);
        assert_eq!(res.confidence, ConfidenceLevel::High);
    }

    #[test]
    fn test_midnight_timezone_boundary_conversion() {
        // Test timestamps near midnight
        // UTC 2024-05-18 23:45:00 converted to IST (UTC+05:30) is 2024-05-19 05:15:00
        let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
        let utc_dt = chrono::DateTime::parse_from_rfc3339("2024-05-18T23:45:00Z").unwrap();
        let ist_dt = utc_dt.with_timezone(&ist_offset);
        assert_eq!(ist_dt.format("%Y-%m-%d").to_string(), "2024-05-19");

        // UTC 2024-05-18 01:15:00 converted to PST (UTC-08:00) is 2024-05-17 17:15:00
        let pst_offset = chrono::FixedOffset::west_opt(8 * 3600).unwrap();
        let pst_dt = utc_dt.with_timezone(&pst_offset);
        assert_eq!(pst_dt.format("%Y-%m-%d").to_string(), "2024-05-18");
    }

    #[test]
    fn test_resolve_fallback_to_local_mtime() {
        let path = PathBuf::from("archive.zip");
        let res = resolve_date(&path, Some("zip"), None, Some(1715000000000), true);
        assert!(res.resolved_date.is_some());
        assert_eq!(res.date_source, DateSource::FilesystemModified);
        assert_eq!(res.confidence, ConfidenceLevel::Fallback);
    }
}
