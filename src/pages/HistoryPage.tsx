import { Clock, Folder } from 'lucide-react';
import type { JobSummary } from '../types';

interface HistoryPageProps {
  pastJobs: Array<{
    id: string;
    date: string;
    source: string;
    destination: string;
    summary: JobSummary;
    status: string;
  }>;
  onSelectJob: (jobId: string) => void;
}

export function HistoryPage({ pastJobs }: HistoryPageProps) {
  if (pastJobs.length === 0) {
    return (
      <div className="max-w-4xl mx-auto py-16 text-center space-y-3">
        <div className="mx-auto w-12 h-12 rounded-xl bg-surface border border-border flex items-center justify-center text-text-muted">
          <Clock className="h-6 w-6" />
        </div>
        <h3 className="text-sm font-semibold text-text-primary">No Job History Yet</h3>
        <p className="text-xs text-text-secondary max-w-sm mx-auto">
          Completed and interrupted organization jobs will appear here with verification metrics and reports.
        </p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto space-y-4 py-4">
      <div>
        <h2 className="text-lg font-semibold text-text-primary tracking-tight">Job History</h2>
        <p className="text-xs text-text-secondary mt-0.5">
          Persistent history of past organization operations.
        </p>
      </div>

      <div className="space-y-3">
        {pastJobs.map((job) => (
          <div
            key={job.id}
            className="rounded-xl border border-border bg-surface p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4"
          >
            <div className="space-y-1.5 min-w-0">
              <div className="flex items-center gap-2">
                <span className="font-mono text-xs font-semibold text-text-primary">
                  {job.id.slice(0, 8)}
                </span>
                <span className="text-[10px] px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-medium">
                  {job.status}
                </span>
                <span className="text-[11px] text-text-muted">{job.date}</span>
              </div>

              <div className="text-xs text-text-secondary flex items-center gap-1.5 truncate">
                <Folder className="h-3.5 w-3.5 shrink-0 text-accent" />
                <span className="truncate">{job.source}</span>
                <span className="text-text-muted">&rarr;</span>
                <span className="truncate text-text-primary">{job.destination}</span>
              </div>
            </div>

            <div className="flex items-center gap-4 text-xs font-mono shrink-0">
              <div className="text-right">
                <div className="font-semibold text-text-primary">
                  {job.summary.total_files.toLocaleString()} files
                </div>
                <div className="text-[10px] text-text-muted">
                  {job.summary.unique_folders_count} date folders
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
