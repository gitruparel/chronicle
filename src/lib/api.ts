import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import type {
  AppInfoResponse,
  CreateJobResponse,
  JobOptions,
  JobProgress,
  JobRecord,
  JobSummary,
  OverlapCheckResult,
  PaginatedFilesResponse,
  PathValidationResult,
} from '../types';

export const api = {
  // Folder pickers via native dialog plugin
  async pickFolder(title: string): Promise<string | null> {
    const selected = await open({
      directory: true,
      multiple: false,
      title,
    });
    if (typeof selected === 'string') {
      return selected;
    }
    return null;
  },

  async pickSaveFile(defaultName: string, filterName: string, extensions: string[]): Promise<string | null> {
    const selected = await save({
      defaultPath: defaultName,
      filters: [{ name: filterName, extensions }],
    });
    return selected;
  },

  // Backend commands
  async validatePath(path: string): Promise<PathValidationResult> {
    return invoke<PathValidationResult>('validate_path', { path });
  },

  async checkOverlap(source: string, destination: string): Promise<OverlapCheckResult> {
    return invoke<OverlapCheckResult>('check_folder_overlap_cmd', { source, destination });
  },

  async createAndScanJob(
    name: string,
    sourcePath: string,
    destinationPath: string,
    options: JobOptions
  ): Promise<CreateJobResponse> {
    return invoke<CreateJobResponse>('create_and_scan_job', {
      name,
      sourcePath,
      destinationPath,
      options,
    });
  },

  async getPreviewFiles(
    jobId: string,
    page: number,
    pageSize: number,
    filter?: string,
    search?: string
  ): Promise<PaginatedFilesResponse> {
    return invoke<PaginatedFilesResponse>('get_preview_files', {
      jobId,
      page,
      pageSize,
      filter: filter === 'all' ? null : filter || null,
      search: search?.trim() ? search.trim() : null,
    });
  },

  async getJobSummary(jobId: string): Promise<JobSummary> {
    return invoke<JobSummary>('get_job_summary_cmd', { jobId });
  },

  async getAllJobs(): Promise<JobRecord[]> {
    return invoke<JobRecord[]>('get_all_jobs_cmd');
  },

  async startCopy(jobId: string, options: JobOptions): Promise<void> {
    return invoke<void>('start_copy_job', { jobId, options });
  },

  async pauseCopy(jobId: string): Promise<boolean> {
    return invoke<boolean>('pause_copy_job', { jobId });
  },

  async resumeCopy(jobId: string): Promise<boolean> {
    return invoke<boolean>('resume_copy_job', { jobId });
  },

  async cancelCopy(jobId: string): Promise<boolean> {
    return invoke<boolean>('cancel_copy_job', { jobId });
  },

  async retryFailed(jobId: string, options: JobOptions): Promise<number> {
    return invoke<number>('retry_failed_job', { jobId, options });
  },

  async exportReport(jobId: string, format: 'csv' | 'json', outputPath: string): Promise<void> {
    return invoke<void>('export_report', {
      jobId,
      format,
      outputPath,
    });
  },

  async openDestinationFolder(path: string): Promise<void> {
    return invoke<void>('open_destination_folder', { path });
  },

  async getAppInfo(): Promise<AppInfoResponse> {
    return invoke<AppInfoResponse>('get_app_info');
  },

  // Progress events subscription
  onJobProgress(callback: (progress: JobProgress) => void): Promise<UnlistenFn> {
    return listen<JobProgress>('job-progress', (event) => {
      callback(event.payload);
    });
  },
};
