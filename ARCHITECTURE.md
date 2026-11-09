# Chronicle Architecture Specification

## 1. System Overview

Chronicle is an offline-first Windows desktop application built with **Tauri 2**, **Rust**, and **React 18 + TypeScript + Tailwind CSS**. It provides deterministic, non-destructive file organization across arbitrary file types and extensions.

```
+-------------------------------------------------------------------------+
|                         React / TypeScript UI                           |
|  [Dashboard]      [Jobs & Preview]      [Execution Monitor]  [Settings] |
|                                                                         |
|  State Management (Zustand / Event Listeners) / Virtualized Tables     |
+-------------------------------------------------------------------------+
                                    |
                           Tauri IPC (Typed JSON)
                                    |
+-------------------------------------------------------------------------+
|                           Tauri 2 Rust Backend                          |
|                                                                         |
|  +---------------------+   +---------------------+   +---------------+  |
|  |   Scanner Engine    |   |    Date Resolver    |   |  Copy Engine  |  |
|  | - Recursive walk    |   | - ExifTool batch    |   | - Stream copy |  |
|  | - Symlink/junction  |   | - Tag hierarchy     |   | - SHA-256 ver |  |
|  | - Long path support |   | - Mtime fallback    |   | - Temp rename |  |
|  +---------------------+   +---------------------+   +---------------+  |
|             \                         |                     /           |
|              v                        v                    v            |
|       +-------------------------------------------------------+         |
|       |               Job Coordinator & SQLite DB             |         |
|       | - Jobs & Files Tables  - Progress Events              |         |
|       | - Crash Reconciliation - Pause / Resume / Cancel      |         |
|       +-------------------------------------------------------+         |
+-------------------------------------------------------------------------+
       |                                                 |
       v                                                 v
+---------------+                               +-------------------------+
|  ExifTool CLI |                               | Windows Filesystem      |
|  (Bundled)    |                               | Source (RO) -> Dest (W) |
+---------------+                               +-------------------------+
```

---

## 2. Core Subsystems

### 2.1 Scanner Engine (`src-tauri/src/scanner/`)
- **Universal Traversal**: Enumerates all directory entries recursively using `walkdir`.
- **Zero Silent Drops**: Unreadable directory entries and files whose filesystem metadata cannot be read are never omitted. Traversal errors are captured as `ScanErrorItem` and returned in `ScanResult { files, errors }`. The job coordinator persists them into SQLite `job_files` with `status: 'failed'` and destination `"Scan Errors"`, satisfying the accountability formula:
  $$\text{Discovered} = \text{Verified} + \text{Failed} + \text{Cancelled} + \text{Excluded}$$
