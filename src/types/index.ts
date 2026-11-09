export type JobStatus =
  | 'created'
  | 'scanning'
  | 'scanned'
  | 'analysing'
  | 'analysed'
  | 'running'
  | 'paused'
  | 'completed'
  | 'cancelled'
  | 'failed';

export type FileStatus =
  | 'discovered'
  | 'analysed'
  | 'pending'
  | 'copying'
  | 'verified'
  | 'failed'
  | 'skipped';

export type DateSource =
  | 'exif_original'
  | 'quicktime_creation'
  | 'embedded_doc_audio'
  | 'embedded_other'
  | 'filesystem_modified'
  | 'none';

export type ConfidenceLevel = 'high' | 'medium' | 'low' | 'fallback';

export interface DateResolution {
  resolved_date: string | null;
  date_source: DateSource;
  raw_timestamp: string | null;
  confidence: ConfidenceLevel;
  reason: string;
  timezone_info: string | null;
}

export interface PlannedFile {
  id: string;
  source_path: string;
  relative_path: string;
  filename: string;
  extension: string | null;
  size_bytes: number;
  modified_ms: number | null;
  date_resolution: DateResolution;
  destination_folder: string;
  planned_filename: string;
  destination_path: string;
  is_collision: boolean;
  collision_suffix: number | null;
  status: FileStatus;
  sha256: string | null;
  error_message: string | null;
}

export interface JobOptions {
  recursive: boolean;
  copy_duplicates: boolean;
  mtime_fallback: boolean;
  verify_hash: boolean;
  worker_count: number;
}

export interface JobSummary {
  total_files: number;
  total_bytes: number;
  embedded_date_count: number;
  mtime_date_count: number;
  no_date_count: number;
  metadata_error_count: number;
  duplicate_content_count: number;
  unique_folders_count: number;
}

export interface JobProgress {
  job_id: string;
  status: JobStatus;
  total_files: number;
  total_bytes: number;
  copied_files: number;
  copied_bytes: number;
  failed_files: number;
  current_file: string | null;
  current_operation: string;
  speed_bytes_per_sec: number;
  eta_seconds: number | null;
}

export interface PathValidationResult {
  is_valid: boolean;
  canonical_path: string | null;
  exists: boolean;
  is_directory: boolean;
  available_bytes: number | null;
  error_message: string | null;
}

export interface OverlapCheckResult {
  is_safe: boolean;
  error_message: string | null;
}

export interface CreateJobResponse {
  job_id: string;
  summary: JobSummary;
}

export interface PaginatedFilesResponse {
  files: PlannedFile[];
  total: number;
  page: number;
  page_size: number;
}

export interface AppInfoResponse {
  version: string;
  exiftool_available: boolean;
  exiftool_path: string | null;
}
