# Chronicle Implementation Plan & Task Checklist

## Phase 1: Environment & Project Scaffolding
- [x] 1.1 Verify Node, Rust, Cargo, and MSVC toolchains in workspace.
- [x] 1.2 Scaffold Tauri 2 configuration (`tauri.conf.json`, `capabilities/default.json`, `Cargo.toml`).
- [x] 1.3 Scaffold React 18 + TypeScript + Vite + Tailwind CSS frontend (`package.json`, `vite.config.ts`, `tailwind.config.js`).
- [x] 1.4 Integrate Lucide React icons and design system components.
- [x] 1.5 Verify clean build of frontend and backend (`npm run build`, `cargo check`).
- [x] 1.6 Integrate native Windows folder selection via `@tauri-apps/plugin-dialog` and backend fallback.

## Phase 2: Recursive Scanner & Universal Enumeration
- [x] 2.1 Implement `src-tauri/src/scanner/` with path canonicalization and safety validation (reject source == destination or destination inside source).
- [x] 2.2 Implement recursive file traversal skipping symlinks/reparse points to avoid loops.
- [x] 2.3 Collect file attributes (size, mtime, extension, readonly flag).
- [x] 2.4 Handle zero-byte, hidden, extensionless, and Unicode files.
- [x] 2.5 Unit tests for scanner (including directory loops and overlap protection).

## Phase 3: Metadata Extraction & Deterministic Date Resolver
- [x] 3.1 Bundle ExifTool binary and license notices in `src-tauri/resources/`.
- [x] 3.2 Implement batched ExifTool execution (`-j -charset filename=utf8`).
- [x] 3.3 Implement `src-tauri/src/date_resolver/` priority hierarchy:
  - Primary capture/creation timestamps (EXIF DateTimeOriginal, QuickTime creation, audio/document creation).
  - Secondary creation timestamps.
  - Filesystem last-modified fallback.
  - No Date fallback.
- [x] 3.4 Preserve raw timestamp, selected tag, timezone offset, and confidence level.
- [x] 3.5 Unit and integration tests for date resolver with diverse file formats.

## Phase 4: Collision Resolution & Destination Planner
- [x] 4.1 Implement `src-tauri/src/organizer/` to calculate target folder (`YYYY-MM-DD` or `No Date`).
- [x] 4.2 Implement deterministic collision resolution (`photo (1).jpg`, `photo (2).jpg`) against both existing destination files and planned files.
- [x] 4.3 Ensure duplicate content (identical SHA-256) is never skipped or overwritten.
- [x] 4.4 Expose preview data structure with collision annotations.
- [x] 4.5 Unit tests for collision resolution and path planning.

## Phase 5: Streaming Copy Engine & Verification
- [x] 5.1 Implement chunked streaming copy (`src-tauri/src/copy_engine/`) with zero full-file RAM buffering.
- [x] 5.2 Simultaneous SHA-256 calculation during copy.
- [x] 5.3 Two-phase commit: stream to `.chronicle_tmp_*`, verify size + SHA-256, atomic rename.
- [x] 5.4 Pre-flight destination disk space check.
- [x] 5.5 Source file immutability and change detection (mtime/size check before/after).
- [x] 5.6 Graceful cancellation and cleanup of temporary files.
- [x] 5.7 Unit and integration tests for copy engine and verification.

## Phase 6: SQLite Job Engine & Recovery
- [x] 6.1 Implement `src-tauri/src/database/` with migrations, WAL mode, and busy timeout.
- [x] 6.2 Schema for `jobs` and `job_files` tracking states (`discovered`, `analysed`, `pending`, `copying`, `verified`, `failed`, `cancelled`).
- [x] 6.3 State transitions and transaction wrappers.
- [x] 6.4 Cooperative cancellation, pause, and resume.
- [x] 6.5 Crash recovery: reconcile database records with destination filesystem on startup.
- [x] 6.6 Retry mechanism for failed files.
- [x] 6.7 Integration tests for job persistence, crash simulation, and recovery.

## Phase 7: UI Implementation & User Workflows
- [x] 7.1 Linear/Vercel-inspired dark/light theme UI shell (Header, Navigation tabs, Status bar).
- [x] 7.2 Navigation: Dashboard / New Job, Jobs, History, Settings.
- [x] 7.3 New Job screen:
  - Source & destination pickers.
  - Configurable options (recursive, copy duplicates, mtime fallback, verification).
- [x] 7.4 Scan Results / Preview screen:
  - Metric summary cards (total files, bytes, embedded dates, mtimes, no date, collisions).
  - Search, filter by date source/status.
  - Paginated/virtualized preview table.
  - Inspect file metadata modal.
  - Collision preview before execution.
- [x] 7.5 Execution screen:
  - Real-time file & byte progress bars.
  - Current file display, live activity log.
  - Pause, resume, cancel controls.
- [x] 7.6 Completion screen:
  - Summary metrics.
  - "Open destination folder" button.
  - Export CSV and JSON reports.
  - Retry failed files button.
- [x] 7.7 History and Settings views:
  - Past jobs with status and detail view.
  - Settings (concurrency, verification mode, fallback policy, licenses/about).

## Phase 8: Testing, Hardening & Packaging
- [x] 8.1 End-to-end integration tests covering large synthetic datasets (10,000+ files).
- [x] 8.2 Edge case testing: Unicode filenames, long paths (>260 chars), zero-byte files, corrupt files, permission errors.
- [x] 8.3 ExifTool bundling and license notices verification.
- [x] 8.4 Verification of production build (`npm run build`, `cargo test`, `cargo build`).
- [x] 8.5 Verification of offline-first operation and privacy guarantees.
