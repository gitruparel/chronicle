import { useState, useEffect, useRef } from 'react';
import {
  Loader2,
  CheckCircle2,
  AlertTriangle,
  Pause,
  Play,
  XCircle,
  FileText,
  HardDrive,
  Activity,
  Clock,
} from 'lucide-react';
import { api } from '../lib/api';
import type { JobOptions, JobProgress, JobStatus } from '../types';
import { formatBytes } from '../lib/utils';

interface ExecutionPageProps {
  jobId: string;
  totalFiles: number;
  totalBytes: number;
  options: JobOptions;
  isRetry?: boolean;
  onFinished: (status: JobStatus) => void;
}

export function ExecutionPage({
  jobId,
  totalFiles,
  totalBytes,
  options,
  isRetry = false,
  onFinished,
}: ExecutionPageProps) {
  const [progress, setProgress] = useState<JobProgress>({
    job_id: jobId,
    status: 'running',
    total_files: totalFiles,
    total_bytes: totalBytes,
    copied_files: 0,
    copied_bytes: 0,
    failed_files: 0,
    excluded_files: 0,
    current_file: null,
    current_operation: 'Initializing copy engine...',
    speed_bytes_per_sec: 0,
    eta_seconds: null,
  });

  const [isPaused, setIsPaused] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const hasStartedRef = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await api.onJobProgress((p) => {
        if (p.job_id === jobId) {
          setProgress(p);

          if (p.current_file) {
            setLogs((prev) => [
              `[${new Date().toLocaleTimeString()}] ${p.current_operation}`,
              ...prev.slice(0, 49),
            ]);
          }

          if (['completed', 'failed', 'cancelled'].includes(p.status)) {
            onFinished(p.status);
          }
        }
      });

      // Start the copy engine exactly once per mount
      if (!hasStartedRef.current) {
        hasStartedRef.current = true;
        try {
          if (isRetry) {
            await api.retryFailed(jobId, options);
          } else {
            await api.startCopy(jobId, options);
          }
        } catch (err) {
          console.error('Failed to trigger copy job:', err);
        }
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, [jobId, options, isRetry, onFinished]);

  const handlePause = async () => {
    try {
      await api.pauseCopy(jobId);
      setIsPaused(true);
    } catch (e) {
      console.error('Pause failed:', e);
    }
  };

  const handleResume = async () => {
    try {
      await api.resumeCopy(jobId);
      setIsPaused(false);
    } catch (e) {
      console.error('Resume failed:', e);
    }
  };

  const handleCancel = async () => {
    try {
      await api.cancelCopy(jobId);
    } catch (e) {
      console.error('Cancel failed:', e);
    }
  };

  const filePercentage =
    progress.total_files > 0 ? Math.min(100, Math.round((progress.copied_files / progress.total_files) * 100)) : 0;

  const bytePercentage =
    progress.total_bytes > 0 ? Math.min(100, Math.round((progress.copied_bytes / progress.total_bytes) * 100)) : 0;

  return (
    <div className="max-w-4xl mx-auto space-y-6 py-4">
      {/* Header & Status */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-accent/10 border border-accent/20 flex items-center justify-center text-accent">
            {isPaused ? <Pause className="h-5 w-5" /> : <Loader2 className="h-5 w-5 animate-spin" />}
          </div>
          <div>
            <h2 className="text-base font-semibold text-text-primary tracking-tight">
              {isPaused ? 'Copy Paused' : isRetry ? 'Retrying Failed Files...' : 'Organizing and Copying Files...'}
            </h2>
            <p className="text-xs text-text-secondary">
              Bounded concurrent streams with SHA-256 verification and physical disk commit.
            </p>
          </div>
        </div>

        {/* Controls */}
        <div className="flex items-center gap-2">
          {isPaused ? (
            <button
              onClick={handleResume}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
            >
              <Play className="h-3.5 w-3.5 text-emerald-400" />
              Resume
            </button>
          ) : (
            <button
              onClick={handlePause}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-secondary hover:text-text-primary transition-colors"
            >
              <Pause className="h-3.5 w-3.5" />
              Pause
            </button>
          )}

          <button
            onClick={handleCancel}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-danger/30 bg-danger/10 hover:bg-danger/20 text-xs font-medium text-danger transition-colors"
          >
            <XCircle className="h-3.5 w-3.5" />
            Cancel
          </button>
        </div>
      </div>

      {/* Progress Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* File Count Progress */}
        <div className="rounded-xl border border-border bg-surface p-4 space-y-3">
          <div className="flex items-center justify-between text-xs">
            <span className="font-semibold text-text-primary flex items-center gap-1.5">
              <FileText className="h-3.5 w-3.5 text-accent" />
              Files Processed
            </span>
            <span className="font-mono text-text-muted">
              {progress.copied_files.toLocaleString()} / {progress.total_files.toLocaleString()} ({filePercentage}%)
            </span>
          </div>

          <div className="h-2 w-full bg-background rounded-full overflow-hidden border border-border">
            <div
              className="h-full bg-accent rounded-full transition-all duration-300"
              style={{ width: `${filePercentage}%` }}
            />
          </div>
        </div>

        {/* Bytes Progress */}
        <div className="rounded-xl border border-border bg-surface p-4 space-y-3">
          <div className="flex items-center justify-between text-xs">
            <span className="font-semibold text-text-primary flex items-center gap-1.5">
              <HardDrive className="h-3.5 w-3.5 text-emerald-400" />
              Data Transferred
            </span>
            <span className="font-mono text-text-muted">
              {formatBytes(progress.copied_bytes)} / {formatBytes(progress.total_bytes)} ({bytePercentage}%)
            </span>
          </div>

          <div className="h-2 w-full bg-background rounded-full overflow-hidden border border-border">
            <div
              className="h-full bg-emerald-500 rounded-full transition-all duration-300"
              style={{ width: `${bytePercentage}%` }}
            />
          </div>
        </div>
      </div>

      {/* Live Operation Status */}
      <div className="rounded-xl border border-border bg-surface p-4 space-y-2">
        <div className="text-[11px] font-medium text-text-muted uppercase tracking-wider flex items-center gap-1.5">
          <Activity className="h-3 w-3 text-accent" />
          Active Operation
        </div>
        <div className="font-mono text-xs text-text-primary truncate">
          {progress.current_file ? progress.current_file : 'Preparing files...'}
        </div>
        <div className="text-[11px] text-text-secondary truncate">
          {progress.current_operation}
        </div>
      </div>

      {/* Counters Bar */}
      <div className="grid grid-cols-3 gap-3 text-center">
        <div className="rounded-xl border border-border bg-surface p-3 space-y-0.5">
          <span className="text-[11px] text-emerald-400 flex items-center justify-center gap-1">
            <CheckCircle2 className="h-3 w-3" /> Successfully Copied
          </span>
          <div className="text-base font-semibold text-text-primary font-mono">
            {progress.copied_files.toLocaleString()}
          </div>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-0.5">
          <span className="text-[11px] text-danger flex items-center justify-center gap-1">
            <AlertTriangle className="h-3 w-3" /> Failed
          </span>
          <div className="text-base font-semibold text-text-primary font-mono">
            {progress.failed_files.toLocaleString()}
          </div>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-0.5">
          <span className="text-[11px] text-text-muted flex items-center justify-center gap-1">
            <Clock className="h-3 w-3" /> Remaining
          </span>
          <div className="text-base font-semibold text-text-primary font-mono">
            {Math.max(0, progress.total_files - progress.copied_files - progress.failed_files).toLocaleString()}
          </div>
        </div>
      </div>

      {/* Live Log Stream */}
      <div className="rounded-xl border border-border bg-surface overflow-hidden space-y-0">
        <div className="border-b border-border px-4 py-2.5 bg-surface-hover/30 text-[11px] font-semibold text-text-muted uppercase tracking-wider">
          Execution Log
        </div>
        <div className="p-3 font-mono text-[11px] text-text-secondary h-44 overflow-y-auto space-y-1 bg-background select-text">
          {logs.length === 0 ? (
            <div className="text-text-muted italic">Waiting for copy events...</div>
          ) : (
            logs.map((log, idx) => (
              <div key={idx} className="truncate">
                {log}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
