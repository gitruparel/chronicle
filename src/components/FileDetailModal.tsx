import { X, Calendar, FileText, Tag, Shield, Clock, HardDrive } from 'lucide-react';
import type { PlannedFile } from '../types';
import { formatBytes, formatDateSourceLabel } from '../lib/utils';

interface FileDetailModalProps {
  file: PlannedFile | null;
  onClose: () => void;
}

export function FileDetailModal({ file, onClose }: FileDetailModalProps) {
  if (!file) return null;

  const sourceMeta = formatDateSourceLabel(file.date_resolution.date_source);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div className="w-full max-w-xl rounded-xl border border-border bg-surface shadow-2xl flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-3.5 bg-surface-hover/30">
          <div className="flex items-center gap-2.5 overflow-hidden">
            <FileText className="h-4 w-4 text-accent shrink-0" />
            <span className="font-semibold text-sm truncate" title={file.filename}>
              {file.filename}
            </span>
          </div>
          <button
            onClick={onClose}
            className="rounded p-1 text-text-muted hover:text-text-primary hover:bg-surface-hover transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5 space-y-4 max-h-[75vh] overflow-y-auto font-sans text-xs">
          {/* Destination Planning Box */}
          <div className="rounded-lg border border-border bg-background p-3.5 space-y-2">
            <div className="text-[11px] font-medium text-text-muted uppercase tracking-wider">
              Planned Destination
            </div>
            <div className="font-mono text-sm text-accent flex items-center gap-2">
              <Calendar className="h-4 w-4" />
              <span>{file.destination_folder} / {file.planned_filename}</span>
            </div>
            {file.is_collision && (
              <div className="text-amber-400 text-xs flex items-center gap-1.5 pt-1">
                <Tag className="h-3.5 w-3.5" />
                <span>Collision resolved with suffix: ({file.collision_suffix})</span>
              </div>
            )}
          </div>

          {/* Date Resolution Details */}
          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <span className="text-text-muted flex items-center gap-1.5">
                <Tag className="h-3.5 w-3.5" /> Date Source
              </span>
              <div className="pt-1">
                <span className={`inline-block px-2 py-0.5 rounded text-[11px] font-medium border ${sourceMeta.color}`}>
                  {sourceMeta.label}
                </span>
              </div>
            </div>

            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <span className="text-text-muted flex items-center gap-1.5">
                <Shield className="h-3.5 w-3.5" /> Confidence Level
              </span>
              <div className="pt-1 font-semibold capitalize text-text-primary">
                {file.date_resolution.confidence}
              </div>
            </div>

            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <span className="text-text-muted flex items-center gap-1.5">
                <Clock className="h-3.5 w-3.5" /> Raw Timestamp
              </span>
              <div className="pt-1 font-mono text-text-primary truncate" title={file.date_resolution.raw_timestamp || 'None'}>
                {file.date_resolution.raw_timestamp || 'None recorded'}
              </div>
            </div>

            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <span className="text-text-muted flex items-center gap-1.5">
                <HardDrive className="h-3.5 w-3.5" /> File Size
              </span>
              <div className="pt-1 font-mono text-text-primary">
                {formatBytes(file.size_bytes)} ({file.size_bytes.toLocaleString()} bytes)
              </div>
            </div>
          </div>

          {/* Reason / Rationale */}
          <div className="rounded-lg border border-border bg-background p-3 space-y-1">
            <div className="text-text-muted">Resolution Rationale</div>
            <p className="text-text-secondary leading-relaxed pt-0.5">
              {file.date_resolution.reason}
            </p>
          </div>

          {/* Timezone Information */}
          {file.date_resolution.timezone_info && (
            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <div className="text-text-muted">Timezone Metadata</div>
              <p className="font-mono text-text-primary">
                {file.date_resolution.timezone_info}
              </p>
            </div>
          )}

          {/* Source Path */}
          <div className="rounded-lg border border-border bg-background p-3 space-y-1">
            <div className="text-text-muted">Original Source Path</div>
            <p className="font-mono text-text-primary break-all select-text">
              {file.source_path}
            </p>
          </div>

          {/* Hash (if computed) */}
          {file.sha256 && (
            <div className="rounded-lg border border-border bg-background p-3 space-y-1">
              <div className="text-text-muted">SHA-256 Checksum</div>
              <p className="font-mono text-[11px] text-emerald-400 break-all select-text">
                {file.sha256}
              </p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-border px-5 py-3 flex justify-end bg-surface-hover/20">
          <button
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg text-xs font-medium bg-surface border border-border hover:bg-surface-hover text-text-primary transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
