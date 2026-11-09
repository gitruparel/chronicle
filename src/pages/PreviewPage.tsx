import { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Calendar,
  FileText,
  Search,
  Filter,
  ArrowRight,
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  Info,
  Clock,
  Layers,
  Copy,
  AlertCircle,
  Tag,
  ShieldAlert,
} from 'lucide-react';
import { api } from '../lib/api';
import type { JobSummary, PlannedFile } from '../types';
import { formatBytes, formatDateSourceLabel } from '../lib/utils';
import { FileDetailModal } from '../components/FileDetailModal';

interface PreviewPageProps {
  jobId: string;
  summary: JobSummary;
  onBack: () => void;
  onConfirmCopy: () => void;
}

export function PreviewPage({ jobId, summary, onBack, onConfirmCopy }: PreviewPageProps) {
  const [files, setFiles] = useState<PlannedFile[]>([]);
  const [page, setPage] = useState(0);
  const [pageSize] = useState(50);
  const [totalFiles, setTotalFiles] = useState(summary.total_files);
  const [isLoading, setIsLoading] = useState(true);
  const [searchTerm, setSearchTerm] = useState('');
  const [sourceFilter, setSourceFilter] = useState<string>('all');
  const [selectedFile, setSelectedFile] = useState<PlannedFile | null>(null);

  const fetchFiles = useCallback(async (p: number) => {
    setIsLoading(true);
    try {
      const res = await api.getPreviewFiles(jobId, p, pageSize);
      setFiles(res.files);
      setTotalFiles(res.total);
    } catch (e) {
      console.error('Failed to fetch preview files:', e);
    } finally {
      setIsLoading(false);
    }
  }, [jobId, pageSize]);

  useEffect(() => {
    fetchFiles(page);
  }, [page, fetchFiles]);

  // Client-side filtering on current page
  const filteredFiles = useMemo(() => {
    return files.filter((f) => {
      const matchesSearch =
        f.filename.toLowerCase().includes(searchTerm.toLowerCase()) ||
        f.relative_path.toLowerCase().includes(searchTerm.toLowerCase());

      if (!matchesSearch) return false;

      if (sourceFilter === 'embedded') {
        return ['exif_original', 'quicktime_creation', 'embedded_doc_audio', 'embedded_other'].includes(
          f.date_resolution.date_source
        );
      }
      if (sourceFilter === 'modified') {
        return f.date_resolution.date_source === 'filesystem_modified';
      }
      if (sourceFilter === 'no_date') {
        return f.date_resolution.date_source === 'none';
      }
      if (sourceFilter === 'collisions') {
        return f.is_collision;
      }
      if (sourceFilter === 'excluded') {
        return f.status === 'excluded';
      }
      if (sourceFilter === 'errors') {
        return f.status === 'failed' || f.destination_folder === 'Scan Errors';
      }
      return true;
    });
  }, [files, searchTerm, sourceFilter]);

  const totalPages = Math.ceil(totalFiles / pageSize);

  return (
    <div className="max-w-6xl mx-auto space-y-5 py-2">
      {/* Detail Modal */}
      <FileDetailModal file={selectedFile} onClose={() => setSelectedFile(null)} />

      {/* Top Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-text-primary tracking-tight">Organization Preview</h2>
          <p className="text-xs text-text-secondary mt-0.5">
            Review resolved dates and planned destination paths before executing copies.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={onBack}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-surface hover:bg-surface-hover text-xs font-medium text-text-secondary hover:text-text-primary transition-colors"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            Back
          </button>
          <button
            onClick={onConfirmCopy}
            className="flex items-center gap-1.5 px-5 py-1.5 rounded-lg bg-accent hover:bg-accent-hover text-white text-xs font-semibold shadow-lg shadow-accent/20 transition-all"
          >
            Confirm & Copy Files
            <ArrowRight className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {/* Summary KPI Cards */}
      <div className="grid grid-cols-2 md:grid-cols-6 gap-3">
        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-text-muted flex items-center gap-1">
            <FileText className="h-3 w-3" /> Total Files
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.total_files.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted font-mono">{formatBytes(summary.total_bytes)}</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-blue-400 flex items-center gap-1">
            <Calendar className="h-3 w-3" /> Embedded Dates
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.embedded_date_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">EXIF / Media / Docs</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-amber-400 flex items-center gap-1">
            <Clock className="h-3 w-3" /> Modified Fallback
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.mtime_date_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">Filesystem mtime</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-slate-400 flex items-center gap-1">
            <AlertCircle className="h-3 w-3" /> No Date
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.no_date_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">Routes to No Date/</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-indigo-400 flex items-center gap-1">
            <Layers className="h-3 w-3" /> Date Folders
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.unique_folders_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">Unique YYYY-MM-DD</span>
        </div>

        <div className="rounded-xl border border-border bg-surface p-3 space-y-1">
          <span className="text-[11px] text-purple-400 flex items-center gap-1">
            <Copy className="h-3 w-3" /> Duplicates
          </span>
          <div className="text-base font-semibold text-text-primary">
            {summary.duplicate_content_count.toLocaleString()}
          </div>
          <span className="text-[10px] text-text-muted">All copied safely</span>
        </div>
      </div>

      {/* Accountability Banner if non-regular files or errors exist */}
      {(summary.scan_error_count > 0 || summary.excluded_count > 0) && (
        <div className="flex items-center gap-3 p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-300 text-xs">
          <ShieldAlert className="h-4 w-4 shrink-0 text-amber-400" />
          <div className="flex-1 flex flex-wrap items-center gap-x-4 gap-y-1">
            <span><strong>Zero-Drop Accountability:</strong> {summary.total_files.toLocaleString()} total items discovered.</span>
            {summary.scan_error_count > 0 && <span>&bull; {summary.scan_error_count} unreadable traversal errors</span>}
            {summary.excluded_count > 0 && <span>&bull; {summary.excluded_count} policy-excluded (symlinks/devices)</span>}
            <span>&bull; {summary.total_files - summary.scan_error_count - summary.excluded_count} regular files planned for copy.</span>
          </div>
        </div>
      )}

      {/* Search and Filters Bar */}
      <div className="flex flex-col sm:flex-row items-center justify-between gap-3 bg-surface p-2.5 rounded-xl border border-border">
        <div className="relative w-full sm:w-80">
          <Search className="absolute left-3 top-2.5 h-3.5 w-3.5 text-text-muted" />
          <input
            type="text"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Search filename or path..."
            className="w-full bg-background border border-border rounded-lg pl-8 pr-3 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent"
          />
        </div>

        <div className="flex items-center gap-2 w-full sm:w-auto overflow-x-auto">
          <span className="text-[11px] text-text-muted flex items-center gap-1 shrink-0">
            <Filter className="h-3 w-3" /> Filter:
          </span>
          {[
            { id: 'all', label: 'All Files' },
            { id: 'embedded', label: 'Embedded Dates' },
            { id: 'modified', label: 'Modified Date' },
            { id: 'no_date', label: 'No Date' },
            { id: 'collisions', label: 'Collisions' },
            ...(summary.excluded_count > 0 ? [{ id: 'excluded', label: `Excluded (${summary.excluded_count})` }] : []),
            ...(summary.scan_error_count > 0 ? [{ id: 'errors', label: `Errors (${summary.scan_error_count})` }] : []),
          ].map((item) => (
            <button
              key={item.id}
              onClick={() => setSourceFilter(item.id)}
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

      {/* Preview Table */}
      <div className="rounded-xl border border-border bg-surface overflow-hidden shadow-sm">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="border-b border-border bg-surface-hover/50 text-[11px] font-semibold text-text-muted uppercase tracking-wider">
                <th className="py-2.5 px-3">Filename / Source</th>
                <th className="py-2.5 px-3">Size</th>
                <th className="py-2.5 px-3">Resolved Date</th>
                <th className="py-2.5 px-3">Source Tag</th>
                <th className="py-2.5 px-3">Target Folder & Name</th>
                <th className="py-2.5 px-3 text-right">Details</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {isLoading ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-text-muted text-xs">
                    Loading preview files...
                  </td>
                </tr>
              ) : filteredFiles.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-text-muted text-xs">
                    No files matching current criteria.
                  </td>
                </tr>
              ) : (
                filteredFiles.map((file) => {
                  const tagMeta = formatDateSourceLabel(file.date_resolution.date_source);
                  return (
                    <tr
                      key={file.id}
                      className="hover:bg-surface-hover/40 transition-colors group"
                    >
                      <td className="py-2.5 px-3 max-w-[280px]">
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

                      <td className="py-2.5 px-3 max-w-[280px]">
                        <div className="font-mono text-accent text-[11px] truncate" title={`${file.destination_folder}/${file.planned_filename}`}>
                          {file.destination_folder} / {file.planned_filename}
                        </div>
                        {file.is_collision && (
                          <span className="inline-flex items-center gap-1 text-[10px] text-amber-400 font-medium">
                            <Tag className="h-2.5 w-2.5" /> Collision suffix ({file.collision_suffix})
                          </span>
                        )}
                      </td>

                      <td className="py-2.5 px-3 text-right">
                        <button
                          onClick={() => setSelectedFile(file)}
                          className="px-2 py-1 rounded bg-background border border-border hover:bg-surface-hover text-text-muted hover:text-text-primary transition-colors text-[11px]"
                          title="Inspect metadata details"
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

        {/* Pagination Bar */}
        <div className="flex items-center justify-between border-t border-border px-4 py-2.5 bg-surface text-xs text-text-muted">
          <div>
            Showing {Math.min(filteredFiles.length, pageSize)} of {totalFiles.toLocaleString()} files
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setPage(Math.max(0, page - 1))}
              disabled={page === 0 || isLoading}
              className="p-1 rounded border border-border hover:bg-surface-hover disabled:opacity-40 disabled:pointer-events-none text-text-primary transition-colors"
            >
              <ChevronLeft className="h-3.5 w-3.5" />
            </button>
            <span className="text-[11px] font-mono px-2">
              Page {page + 1} of {Math.max(1, totalPages)}
            </span>
            <button
              onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
              disabled={page >= totalPages - 1 || isLoading}
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
