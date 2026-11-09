import { useState, useEffect } from 'react';
import {
  Calendar,
  FolderOpen,
  Eye,
  Activity,
  History,
  Settings as SettingsIcon,
  Moon,
  Sun,
  ShieldCheck,
} from 'lucide-react';
import { NewJobPage } from './pages/NewJobPage';
import { PreviewPage } from './pages/PreviewPage';
import { ExecutionPage } from './pages/ExecutionPage';
import { CompletionPage } from './pages/CompletionPage';
import { SettingsPage } from './pages/SettingsPage';
import { HistoryPage } from './pages/HistoryPage';
import type { JobOptions, JobStatus, JobSummary } from './types';

type ActiveTab = 'new_job' | 'preview' | 'executing' | 'completed' | 'history' | 'settings';

interface PastJobRecord {
  id: string;
  date: string;
  source: string;
  destination: string;
  summary: JobSummary;
  status: string;
}

export function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>('new_job');
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');

  // Active Job State
  const [currentJobId, setCurrentJobId] = useState<string | null>(null);
  const [currentSummary, setCurrentSummary] = useState<JobSummary | null>(null);
  const [currentSource, setCurrentSource] = useState<string>('');
  const [currentDestination, setCurrentDestination] = useState<string>('');
  const [currentOptions, setCurrentOptions] = useState<JobOptions>({
    recursive: true,
    copy_duplicates: true,
    mtime_fallback: true,
    verify_hash: true,
    worker_count: 4,
  });
  const [completionStatus, setCompletionStatus] = useState<JobStatus>('completed');

  // Job History
  const [pastJobs, setPastJobs] = useState<PastJobRecord[]>([]);

  useEffect(() => {
    // Theme sync
    const root = document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
      root.classList.remove('light');
    } else {
      root.classList.add('light');
      root.classList.remove('dark');
    }
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === 'dark' ? 'light' : 'dark'));
  };

  const handleScanComplete = (
    jobId: string,
    summary: JobSummary,
    source: string,
    destination: string,
    options: JobOptions
  ) => {
    setCurrentJobId(jobId);
    setCurrentSummary(summary);
    setCurrentSource(source);
    setCurrentDestination(destination);
    setCurrentOptions(options);
    setActiveTab('preview');
  };

  const handleConfirmCopy = () => {
    setActiveTab('executing');
  };

  const handleCopyFinished = (status: JobStatus) => {
    setCompletionStatus(status);
    if (currentJobId && currentSummary) {
      setPastJobs((prev) => [
        {
          id: currentJobId,
          date: new Date().toLocaleDateString(),
          source: currentSource,
          destination: currentDestination,
          summary: currentSummary,
          status,
        },
        ...prev,
      ]);
    }
    setActiveTab('completed');
  };

  const handleStartNewJob = () => {
    setCurrentJobId(null);
    setCurrentSummary(null);
    setActiveTab('new_job');
  };

  const handleRetry = () => {
    setActiveTab('executing');
  };

  return (
    <div className="flex h-screen w-screen flex-col bg-background text-text-primary antialiased font-sans">
      {/* Top Application Bar */}
      <header className="flex h-12 items-center justify-between border-b border-border px-4 select-none shrink-0 bg-surface">
        {/* Left: Branding & Tagline */}
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-lg bg-accent/10 border border-accent/20 flex items-center justify-center text-accent">
              <Calendar className="h-4 w-4" />
            </div>
            <span className="font-semibold text-sm tracking-tight text-text-primary">Chronicle</span>
          </div>
          <span className="text-[11px] px-2 py-0.5 rounded bg-background border border-border text-text-muted font-mono">
            v1.0.0
          </span>
          <span className="hidden sm:inline-flex items-center gap-1 text-[11px] text-text-muted">
            <ShieldCheck className="h-3 w-3 text-emerald-400" /> Offline-First Organizer
          </span>
        </div>

        {/* Center: Navigation Tabs */}
        <nav className="flex items-center gap-1 bg-background p-0.5 rounded-lg border border-border text-xs">
          <button
            onClick={() => setActiveTab('new_job')}
            className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition-colors ${
              activeTab === 'new_job'
                ? 'bg-surface text-text-primary font-medium shadow-sm'
                : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            <FolderOpen className="h-3.5 w-3.5" />
            New Job
          </button>

          {currentJobId && (
            <button
              onClick={() => setActiveTab('preview')}
              className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition-colors ${
                activeTab === 'preview'
                  ? 'bg-surface text-text-primary font-medium shadow-sm'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
            >
              <Eye className="h-3.5 w-3.5" />
              Preview
            </button>
          )}

          {activeTab === 'executing' && (
            <button
              onClick={() => setActiveTab('executing')}
              className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-surface text-accent font-medium shadow-sm"
            >
              <Activity className="h-3.5 w-3.5 animate-pulse" />
              Copying...
            </button>
          )}

          {activeTab === 'completed' && (
            <button
              onClick={() => setActiveTab('completed')}
              className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-surface text-emerald-400 font-medium shadow-sm"
            >
              Completed
            </button>
          )}

          <button
            onClick={() => setActiveTab('history')}
            className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition-colors ${
              activeTab === 'history'
                ? 'bg-surface text-text-primary font-medium shadow-sm'
                : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            <History className="h-3.5 w-3.5" />
            History
          </button>

          <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition-colors ${
              activeTab === 'settings'
                ? 'bg-surface text-text-primary font-medium shadow-sm'
                : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            <SettingsIcon className="h-3.5 w-3.5" />
            Settings
          </button>
        </nav>

        {/* Right: Theme Toggle */}
        <div className="flex items-center gap-2">
          <button
            onClick={toggleTheme}
            className="p-1.5 rounded-lg border border-border bg-background hover:bg-surface-hover text-text-secondary hover:text-text-primary transition-colors"
            title={`Switch to ${theme === 'dark' ? 'Light' : 'Dark'} Mode`}
          >
            {theme === 'dark' ? <Sun className="h-4 w-4 text-amber-400" /> : <Moon className="h-4 w-4 text-purple-400" />}
          </button>
        </div>
      </header>

      {/* Main Content Viewport */}
      <main className="flex-1 overflow-y-auto px-6 py-4">
        {activeTab === 'new_job' && (
          <NewJobPage onScanComplete={handleScanComplete} />
        )}

        {activeTab === 'preview' && currentJobId && currentSummary && (
          <PreviewPage
            jobId={currentJobId}
            summary={currentSummary}
            onBack={() => setActiveTab('new_job')}
            onConfirmCopy={handleConfirmCopy}
          />
        )}

        {activeTab === 'executing' && currentJobId && currentSummary && (
          <ExecutionPage
            jobId={currentJobId}
            totalFiles={currentSummary.total_files}
            totalBytes={currentSummary.total_bytes}
            options={currentOptions}
            onFinished={handleCopyFinished}
          />
        )}

        {activeTab === 'completed' && currentJobId && currentSummary && (
          <CompletionPage
            jobId={currentJobId}
            status={completionStatus}
            summary={currentSummary}
            destinationPath={currentDestination}
            onNewJob={handleStartNewJob}
            onRetry={handleRetry}
          />
        )}

        {activeTab === 'history' && (
          <HistoryPage
            pastJobs={pastJobs}
            onSelectJob={() => {}}
          />
        )}

        {activeTab === 'settings' && (
          <SettingsPage theme={theme} onToggleTheme={toggleTheme} />
        )}
      </main>

      {/* Subtle Status Footer */}
      <footer className="h-7 border-t border-border px-4 flex items-center justify-between text-[11px] text-text-muted bg-surface select-none shrink-0 font-mono">
        <div className="flex items-center gap-3">
          <span>Chronicle Offline Engine</span>
          <span>&bull;</span>
          <span>Zero Telemetry</span>
        </div>
        <div className="flex items-center gap-3">
          <span>Non-destructive Copy</span>
          <span>&bull;</span>
          <span>SHA-256 Verified</span>
        </div>
      </footer>
    </div>
  );
}

export default App;
