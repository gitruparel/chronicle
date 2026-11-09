import { useState, useEffect } from 'react';
import { Clock, Folder, CheckCircle2, AlertTriangle, RefreshCw } from 'lucide-react';
import { api } from '../lib/api';
import type { JobRecord } from '../types';
import { formatBytes } from '../lib/utils';

export function HistoryPage() {
  const [jobs, setJobs] = useState<JobRecord[]>([]);
  const [loading, setLoading] = useState(true);

  const loadHistory = async () => {
    setLoading(true);
    try {
      const records = await api.getAllJobs();
      setJobs(records);
    } catch (e) {
      console.error('Failed to load job history from SQLite:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadHistory();
  }, []);

  if (loading) {
    return (
      <div className="max-w-4xl mx-auto py-16 text-center space-y-3">
        <RefreshCw className="h-6 w-6 text-accent animate-spin mx-auto" />
        <p className="text-xs text-text-muted">Loading job records from SQLite database...</p>
      </div>
    );
  }

  if (jobs.length === 0) {
    return (
      <div className="max-w-4xl mx-auto py-16 text-center space-y-3">
        <div className="mx-auto w-12 h-12 rounded-xl bg-surface border border-border flex items-center justify-center text-text-muted">
          <Clock className="h-6 w-6" />
        </div>
        <h3 className="text-sm font-semibold text-text-primary">No Job History Yet</h3>
        <p className="text-xs text-text-secondary max-w-sm mx-auto">
          Completed and interrupted organization jobs are stored in SQLite and will appear here with verification metrics and reports.
        </p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto space-y-4 py-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-text-primary tracking-tight">Persistent Job History</h2>
          <p className="text-xs text-text-secondary mt-0.5">
            Historical organization jobs stored in local SQLite database.
          </p>
        </div>
        <button
          onClick={loadHistory}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
        >
          <RefreshCw className="h-3.5 w-3.5" />
          Refresh
        </button>
      </div>

      <div className="space-y-3">
        {jobs.map((job) => {
          const isCompleted = job.status === 'completed';
          const isFailed = job.status === 'failed';
          const isPaused = job.status === 'paused';

          return (
            <div
              key={job.id}
              className="rounded-xl border border-border bg-surface p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 shadow-sm"
            >
              <div className="space-y-1.5 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-xs font-semibold text-text-primary">
                    {job.name || job.id.slice(0, 8)}
                  </span>
                  <span
                    className={`text-[10px] px-2 py-0.5 rounded font-medium border uppercase tracking-wider ${
                      isCompleted
                        ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                        : isFailed
                        ? 'bg-danger/10 text-danger border-danger/20'
                        : isPaused
                        ? 'bg-amber-500/10 text-amber-400 border-amber-500/20'
                        : 'bg-blue-500/10 text-blue-400 border-blue-500/20'
                    }`}
                  >
                    {job.status}
                  </span>
                  <span className="text-[11px] text-text-muted">
                    {new Date(job.created_at).toLocaleDateString()} {new Date(job.created_at).toLocaleTimeString()}
                  </span>
                </div>

                <div className="text-xs text-text-secondary flex items-center gap-1.5 truncate">
                  <Folder className="h-3.5 w-3.5 shrink-0 text-accent" />
                  <span className="truncate">{job.source_path}</span>
                  <span className="text-text-muted">&rarr;</span>
                  <span className="truncate text-text-primary">{job.destination_path}</span>
                </div>
              </div>

              <div className="flex items-center gap-5 text-xs font-mono shrink-0">
                <div className="text-right">
                  <div className="font-semibold text-text-primary flex items-center gap-1 justify-end">
                    {isCompleted ? (
                      <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400" />
                    ) : (
                      <AlertTriangle className="h-3.5 w-3.5 text-amber-400" />
                    )}
                    {job.copied_files.toLocaleString()} / {job.total_files.toLocaleString()} files
                  </div>
                  <div className="text-[10px] text-text-muted">
                    {formatBytes(job.copied_bytes)} / {formatBytes(job.total_bytes)}
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
