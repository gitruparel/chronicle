import { useState, useEffect, useCallback } from 'react';
import {
  Clock,
  Folder,
  CheckCircle2,
  AlertTriangle,
  RefreshCw,
  ArrowLeft,
  Search,
  Filter,
  FolderOpen,
  FileSpreadsheet,
  FileCode,
  Calendar,
  Copy,
  AlertCircle,
  FileText,
  Tag,
  ShieldCheck,
  ChevronLeft,
  ChevronRight,
  Info,
  ChevronRight as ChevronRightIcon,
  Play,
} from 'lucide-react';
import { api } from '../lib/api';
import type { JobRecord, JobSummary, PlannedFile } from '../types';
import { formatBytes, formatDateSourceLabel } from '../lib/utils';
import { FileDetailModal } from '../components/FileDetailModal';

interface HistoryPageProps {
  onOpenJobInPreview?: (jobId: string, summary: JobSummary, destination: string) => void;
}

export function HistoryPage({ onOpenJobInPreview }: HistoryPageProps) {
  const [jobs, setJobs] = useState<JobRecord[]>([]);
  const [loading, setLoading] = useState(true);

  // Selected Job State for Detailed View
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobRecord | null>(null);
  const [jobSummary, setJobSummary] = useState<JobSummary | null>(null);
  const [files, setFiles] = useState<PlannedFile[]>([]);
  const [totalFiles, setTotalFiles] = useState(0);
  const [page, setPage] = useState(0);
  const [pageSize] = useState(50);
  const [isLoadingFiles, setIsLoadingFiles] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [sourceFilter, setSourceFilter] = useState('all');
  const [selectedFileForModal, setSelectedFileForModal] = useState<PlannedFile | null>(null);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);

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

  // Fetch job details when a job is selected
  const openJobDetail = async (job: JobRecord) => {
    setSelectedJobId(job.id);
    setSelectedJob(job);
    setPage(0);
    setSearchTerm('');
    setSourceFilter('all');
    setExportMessage(null);

    try {
      const summary = await api.getJobSummary(job.id);
      setJobSummary(summary);
    } catch (e) {
      console.error('Failed to fetch summary for job:', e);
    }
  };

  const fetchFiles = useCallback(async (jobId: string, p: number, filter: string, search: string) => {
    setIsLoadingFiles(true);
    try {
      const res = await api.getPreviewFiles(jobId, p, pageSize, filter, search);
      setFiles(res.files);
      setTotalFiles(res.total);
    } catch (e) {
      console.error('Failed to fetch files for job detail:', e);
    } finally {
      setIsLoadingFiles(false);
    }
  }, [pageSize]);

  useEffect(() => {
    if (selectedJobId) {
      fetchFiles(selectedJobId, page, sourceFilter, searchTerm);
    }
  }, [selectedJobId, page, sourceFilter, searchTerm, fetchFiles]);

  const handleSearchChange = (term: string) => {
    setSearchTerm(term);
    setPage(0);
  };

  const handleFilterChange = (filterId: string) => {
    setSourceFilter(filterId);
    setPage(0);
  };

  const handleOpenFolder = async (path: string) => {
    try {
      await api.openDestinationFolder(path);
    } catch (e) {
      console.error('Failed to open destination folder:', e);
    }
  };

  const handleExportCsv = async (jobId: string) => {
    setIsExporting(true);
    setExportMessage(null);
    try {
      const defaultName = `chronicle-report-${jobId.slice(0, 8)}.csv`;
      const savePath = await api.pickSaveFile(defaultName, 'CSV Document', ['csv']);
      if (savePath) {
        await api.exportReport(jobId, 'csv', savePath);
        setExportMessage(`CSV report saved to: ${savePath}`);
      }
    } catch (err: unknown) {
      const msg = typeof err === 'string' ? err : (err as Error).message;
      setExportMessage(`Export error: ${msg}`);
    } finally {
      setIsExporting(false);
    }
  };

  const handleExportJson = async (jobId: string) => {
    setIsExporting(true);
    setExportMessage(null);
    try {
      const defaultName = `chronicle-report-${jobId.slice(0, 8)}.json`;
      const savePath = await api.pickSaveFile(defaultName, 'JSON Report', ['json']);
      if (savePath) {
        await api.exportReport(jobId, 'json', savePath);
        setExportMessage(`JSON report saved to: ${savePath}`);
      }
    } catch (err: unknown) {
      const msg = typeof err === 'string' ? err : (err as Error).message;
      setExportMessage(`Export error: ${msg}`);
    } finally {
      setIsExporting(false);
    }
  };

  if (loading) {
    return (
      <div className="max-w-4xl mx-auto py-16 text-center space-y-3">
        <RefreshCw className="h-6 w-6 text-accent animate-spin mx-auto" />
        <p className="text-xs text-text-muted">Loading job records from SQLite database...</p>
      </div>
    );
  }

  // ==========================================
  // VIEW 1: DETAILED VIEW FOR A SELECTED JOB
  // ==========================================
  if (selectedJobId && selectedJob) {
    const isCompleted = selectedJob.status === 'completed';
    const isFailed = selectedJob.status === 'failed';
    const isPaused = selectedJob.status === 'paused';
    const isAnalysed = selectedJob.status === 'analysed';
    const totalPages = Math.ceil(totalFiles / pageSize);

    return (
      <div className="max-w-6xl mx-auto space-y-5 py-2">
        <FileDetailModal
          file={selectedFileForModal}
          onClose={() => setSelectedFileForModal(null)}
        />

        {/* Top Header Bar */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                setSelectedJobId(null);
                setSelectedJob(null);
                setJobSummary(null);
              }}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-secondary hover:text-text-primary transition-colors"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              Back to History
            </button>
            <div>
              <h2 className="text-lg font-semibold text-text-primary tracking-tight flex items-center gap-2">
                <span>{selectedJob.name || `Job ${selectedJob.id.slice(0, 8)}`}</span>
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
                  {selectedJob.status}
                </span>
              </h2>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="flex items-center gap-2 flex-wrap">
            <button
              onClick={() => handleOpenFolder(selectedJob.destination_path)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
              title="Open organized destination folder in Windows Explorer"
            >
              <FolderOpen className="h-3.5 w-3.5 text-accent" />
              Open Folder
            </button>

            <button
              onClick={() => handleExportCsv(selectedJob.id)}
              disabled={isExporting}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors disabled:opacity-50"
            >
              <FileSpreadsheet className="h-3.5 w-3.5 text-emerald-400" />
              Export CSV
            </button>

            <button
              onClick={() => handleExportJson(selectedJob.id)}
              disabled={isExporting}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors disabled:opacity-50"
            >
              <FileCode className="h-3.5 w-3.5 text-purple-400" />
              Export JSON
            </button>

            {isAnalysed && onOpenJobInPreview && jobSummary && (
              <button
                onClick={() => onOpenJobInPreview(selectedJob.id, jobSummary, selectedJob.destination_path)}
                className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-accent hover:bg-accent-hover text-white text-xs font-semibold shadow-sm transition-all"
              >
                <Play className="h-3.5 w-3.5" />
                Preview & Copy
              </button>
            )}
          </div>
        </div>

        {/* Path & Time Banner */}
        <div className="rounded-xl border border-border bg-surface p-3.5 space-y-2 text-xs">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 text-text-secondary border-b border-border/50 pb-2">
            <div className="flex items-center gap-2">
              <span className="text-text-muted">Job ID:</span>
              <span className="font-mono text-text-primary select-all">{selectedJob.id}</span>
            </div>
            <div className="flex items-center gap-4 text-text-muted">
              <span>Created: {new Date(selectedJob.created_at).toLocaleString()}</span>
              {selectedJob.completed_at && (
                <span>Completed: {new Date(selectedJob.completed_at).toLocaleString()}</span>
              )}
            </div>
          </div>

          <div className="flex flex-col md:flex-row md:items-center gap-2 md:gap-4 font-mono text-[11px] pt-0.5">
            <div className="flex items-center gap-1.5 min-w-0 flex-1 truncate">
              <Folder className="h-3.5 w-3.5 shrink-0 text-text-muted" />
              <span className="text-text-muted shrink-0">Source:</span>
              <span className="truncate text-text-primary" title={selectedJob.source_path}>
                {selectedJob.source_path}
              </span>
            </div>
            <span className="text-text-muted hidden md:inline">&rarr;</span>
            <div className="flex items-center gap-1.5 min-w-0 flex-1 truncate">
              <Folder className="h-3.5 w-3.5 shrink-0 text-accent" />
              <span className="text-text-muted shrink-0">Destination:</span>
              <span className="truncate text-accent" title={selectedJob.destination_path}>
                {selectedJob.destination_path}
              </span>
            </div>
          </div>
        </div>

        {/* Export Notification */}
        {exportMessage && (
          <div className="p-3 rounded-xl bg-accent/10 border border-accent/20 text-accent text-xs flex items-center justify-between">
            <span>{exportMessage}</span>
            <button onClick={() => setExportMessage(null)} className="text-text-muted hover:text-text-primary text-xs">
              Dismiss
            </button>
          </div>
        )}

        {/* KPI Cards: Reliable Summary */}
        {jobSummary ? (
          <div className="grid grid-cols-2 md:grid-cols-6 gap-3">
            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-text-muted flex items-center gap-1">
                <FileText className="h-3 w-3" /> Total Files
              </span>
              <div className="text-base font-semibold text-text-primary">
                {jobSummary.total_files.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted font-mono">{formatBytes(jobSummary.total_bytes)}</span>
            </div>

            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-emerald-400 flex items-center gap-1">
                <ShieldCheck className="h-3 w-3" /> Verified Copied
              </span>
              <div className="text-base font-semibold text-text-primary">
                {selectedJob.copied_files.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted font-mono">{formatBytes(selectedJob.copied_bytes)} (SHA-256)</span>
            </div>

            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-blue-400 flex items-center gap-1">
                <Calendar className="h-3 w-3" /> Embedded Dates
              </span>
              <div className="text-base font-semibold text-text-primary">
                {jobSummary.embedded_date_count.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted">EXIF / Media / Docs</span>
            </div>

            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-amber-400 flex items-center gap-1">
                <Clock className="h-3 w-3" /> Modified Date
              </span>
              <div className="text-base font-semibold text-text-primary">
                {jobSummary.mtime_date_count.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted">Filesystem mtime</span>
            </div>

            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-slate-400 flex items-center gap-1">
                <AlertCircle className="h-3 w-3" /> No Date
              </span>
              <div className="text-base font-semibold text-text-primary">
                {jobSummary.no_date_count.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted">Routes to No Date/</span>
            </div>

            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-purple-400 flex items-center gap-1">
                <Copy className="h-3 w-3" /> Collisions
              </span>
              <div className="text-base font-semibold text-text-primary">
                {jobSummary.duplicate_content_count.toLocaleString()}
              </div>
              <span className="text-[10px] text-text-muted">Zero-overwrite safely</span>
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-text-muted">Total Discovered</span>
              <div className="text-base font-semibold text-text-primary">{selectedJob.total_files.toLocaleString()}</div>
              <span className="text-[10px] text-text-muted font-mono">{formatBytes(selectedJob.total_bytes)}</span>
            </div>
            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-emerald-400">Copied & Verified</span>
              <div className="text-base font-semibold text-text-primary">{selectedJob.copied_files.toLocaleString()}</div>
              <span className="text-[10px] text-text-muted font-mono">{formatBytes(selectedJob.copied_bytes)}</span>
            </div>
            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-danger">Failed Files</span>
              <div className="text-base font-semibold text-text-primary">{selectedJob.failed_files.toLocaleString()}</div>
              <span className="text-[10px] text-text-muted">Requires retry</span>
            </div>
            <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
              <span className="text-[11px] text-text-muted">Excluded (Policy)</span>
              <div className="text-base font-semibold text-text-primary">{selectedJob.excluded_files.toLocaleString()}</div>
              <span className="text-[10px] text-text-muted">Symlinks/Junctions</span>
            </div>
          </div>
        )}

        {/* Search & Filter Toolbar */}
        <div className="flex flex-col sm:flex-row items-center justify-between gap-3 bg-surface p-2.5 rounded-xl border border-border">
          <div className="relative w-full sm:w-80">
            <Search className="absolute left-3 top-2.5 h-3.5 w-3.5 text-text-muted" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => handleSearchChange(e.target.value)}
              placeholder="Search transferred filename or path..."
              className="w-full bg-background border border-border rounded-lg pl-8 pr-3 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent"
            />
          </div>

          <div className="flex items-center gap-1.5 w-full sm:w-auto overflow-x-auto">
            <span className="text-[11px] text-text-muted flex items-center gap-1 shrink-0">
              <Filter className="h-3 w-3" /> Filter:
            </span>
            {[
              { id: 'all', label: 'All Files' },
              { id: 'verified', label: 'Copied & Verified' },
              { id: 'errors', label: 'Errors / Failed' },
              { id: 'embedded', label: 'Embedded Dates' },
              { id: 'modified', label: 'Modified Date' },
              { id: 'no_date', label: 'No Date' },
              { id: 'collisions', label: 'Collisions' },
            ].map((item) => (
              <button
                key={item.id}
                onClick={() => handleFilterChange(item.id)}
                className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors shrink-0 ${
                  sourceFilter === item.id
                    ? 'bg-accent text-white'
                    : 'bg-background hover:bg-surface-hover text-text-secondary border border-border'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>

        {/* Transferred Files Table */}
        <div className="rounded-xl border border-border bg-surface overflow-hidden shadow-sm">
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs border-collapse">
              <thead>
                <tr className="border-b border-border bg-surface-hover/50 text-[11px] font-semibold text-text-muted uppercase tracking-wider">
                  <th className="py-2.5 px-3">Filename / Source</th>
                  <th className="py-2.5 px-3">Size</th>
                  <th className="py-2.5 px-3">Resolved Date</th>
                  <th className="py-2.5 px-3">Source Tag</th>
                  <th className="py-2.5 px-3">Destination Path</th>
                  <th className="py-2.5 px-3">Status / Integrity</th>
                  <th className="py-2.5 px-3 text-right">Details</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {isLoadingFiles ? (
                  <tr>
                    <td colSpan={7} className="py-12 text-center text-text-muted text-xs">
                      Loading job files...
                    </td>
                  </tr>
                ) : files.length === 0 ? (
                  <tr>
                    <td colSpan={7} className="py-12 text-center text-text-muted text-xs">
                      No files matching current criteria.
                    </td>
                  </tr>
                ) : (
                  files.map((file) => {
                    const tagMeta = formatDateSourceLabel(file.date_resolution.date_source);
                    const isVerified = file.status === 'verified';
                    const isFileFailed = file.status === 'failed';

                    return (
                      <tr key={file.id} className="hover:bg-surface-hover/40 transition-colors group">
                        <td className="py-2.5 px-3 max-w-[260px]">
                          <div className="font-medium text-text-primary truncate" title={file.filename}>
                            {file.filename}
                          </div>
                          <div className="text-[10px] text-text-muted font-mono truncate" title={file.relative_path}>
                            {file.relative_path}
                          </div>
                        </td>

                        <td className="py-2.5 px-3 font-mono text-text-secondary whitespace-nowrap">
                          {formatBytes(file.size_bytes)}
                        </td>

                        <td className="py-2.5 px-3 whitespace-nowrap">
                          <span className="font-mono font-medium text-text-primary">
                            {file.date_resolution.resolved_date || 'No Date'}
                          </span>
                        </td>

                        <td className="py-2.5 px-3 whitespace-nowrap">
                          <span
                            className={`inline-block px-2 py-0.5 rounded text-[10px] font-medium border ${tagMeta.color}`}
                          >
                            {tagMeta.label}
                          </span>
                        </td>

                        <td className="py-2.5 px-3 max-w-[260px]">
                          <div
                            className="font-mono text-accent text-[11px] truncate"
                            title={`${file.destination_folder}/${file.planned_filename}`}
                          >
                            {file.destination_folder} / {file.planned_filename}
                          </div>
                          {file.is_collision && (
                            <span className="inline-flex items-center gap-1 text-[10px] text-amber-400 font-medium">
                              <Tag className="h-2.5 w-2.5" /> Collision suffix ({file.collision_suffix})
                            </span>
                          )}
                        </td>

                        <td className="py-2.5 px-3 whitespace-nowrap">
                          {isVerified ? (
                            <div className="flex items-center gap-1 text-emerald-400 text-[11px] font-medium">
                              <CheckCircle2 className="h-3 w-3 shrink-0" />
                              <span>Verified</span>
                              {file.sha256 && (
                                <span className="font-mono text-[9px] text-text-muted truncate max-w-[80px]" title={file.sha256}>
                                  ({file.sha256.slice(0, 6)}...)
                                </span>
                              )}
                            </div>
                          ) : isFileFailed ? (
                            <div className="flex items-center gap-1 text-danger text-[11px] font-medium" title={file.error_message || 'Failed'}>
                              <AlertTriangle className="h-3 w-3 shrink-0" />
                              <span className="truncate max-w-[120px]">{file.error_message || 'Failed'}</span>
                            </div>
                          ) : (
                            <span className="text-[11px] text-text-muted capitalize">{file.status}</span>
                          )}
                        </td>

                        <td className="py-2.5 px-3 text-right">
                          <button
                            onClick={() => setSelectedFileForModal(file)}
                            className="px-2 py-1 rounded bg-background border border-border hover:bg-surface-hover text-text-muted hover:text-text-primary transition-colors text-[11px]"
                            title="Inspect full file verification & metadata details"
                          >
                            <Info className="h-3 w-3" />
                          </button>
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>

          {/* Pagination Controls */}
          <div className="flex items-center justify-between border-t border-border px-4 py-2.5 bg-surface text-xs text-text-muted">
            <div>
              Showing {Math.min(files.length, pageSize)} of {totalFiles.toLocaleString()} files
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setPage(Math.max(0, page - 1))}
                disabled={page === 0 || isLoadingFiles}
                className="p-1 rounded border border-border hover:bg-surface-hover disabled:opacity-40 disabled:pointer-events-none text-text-primary transition-colors"
              >
                <ChevronLeft className="h-3.5 w-3.5" />
              </button>
              <span className="text-[11px] font-mono px-2">
                Page {page + 1} of {Math.max(1, totalPages)}
              </span>
              <button
                onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
                disabled={page >= totalPages - 1 || isLoadingFiles}
                className="p-1 rounded border border-border hover:bg-surface-hover disabled:opacity-40 disabled:pointer-events-none text-text-primary transition-colors"
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // ==========================================
  // VIEW 2: ALL JOBS LIST VIEW
  // ==========================================
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
            Select any past organization job to inspect transferred files, verification hashes, and full audit metrics.
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
              onClick={() => openJobDetail(job)}
              className="rounded-xl border border-border bg-surface hover:border-accent/50 hover:bg-surface-hover/50 p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 shadow-sm cursor-pointer transition-all group"
            >
              <div className="space-y-1.5 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-xs font-semibold text-text-primary group-hover:text-accent transition-colors">
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

              <div className="flex items-center gap-4 text-xs font-mono shrink-0">
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

                <div className="p-1 rounded bg-background border border-border group-hover:border-accent group-hover:bg-accent/10 text-text-muted group-hover:text-accent transition-all">
                  <ChevronRightIcon className="h-4 w-4" />
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
