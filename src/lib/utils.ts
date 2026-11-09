import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { DateSource } from '../types';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBytes(bytes: number, decimals = 1): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

export function formatDateSourceLabel(source: DateSource): { label: string; color: string } {
  switch (source) {
    case 'exif_original':
      return { label: 'EXIF Original', color: 'bg-blue-500/10 text-blue-400 border-blue-500/20' };
    case 'quicktime_creation':
      return { label: 'QuickTime', color: 'bg-purple-500/10 text-purple-400 border-purple-500/20' };
    case 'embedded_doc_audio':
      return { label: 'Embedded Media', color: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20' };
    case 'embedded_other':
      return { label: 'Embedded Other', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20' };
    case 'filesystem_modified':
      return { label: 'Modified Date', color: 'bg-amber-500/10 text-amber-400 border-amber-500/20' };
    case 'none':
    default:
      return { label: 'No Date', color: 'bg-slate-500/10 text-slate-400 border-slate-500/20' };
  }
}

export function formatTimeRemaining(seconds: number | null): string {
  if (seconds === null || seconds < 0 || !isFinite(seconds)) return '--';
  if (seconds < 60) return `${Math.round(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) return `${mins}m ${secs}s`;
  const hours = Math.floor(mins / 60);
  const remMins = mins % 60;
  return `${hours}h ${remMins}m`;
}