- **Safety Checks**:
  - Validates that source path exists and is a directory.
  - Validates destination path. Rejects if `source == destination` or if `destination` is nested inside `source`.
  - Canonicalizes paths using `dunce` to eliminate Windows extended-length prefixes (`\\?\`) where appropriate while supporting long paths (>260 chars).
  - **Symlinks & Junctions**: Checks file attributes for `FILE_ATTRIBUTE_REPARSE_POINT`. Skips following reparse points by default to avoid directory recursion loops; records skipped symlinks as informational logs.
  - Zero-byte files, hidden files, read-only files, and extensionless files are preserved and included.

### 2.2 Date Resolver (`src-tauri/src/date_resolver/`)
Resolves a deterministic calendar date (`YYYY-MM-DD`) and timestamp for each file.

#### Priority Hierarchy:
1. **Semantic Primary Creation / Capture Metadata (ExifTool)**:
   - Photos / Images: `DateTimeOriginal`, `SubSecDateTimeOriginal`, `OffsetTimeOriginal`.
   - Videos: `QuickTime:CreateDate`, `QuickTime:CreationDate`, `QuickTime:TrackCreateDate` (with timezone offset normalization).
   - Audio: `ID3:RecordingTime`, `ID3:Year`, `FLAC:Date`, `Vorbis:Date`.
   - Documents: `PDF:CreationDate`, `XMP:CreateDate`, `Office:CreationDate`.
   - Extensionless files are inspected without extension filtering.
2. **Semantic Secondary Embedded Metadata**:
   - `MediaCreateDate`, `ContentCreateDate`, `HistoryWhen`.
   - Explicitly rejects `ModifyDate` or generic file tags when seeking primary capture, unless configured.
3. **Filesystem Last-Modified Timestamp (Fallback)**:
   - Used when enabled in job settings if embedded metadata is absent, unreadable, or invalid.
   - Evaluated in the user's local timezone (`chrono::Local`) rather than naive UTC, preventing date shifts near midnight (e.g. UTC+05:30 IST).
4. **No Date**:
   - Files with missing metadata and disabled/absent modified dates are routed to the `No Date/` folder.

#### Timezone & Normalization Rules:
- EXIF dates lacking timezone offsets are preserved in local wall-clock time as recorded by the capturing device (no arbitrary UTC shifts).
- QuickTime tags stored in UTC ('Z') are explicitly converted to local calendar dates matching Windows Explorer expectations.
- Result payload captures: `resolved_date` (`YYYY-MM-DD`), `date_source` (e.g. `Exif:DateTimeOriginal`, `Filesystem:Modified`, `None`), `raw_timestamp`, `confidence_level`, and `rationale`.

### 2.3 Collision & Destination Planner (`src-tauri/src/organizer/`)
- Computes target paths: `<Destination>/<YYYY-MM-DD>/<OriginalFilename>.<ext>` or `<Destination>/No Date/<OriginalFilename>.<ext>`.
- **Deterministic Collision Resolution**:
  - In-memory and database-backed collision tracking.
  - If a file `photo.jpg` targets a date folder that already has `photo.jpg` planned or existing on disk:
    - First duplicate candidate becomes `photo (1).jpg`.
    - Subsequent become `photo (2).jpg`, `photo (3).jpg`, etc.
  - Identical content (matching SHA-256) is **never** skipped. Both copies are preserved with distinct suffixed names.

### 2.4 Copy Engine (`src-tauri/src/copy_engine/`)
- **Zero In-Memory Buffering**: Streams files in 64 KB chunks via buffered reads and writes.
- **Two-Phase Commit & Independent Physical Verification**:
  1. Stream copy into temporary destination file: `.chronicle_tmp_<uuid>_<filename>`.
  2. Incremental SHA-256 calculation on source stream.
  3. Mid-stream cooperative pause (`pause_flag`) and cancellation checks.
  4. Physical disk flush via `dst_file.sync_all()` to guarantee physical storage persistence before validation.
  5. **Independent Destination Hash Verification**: Re-opens temporary destination file from disk, computes SHA-256 independently, and verifies `src_hash == dst_hash`.
  6. **Kernel-Level Atomic No-Overwrite Commit**: Commits via `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` (returning `ERROR_ALREADY_EXISTS` on collision), guaranteeing absolute race immunity against concurrent creation.
- **Cancellation & Cleanup**:
  - Incomplete `.chronicle_tmp_*` files are deleted immediately if copy fails or user cancels.
- **Source Verification**:
  - Records source file size and modified timestamp before copy; verifies source was not altered during reading.

### 2.5 Job Coordinator & SQLite Persistence (`src-tauri/src/database/`)
- SQLite database stored at `%APPDATA%/Chronicle/chronicle.db` (or application data directory).
- Uses WAL (Write-Ahead Logging) mode and busy timeout for concurrent safety.
- **Schema**:
  - `jobs`: Job ID, source path, destination path, options JSON, status (`discovered`, `analysed`, `running`, `paused`, `completed`, `cancelled`, `failed`), timestamps, metrics, error count, excluded count.
  - `job_files`: File ID, job ID, source path, relative path, file size, source mtime, status (`discovered`, `analysed`, `pending`, `copying`, `verified`, `failed`, `cancelled`, `excluded`), resolved date, date source, planned filename, destination path, sha256, error message, attempt count.
- **Multi-Stage Crash Recovery & Self-Healing**:
  - Cleans up orphaned `.chronicle_tmp_*` staging files across all destination directories.
  - Files completed before process interruption self-heal from `copying` to `verified` after validating physical size and SHA-256.
  - Corrupted destination files on disk are safely moved to `.chronicle_corrupted_*` and their database records reset to `pending` so retry can place a clean copy at the planned path.
  - Fully persistent history across application restarts via `get_all_jobs`.

---

## 3. Concurrency Model

- **Metadata Extraction Worker Pool**:
  - Bounded concurrency using `tokio::sync::Semaphore` (default: `num_cpus` or user-configured 2–8 workers).
  - Batched ExifTool execution: invokes ExifTool with JSON output format (`-j -charset filename=utf8`) over batches of files (e.g. 100 files per batch) to avoid process spawn overhead while keeping memory bounded.
- **Copy Worker Pool**:
  - Bounded copy workers (default: 2–4 workers) to prevent disk I/O thrashing on mechanical drives while saturating NVMe SSDs.
- **Event Streaming**:
  - Rust emits throttled progress events to Tauri frontend (e.g. at most every 50ms or per 10 files) containing aggregate bytes copied, files completed, and current file name.

---

## 4. Safety & Security Guarantees

1. **Source Immutability**: Open source files with read-only sharing flags (`FILE_SHARE_READ`).
2. **Disk Space Pre-flight**: Query destination volume available space before starting copy. Abort with a clear error if total required space exceeds available space.
3. **Graceful Cancellation**: Cooperative token checking between file chunks and before each file copy. Completed files remain intact; partial temporary files are cleanly removed.
4. **Data Isolation**: 100% offline, zero internet outbound calls, zero third-party telemetry.
