import { useState } from 'react';
import {
  CheckCircle2,
  FolderOpen,
  FileSpreadsheet,
  FileCode,
  RotateCcw,
  PlusCircle,
  HardDrive,
  Calendar,
  AlertTriangle,
  Clock,
  Copy,
  Layers,
} from 'lucide-react';
import { api } from '../lib/api';
import type { JobStatus, JobSummary } from '../types';
import { formatBytes } from '../lib/utils';

interface CompletionPageProps {
  jobId: string;
  status: JobStatus;
  summary: JobSummary;
  destinationPath: string;
  onNewJob: () => void;
  onRetry: () => void;
}

export function CompletionPage({
  jobId,
  status,
  summary,
  destinationPath,
  onNewJob,
  onRetry,
}: CompletionPageProps) {
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);

  const handleOpenFolder = async () => {
    try {
      await api.openDestinationFolder(destinationPath);
    } catch (e) {
      console.error('Failed to open destination folder:', e);
    }
  };

  const handleExportCsv = async () => {
    setIsExporting(true);
    setExportMessage(null);
    try {
      const defaultName = `chronicle-report-${jobId.slice(0, 8)}.csv`;
      const savePath = await api.pickSaveFile(defaultName, 'CSV Document', ['csv']);
      if (savePath) {
        await api.exportReport(jobId, 'csv', savePath);
        setExportMessage(`CSV report successfully saved to: ${savePath}`);
      }
    } catch (err: unknown) {
      const msg = typeof err === 'string' ? err : (err as Error).message;
      setExportMessage(`Export error: ${msg}`);
    } finally {
      setIsExporting(false);
    }
  };

  const handleExportJson = async () => {
    setIsExporting(true);
    setExportMessage(null);
    try {
      const defaultName = `chronicle-report-${jobId.slice(0, 8)}.json`;
      const savePath = await api.pickSaveFile(defaultName, 'JSON Report', ['json']);
      if (savePath) {
        await api.exportReport(jobId, 'json', savePath);
        setExportMessage(`JSON report successfully saved to: ${savePath}`);
      }
    } catch (err: unknown) {
      const msg = typeof err === 'string' ? err : (err as Error).message;
      setExportMessage(`Export error: ${msg}`);
    } finally {
      setIsExporting(false);
    }
  };

  const isSuccess = status === 'completed';

  return (
    <div className="max-w-4xl mx-auto space-y-6 py-4">
      {/* Status Hero Card */}
      <div className="rounded-2xl border border-border bg-surface p-6 text-center space-y-3">
        <div
          className={`mx-auto w-12 h-12 rounded-full flex items-center justify-center ${
            isSuccess
              ? 'bg-emerald-500/10 border border-emerald-500/20 text-emerald-400'
              : 'bg-amber-500/10 border border-amber-500/20 text-amber-400'
          }`}
        >
          {isSuccess ? <CheckCircle2 className="h-6 w-6" /> : <AlertTriangle className="h-6 w-6" />}
        </div>
        <div>
          <h2 className="text-xl font-bold text-text-primary tracking-tight">
            {isSuccess ? 'Organization Completed' : `Job Finished with Status: ${status.toUpperCase()}`}
          </h2>
          <p className="text-xs text-text-secondary mt-1">
            Every discovered file is accounted for: verified copies have matching SHA-256 and physical disk commit.
          </p>
        </div>

        {/* Primary Destination Action */}
        <div className="pt-2 flex justify-center">
          <button
            onClick={handleOpenFolder}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-accent hover:bg-accent-hover text-white text-xs font-semibold shadow-lg shadow-accent/20 transition-all"
          >
            <FolderOpen className="h-4 w-4" />
            Open Destination Folder
          </button>
        </div>
      </div>

      {/* Metrics Breakdown Grid */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div className="rounded-xl border border-border bg-surface p-4 space-y-1">
          <span className="text-[11px] text-text-muted flex items-center gap-1.5">
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400" /> Total Verified
          </span>
          <div className="text-lg font-bold text-text-primary font-mono">
            {summary.total_files.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">{formatBytes(summary.total_bytes)}</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-4 space-y-1">
          <span className="text-[11px] text-text-muted flex items-center gap-1.5">
            <Calendar className="h-3.5 w-3.5 text-blue-400" /> Embedded Dates
          </span>
          <div className="text-lg font-bold text-text-primary font-mono">
            {summary.embedded_date_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">EXIF & media metadata</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-4 space-y-1">
          <span className="text-[11px] text-text-muted flex items-center gap-1.5">
            <Clock className="h-3.5 w-3.5 text-amber-400" /> Modified Fallback
          </span>
          <div className="text-lg font-bold text-text-primary font-mono">
            {summary.mtime_date_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">Local filesystem mtime</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-4 space-y-1">
          <span className="text-[11px] text-text-muted flex items-center gap-1.5">
            <Layers className="h-3.5 w-3.5 text-indigo-400" /> Date Folders
          </span>
          <div className="text-lg font-bold text-text-primary font-mono">
            {summary.unique_folders_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">Unique YYYY-MM-DD</span>
        </div>
      </div>

      {/* Secondary Metrics */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
        <div className="rounded-xl border border-border bg-surface p-3 space-y-1 text-xs">
          <span className="text-text-muted flex items-center gap-1.5">
            <Copy className="h-3.5 w-3.5 text-purple-400" /> Duplicate Content
          </span>
          <div className="font-semibold text-text-primary">
            {summary.duplicate_content_count.toLocaleString()} files
          </div>
          <p className="text-[10px] text-text-muted">Preserved with collision suffixes</p>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1 text-xs">
          <span className="text-text-muted flex items-center gap-1.5">
            <HardDrive className="h-3.5 w-3.5 text-slate-400" /> No Date Route
          </span>
          <div className="font-semibold text-text-primary">
            {summary.no_date_count.toLocaleString()} files
          </div>
          <p className="text-[10px] text-text-muted">Organized in No Date/ folder</p>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1 text-xs">
          <span className="text-text-muted flex items-center gap-1.5">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-400" /> Failures / Errors
          </span>
          <div className="font-semibold text-text-primary">
            {(summary.scan_error_count + summary.metadata_error_count).toLocaleString()} issues
          </div>
          <p className="text-[10px] text-text-muted">All tracked in database and reports</p>
        </div>
      </div>

      {/* Export & Actions Card */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-4">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider">
          Reports & Next Actions
        </h3>

        <div className="flex flex-wrap items-center gap-3">
          <button
            onClick={handleExportCsv}
            disabled={isExporting}
            className="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-background hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
          >
            <FileSpreadsheet className="h-4 w-4 text-emerald-400" />
            Export CSV Report
          </button>

          <button
            onClick={handleExportJson}
            disabled={isExporting}
            className="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-background hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
          >
            <FileCode className="h-4 w-4 text-blue-400" />
            Export JSON Report
          </button>

          {(!isSuccess || summary.scan_error_count > 0) && (
            <button
              onClick={onRetry}
              className="flex items-center gap-2 px-4 py-2 rounded-lg border border-amber-500/30 bg-amber-500/10 hover:bg-amber-500/20 text-xs font-medium text-amber-400 transition-colors"
            >
              <RotateCcw className="h-4 w-4" />
              Retry Failed Files
            </button>
          )}

          <div className="flex-1" />

          <button
            onClick={onNewJob}
            className="flex items-center gap-2 px-5 py-2 rounded-lg bg-surface border border-border hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
          >
            <PlusCircle className="h-4 w-4 text-accent" />
            Start New Job
          </button>
        </div>

        {exportMessage && (
          <div className="p-3 rounded-lg border border-border bg-background text-xs font-mono text-text-secondary select-text">
            {exportMessage}
          </div>
        )}
      </div>
    </div>
  );
}
