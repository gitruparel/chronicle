import { useState } from 'react';
import { Folder, FolderOpen, ArrowRight, ShieldCheck, AlertTriangle, Loader2 } from 'lucide-react';
import { api } from '../lib/api';
import type { JobOptions, JobSummary } from '../types';

interface NewJobPageProps {
  onScanComplete: (jobId: string, summary: JobSummary, source: string, destination: string, options: JobOptions) => void;
}

export function NewJobPage({ onScanComplete }: NewJobPageProps) {
  const [sourcePath, setSourcePath] = useState(() => {
    return localStorage.getItem('chronicle_last_source_path') || '';
  });
  const [destPath, setDestPath] = useState(() => {
    return localStorage.getItem('chronicle_last_dest_path') || '';
  });
  const [isScanning, setIsScanning] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);

  const [options, setOptions] = useState<JobOptions>({
    recursive: true,
    copy_duplicates: true,
    mtime_fallback: true,
    verify_hash: true,
    worker_count: 4,
  });

  const handlePickSource = async () => {
    try {
      const defaultDir = sourcePath.trim() || localStorage.getItem('chronicle_last_source_path') || undefined;
      const selected = await api.pickFolder('Select Source Directory to Organize', defaultDir);
      if (selected) {
        setSourcePath(selected);
        localStorage.setItem('chronicle_last_source_path', selected);
        setScanError(null);
      }
    } catch (e) {
      console.error('Error picking source folder:', e);
    }
  };

  const handlePickDestination = async () => {
    try {
      const defaultDir = destPath.trim() || localStorage.getItem('chronicle_last_dest_path') || undefined;
      const selected = await api.pickFolder('Select Destination Directory', defaultDir);
      if (selected) {
        setDestPath(selected);
        localStorage.setItem('chronicle_last_dest_path', selected);
        setScanError(null);
      }
    } catch (e) {
      console.error('Error picking destination folder:', e);
    }
  };

  const handleStartScan = async () => {
    if (!sourcePath.trim()) {
      setScanError('Please select a source folder to scan.');
      return;
    }
    if (!destPath.trim()) {
      setScanError('Please select a destination folder for organized files.');
      return;
    }

    setScanError(null);
    setIsScanning(true);

    try {
      // 1. Path safety and overlap check
      const overlap = await api.checkOverlap(sourcePath, destPath);
      if (!overlap.is_safe) {
        setScanError(overlap.error_message || 'Unsafe source and destination overlap detected.');
        setIsScanning(false);
        return;
      }

      // 2. Start scan & metadata analysis
      const jobName = `Job-${new Date().toISOString().slice(0, 10)}`;
      const result = await api.createAndScanJob(jobName, sourcePath, destPath, options);

      onScanComplete(result.job_id, result.summary, sourcePath, destPath, options);
    } catch (err: unknown) {
      const message = typeof err === 'string' ? err : (err as Error).message || 'Failed to scan source directory';
      setScanError(message);
    } finally {
      setIsScanning(false);
    }
  };

  return (
    <div className="max-w-4xl mx-auto space-y-6 py-4">
      {/* Page Title & Philosophy Banner */}
      <div>
        <h2 className="text-lg font-semibold text-text-primary tracking-tight">New Organization Job</h2>
        <p className="text-xs text-text-secondary mt-1">
          Recursively inspect any directory, extract rich metadata across all file extensions, and organize into date folders.
        </p>
      </div>

      {/* Safety Guarantee Callout */}
      <div className="rounded-xl border border-accent/20 bg-accent/5 p-4 flex items-start gap-3">
        <ShieldCheck className="h-5 w-5 text-accent shrink-0 mt-0.5" />
        <div className="text-xs space-y-1">
          <div className="font-semibold text-text-primary">Non-Destructive Copy Principle</div>
          <p className="text-text-secondary leading-relaxed">
            Chronicle strictly performs <strong className="text-text-primary">copies only</strong>. Your original source files will never be modified, renamed, moved, or deleted under any circumstance.
          </p>
        </div>
      </div>

      {/* Directory Selectors */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* Source Directory */}
        <div className="rounded-xl border border-border bg-surface p-4 space-y-3">
          <div className="flex items-center justify-between">
            <label className="text-xs font-semibold text-text-primary flex items-center gap-2">
              <Folder className="h-4 w-4 text-accent" />
              Source Directory
            </label>
            <span className="text-[11px] text-text-muted">Files to organize</span>
          </div>

          <div className="flex gap-2">
            <input
              type="text"
              value={sourcePath}
              onChange={(e) => {
                setSourcePath(e.target.value);
                localStorage.setItem('chronicle_last_source_path', e.target.value);
                setScanError(null);
              }}
              placeholder="Select folder containing files..."
              className="flex-1 bg-background border border-border rounded-lg px-3 py-2 text-xs font-mono text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent"
            />
            <button
              onClick={handlePickSource}
              disabled={isScanning}
              className="px-3 py-2 rounded-lg bg-surface border border-border hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors flex items-center gap-1.5 shrink-0"
            >
              <FolderOpen className="h-3.5 w-3.5" />
              Browse
            </button>
          </div>
        </div>

        {/* Destination Directory */}
        <div className="rounded-xl border border-border bg-surface p-4 space-y-3">
          <div className="flex items-center justify-between">
            <label className="text-xs font-semibold text-text-primary flex items-center gap-2">
              <Folder className="h-4 w-4 text-emerald-400" />
              Destination Directory
            </label>
            <span className="text-[11px] text-text-muted">Target for YYYY-MM-DD</span>
          </div>

          <div className="flex gap-2">
            <input
              type="text"
              value={destPath}
              onChange={(e) => {
                setDestPath(e.target.value);
                localStorage.setItem('chronicle_last_dest_path', e.target.value);
                setScanError(null);
              }}
              placeholder="Select destination root folder..."
              className="flex-1 bg-background border border-border rounded-lg px-3 py-2 text-xs font-mono text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent"
            />
            <button
              onClick={handlePickDestination}
              disabled={isScanning}
              className="px-3 py-2 rounded-lg bg-surface border border-border hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors flex items-center gap-1.5 shrink-0"
            >
              <FolderOpen className="h-3.5 w-3.5" />
              Browse
            </button>
          </div>
        </div>
      </div>

      {/* Configuration Options */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-4">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider">
          Scan & Copy Options
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Recursive */}
          <label className="flex items-start gap-3 p-3 rounded-lg border border-border/60 bg-background/50 hover:bg-background cursor-pointer transition-colors">
            <input
              type="checkbox"
              checked={options.recursive}
              onChange={(e) => setOptions({ ...options, recursive: e.target.checked })}
              className="mt-0.5 rounded border-border bg-surface text-accent focus:ring-accent"
            />
            <div className="text-xs space-y-0.5">
              <span className="font-medium text-text-primary">Recursive Directory Scanning</span>
              <p className="text-[11px] text-text-muted">Scan subdirectories while detecting and skipping symlink loops.</p>
            </div>
          </label>

          {/* Copy Duplicates */}
          <label className="flex items-start gap-3 p-3 rounded-lg border border-border/60 bg-background/50 hover:bg-background cursor-pointer transition-colors">
            <input
              type="checkbox"
              checked={options.copy_duplicates}
              onChange={(e) => setOptions({ ...options, copy_duplicates: e.target.checked })}
              className="mt-0.5 rounded border-border bg-surface text-accent focus:ring-accent"
            />
            <div className="text-xs space-y-0.5">
              <span className="font-medium text-text-primary">Preserve Duplicate Files</span>
              <p className="text-[11px] text-text-muted">Copy all duplicates with collision-safe suffixes. Never drop identical files.</p>
            </div>
          </label>

          {/* Modified Date Fallback */}
          <label className="flex items-start gap-3 p-3 rounded-lg border border-border/60 bg-background/50 hover:bg-background cursor-pointer transition-colors">
            <input
              type="checkbox"
              checked={options.mtime_fallback}
              onChange={(e) => setOptions({ ...options, mtime_fallback: e.target.checked })}
              className="mt-0.5 rounded border-border bg-surface text-accent focus:ring-accent"
            />
            <div className="text-xs space-y-0.5">
              <span className="font-medium text-text-primary">Filesystem Modified Fallback</span>
              <p className="text-[11px] text-text-muted">Use filesystem date if embedded metadata is absent. Otherwise routes to No Date/.</p>
            </div>
          </label>

          {/* SHA-256 Hash Verification */}
          <label className="flex items-start gap-3 p-3 rounded-lg border border-border/60 bg-background/50 hover:bg-background cursor-pointer transition-colors">
            <input
              type="checkbox"
              checked={options.verify_hash}
              onChange={(e) => setOptions({ ...options, verify_hash: e.target.checked })}
              className="mt-0.5 rounded border-border bg-surface text-accent focus:ring-accent"
            />
            <div className="text-xs space-y-0.5">
              <span className="font-medium text-text-primary">SHA-256 Hash Verification</span>
              <p className="text-[11px] text-text-muted">Verify byte integrity on-the-fly and atomically commit copies.</p>
            </div>
          </label>
        </div>
      </div>

      {/* Error Callout */}
      {scanError && (
        <div className="rounded-xl border border-danger/30 bg-danger/10 p-4 flex items-start gap-3 text-danger text-xs">
          <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
          <div className="space-y-1">
            <div className="font-semibold">Unable to Proceed</div>
            <p className="leading-relaxed">{scanError}</p>
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center justify-end gap-3 pt-2">
        <button
          onClick={handleStartScan}
          disabled={isScanning || !sourcePath || !destPath}
          className="flex items-center gap-2 px-6 py-2.5 rounded-lg bg-accent hover:bg-accent-hover text-white text-xs font-semibold shadow-lg shadow-accent/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
        >
          {isScanning ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Scanning & Analyzing Metadata...
            </>
          ) : (
            <>
              Scan & Preview Organization
              <ArrowRight className="h-4 w-4" />
            </>
          )}
        </button>
      </div>
    </div>
  );
}
