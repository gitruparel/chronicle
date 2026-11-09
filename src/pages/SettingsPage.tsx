import { useState, useEffect } from 'react';
import {
  Cpu,
  Moon,
  Sun,
  HardDrive,
  CheckCircle2,
  AlertCircle,
  FileText,
} from 'lucide-react';
import { api } from '../lib/api';
import type { AppInfoResponse } from '../types';

interface SettingsPageProps {
  theme: 'dark' | 'light';
  onToggleTheme: () => void;
}

export function SettingsPage({ theme, onToggleTheme }: SettingsPageProps) {
  const [appInfo, setAppInfo] = useState<AppInfoResponse | null>(null);
  const [workers, setWorkers] = useState(4);
  const [verifyByDefault, setVerifyByDefault] = useState(true);
  const [mtimeFallback, setMtimeFallback] = useState(true);

  useEffect(() => {
    api.getAppInfo().then(setAppInfo).catch(console.error);
  }, []);

  return (
    <div className="max-w-4xl mx-auto space-y-6 py-4">
      {/* Title */}
      <div>
        <h2 className="text-lg font-semibold text-text-primary tracking-tight">Application Settings</h2>
        <p className="text-xs text-text-secondary mt-0.5">
          Configure runtime performance, verification policies, and view bundled licenses.
        </p>
      </div>

      {/* Engine & Concurrency Settings */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-4">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider flex items-center gap-2">
          <Cpu className="h-4 w-4 text-accent" />
          Concurrency & Performance
        </h3>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <span className="text-xs font-medium text-text-primary">Worker Thread Pool</span>
              <p className="text-[11px] text-text-muted">
                Number of concurrent copy streams. (Default: 4. Range: 1 to 16)
              </p>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min="1"
                max="16"
                value={workers}
                onChange={(e) => setWorkers(parseInt(e.target.value))}
                className="w-32 accent-accent cursor-pointer"
              />
              <span className="text-xs font-mono font-semibold text-accent w-6 text-right">
                {workers}
              </span>
            </div>
          </div>

          <div className="flex items-center justify-between border-t border-border/60 pt-3">
            <div className="space-y-0.5">
              <span className="text-xs font-medium text-text-primary">SHA-256 Checksum Verification</span>
              <p className="text-[11px] text-text-muted">
                Compute and verify cryptographic hash on-the-fly during every copy stream.
              </p>
            </div>
            <input
              type="checkbox"
              checked={verifyByDefault}
              onChange={(e) => setVerifyByDefault(e.target.checked)}
              className="rounded border-border bg-background text-accent focus:ring-accent"
            />
          </div>

          <div className="flex items-center justify-between border-t border-border/60 pt-3">
            <div className="space-y-0.5">
              <span className="text-xs font-medium text-text-primary">Filesystem Modified Date Fallback</span>
              <p className="text-[11px] text-text-muted">
                Fallback to filesystem mtime when embedded metadata is missing or unsupported.
              </p>
            </div>
            <input
              type="checkbox"
              checked={mtimeFallback}
              onChange={(e) => setMtimeFallback(e.target.checked)}
              className="rounded border-border bg-background text-accent focus:ring-accent"
            />
          </div>
        </div>
      </div>

      {/* Appearance Settings */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-4">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider flex items-center gap-2">
          {theme === 'dark' ? <Moon className="h-4 w-4 text-purple-400" /> : <Sun className="h-4 w-4 text-amber-400" />}
          Interface Theme
        </h3>

        <div className="flex items-center justify-between">
          <div className="space-y-0.5">
            <span className="text-xs font-medium text-text-primary">Current Theme: {theme === 'dark' ? 'Dark Mode' : 'Light Mode'}</span>
            <p className="text-[11px] text-text-muted">Optimized for high-contrast accessibility and long session focus.</p>
          </div>
          <button
            onClick={onToggleTheme}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-background hover:bg-surface-hover text-xs font-medium text-text-primary transition-colors"
          >
            {theme === 'dark' ? <Sun className="h-3.5 w-3.5 text-amber-400" /> : <Moon className="h-3.5 w-3.5 text-purple-400" />}
            Switch to {theme === 'dark' ? 'Light' : 'Dark'}
          </button>
        </div>
      </div>

      {/* ExifTool & Environment Status */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-3">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider flex items-center gap-2">
          <HardDrive className="h-4 w-4 text-emerald-400" />
          Environment & Engine Status
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
          <div className="p-3 rounded-lg border border-border bg-background space-y-1">
            <span className="text-text-muted">ExifTool Engine</span>
            <div className="flex items-center gap-2 font-medium text-text-primary">
              {appInfo?.exiftool_available ? (
                <>
                  <CheckCircle2 className="h-4 w-4 text-emerald-400" />
                  <span>Bundled Executable Active</span>
                </>
              ) : (
                <>
                  <AlertCircle className="h-4 w-4 text-amber-400" />
                  <span>ExifTool Unavailable (Using mtime)</span>
                </>
              )}
            </div>
            {appInfo?.exiftool_path && (
              <p className="font-mono text-[10px] text-text-muted break-all pt-1">
                {appInfo.exiftool_path}
              </p>
            )}
          </div>

          <div className="p-3 rounded-lg border border-border bg-background space-y-1">
            <span className="text-text-muted">Storage & Engine</span>
            <div className="flex items-center gap-2 font-medium text-text-primary">
              <CheckCircle2 className="h-4 w-4 text-emerald-400" />
              <span>Embedded SQLite WAL Mode</span>
            </div>
            <p className="text-[11px] text-text-muted pt-1">
              Idempotent crash recovery and transaction safety enabled.
            </p>
          </div>
        </div>
      </div>

      {/* Third Party Licenses & Notices */}
      <div className="rounded-xl border border-border bg-surface p-5 space-y-3">
        <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider flex items-center gap-2">
          <FileText className="h-4 w-4 text-accent" />
          Licenses & Third-Party Notices
        </h3>

        <div className="space-y-3 text-xs text-text-secondary leading-relaxed">
          <div className="p-3 rounded-lg border border-border bg-background space-y-1">
            <div className="font-semibold text-text-primary">ExifTool by Phil Harvey</div>
            <p className="text-[11px]">
              Copyright &copy; 2003-2026 Phil Harvey. ExifTool is free software distributed under the Perl Artistic License and the GNU General Public License (GPL). ExifTool is bundled as an external resource binary in <code>resources/exiftool.exe</code> and invoked via standard process interfaces. The complete license text is preserved in <code>resources/EXIFTOOL_LICENSE.txt</code>.
            </p>
          </div>

          <div className="p-3 rounded-lg border border-border bg-background space-y-1">
            <div className="font-semibold text-text-primary">SQLite Database Engine</div>
            <p className="text-[11px]">
              Dedicated to the Public Domain. All code and documentation in SQLite have been dedicated to the public domain by the authors.
            </p>
          </div>

          <div className="p-3 rounded-lg border border-border bg-background space-y-1">
            <div className="font-semibold text-text-primary">Tauri Framework</div>
            <p className="text-[11px]">
              Copyright &copy; Tauri Programme within The Commons Conservancy. Licensed under Apache 2.0 or MIT.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
