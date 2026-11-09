# Chronicle

**Chronicle** is a production-quality, offline-first Windows desktop application engineered to organize files deterministically by their most appropriate date.

Unlike photo-only organizers, Chronicle operates under a universal project principle: **it treats all file extensions equally**, recursively scans source directories, inspects rich metadata (EXIF, QuickTime, XMP, document, and audio tags) where available, resolves dates deterministically, and copies every eligible file into structured date folders (`YYYY-MM-DD` or `No Date`).

---

## Key Principles & Guarantees

1. **Non-Destructive Copy-Only Operation**:
   - Source files are strictly read-only. Chronicle **never** moves, modifies, renames, or deletes files in the source directory.
2. **Universal File Handling**:
   - No media-type separation and no extension filtering. Whether `.jpg`, `.heic`, `.pdf`, `.docx`, `.mp4`, `.zip`, `.raw`, or unknown binary formats without extensions, all files participate in the same organization and copy pipeline.
3. **No Silent Drops**:
   - If metadata extraction fails or is unsupported for a file format, the file is not discarded. It gracefully falls back to filesystem modified time (if enabled) or `No Date/`.
4. **Deterministic Collision Resolution**:
   - Files with identical names never overwrite existing destination files. Collisions are resolved deterministically with numeric suffixes (e.g., `IMG_0001 (1).jpg`).
   - Duplicate-content files (identical SHA-256) are always preserved and copied.
5. **Streaming, Verified Copies**:
   - Large files are streamed in chunks to prevent memory bloat.
   - Files are copied to a temporary file (`.chronicle_tmp_*`), verified via byte size and SHA-256 hash, and atomically renamed.
6. **Crash-Resilient Job Engine**:
   - Powered by an embedded SQLite database with atomic transactions.
   - Operations are idempotent and recoverable. Incomplete jobs can be resumed or retried safely.
7. **Offline-First & Privacy Preserving**:
   - Chronicle makes zero network connections. No telemetry or file data ever leaves the local computer.

---

## Destination Folder Structure

```
Destination/
  2024-05-18/
    IMG_1234.HEIC
    report.pdf
    archive.zip
  2024-05-19/
    recording.mov
    presentation.pptx
  No Date/
    unrecognized.bin
```

---

## Architecture Overview

Chronicle is built on a high-performance **Tauri 2** architecture:
- **Core Engine (Rust)**: High-speed recursive directory traversal, symlink loop detection, chunked streaming copies, SHA-256 verification, ExifTool metadata orchestration, and SQLite job persistence.
- **Frontend (React + TypeScript + Tailwind CSS + Lucide Icons)**: A clean, accessible desktop interface inspired by Linear and Vercel design systems. Includes paginated preview, collision auditing, live execution metrics, error logs, and comprehensive export capabilities.
- **Metadata Engine**: Deterministic date resolver utilizing bundled ExifTool 13.x with fallback to filesystem timestamps.

---

## Building and Running

### Prerequisites
- Windows 10/11 (64-bit)
- Node.js 18+ and npm
- Rust toolchain (1.80+ with MSVC)
- WebView2 Runtime (pre-installed on Windows 10/11)

### Development Setup

```powershell
# Install frontend dependencies
npm install

# Run application in development mode
npm run tauri dev
```

### Running Tests

```powershell
# Rust unit and integration tests
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend type checking and linting
npm run typecheck
```

---

## Third-Party Licenses & Bundled Tools

- **ExifTool**: Copyright 2003-2026 Phil Harvey. Distributed under the Perl Artistic License and GNU General Public License (GPL). ExifTool is bundled as a separate external resource executable and invoked via standard process boundaries. Full license notices are provided in `src-tauri/resources/EXIFTOOL_LICENSE.txt` and accessible within the application Settings.
- **SQLite**: Dedicated to the Public Domain.
- **Tauri**: Dual-licensed under MIT or Apache 2.0.
