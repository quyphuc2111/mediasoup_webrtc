import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ReactMarkdown from 'react-markdown';
import {
  Download,
  RefreshCw,
  CheckCircle,
  AlertCircle,
  Loader2,
  Package,
  Calendar,
  Monitor,
  Activity,
  FileText,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';

// Types matching Rust backend
interface UpdateInfo {
  version: string;
  published_at: string;
  download_url: string;
  sha256: string;
  signature?: string;
  release_notes: string;
  changelog_url?: string;
  min_app_version?: string;
}

type UpdateState =
  | { type: 'Idle' }
  | { type: 'Checking' }
  | { type: 'UpdateAvailable'; data: { version: string; release_notes: string } }
  | { type: 'Downloading'; data: { progress: number; bytes_downloaded: number; total_bytes: number } }
  | { type: 'Verifying' }
  | { type: 'ReadyToInstall' }
  | { type: 'Installing' }
  | { type: 'Restarting' }
  | { type: 'Done' }
  | { type: 'Failed'; data: { error: string; recoverable: boolean } };

interface ClientUpdateStatus {
  client_id: string;
  machine_name?: string;
  ip: string;
  current_version?: string;
  status: string;
  progress?: number;
  last_updated: number;
}

const UpdatesPage: React.FC = () => {
  const [currentVersion] = useState(import.meta.env.VITE_APP_VERSION || '0.1.0');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateState, setUpdateState] = useState<UpdateState>({ type: 'Idle' });
  const [isChecking, setIsChecking] = useState(false);
  const [clientStatuses, setClientStatuses] = useState<ClientUpdateStatus[]>([]);
  const [showChangelog, setShowChangelog] = useState(false);

  // Poll update state
  useEffect(() => {
    const pollState = async () => {
      try {
        const state = await invoke<UpdateState>('get_update_state');
        setUpdateState(state);
      } catch (error) {
        console.error('Failed to get update state:', error);
      }
    };

    pollState();
    const interval = setInterval(pollState, 1000);
    return () => clearInterval(interval);
  }, []);

  // Poll client update statuses
  useEffect(() => {
    const pollClients = async () => {
      try {
        const statuses = await invoke<ClientUpdateStatus[]>('get_client_update_status');
        setClientStatuses(statuses);
      } catch (error) {
        console.error('Failed to get client statuses:', error);
      }
    };

    pollClients();
    const interval = setInterval(pollClients, 2000);
    return () => clearInterval(interval);
  }, []);

  // Listen for update events
  useEffect(() => {
    const unlisten = listen('update-state-changed', (event: any) => {
      setUpdateState(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleCheckForUpdates = async () => {
    setIsChecking(true);
    try {
      const info = await invoke<UpdateInfo | null>('check_for_updates');
      if (info) {
        setUpdateInfo(info);
      }
    } catch (error) {
      console.error('Failed to check for updates:', error);
    } finally {
      setIsChecking(false);
    }
  };

  const handleDownload = async () => {
    try {
      await invoke('download_update');
    } catch (error) {
      console.error('Failed to download update:', error);
    }
  };

  const handleInstall = async () => {
    try {
      await invoke('install_update');
    } catch (error) {
      console.error('Failed to install update:', error);
    }
  };

  const handleRetry = async () => {
    // Retry by checking for updates again
    await handleCheckForUpdates();
  };

  const getStateDisplay = () => {
    switch (updateState.type) {
      case 'Idle':
        return { text: 'Idle', color: 'text-slate-500', icon: Package };
      case 'Checking':
        return { text: 'Checking for updates...', color: 'text-blue-600', icon: Loader2 };
      case 'UpdateAvailable':
        return { text: 'Update available', color: 'text-green-600', icon: CheckCircle };
      case 'Downloading':
        return { text: 'Downloading...', color: 'text-blue-600', icon: Download };
      case 'Verifying':
        return { text: 'Verifying...', color: 'text-blue-600', icon: Loader2 };
      case 'ReadyToInstall':
        return { text: 'Ready to install', color: 'text-green-600', icon: CheckCircle };
      case 'Installing':
        return { text: 'Installing...', color: 'text-blue-600', icon: Loader2 };
      case 'Restarting':
        return { text: 'Restarting...', color: 'text-blue-600', icon: RefreshCw };
      case 'Done':
        return { text: 'Update complete', color: 'text-green-600', icon: CheckCircle };
      case 'Failed':
        return { text: 'Update failed', color: 'text-rose-600', icon: AlertCircle };
      default:
        return { text: 'Unknown', color: 'text-slate-500', icon: Package };
    }
  };

  const stateDisplay = getStateDisplay();
  const StateIcon = stateDisplay.icon;

  const latestVersion = updateState.type === 'UpdateAvailable' ? updateState.data.version : updateInfo?.version;
  const publishDate = updateInfo?.published_at ? new Date(updateInfo.published_at).toLocaleDateString() : null;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">System Updates</h1>
          <p className="text-slate-500">Manage application updates and monitor client status</p>
        </div>
      </div>

      {/* Current Version Card */}
      <div className="bg-white rounded-3xl border border-slate-200 shadow-sm p-8">
        <div className="flex items-start justify-between">
          <div className="flex-1">
            <div className="flex items-center gap-3 mb-4">
              <div className="p-3 bg-indigo-50 rounded-2xl">
                <Package className="w-6 h-6 text-indigo-600" />
              </div>
              <div>
                <h2 className="text-lg font-bold text-slate-800">Current Version</h2>
                <p className="text-3xl font-black text-indigo-600 mt-1">{currentVersion}</p>
              </div>
            </div>

            {latestVersion && (
              <div className="flex items-center gap-4 mt-4 pt-4 border-t border-slate-100">
                <div>
                  <p className="text-xs font-bold text-slate-400 uppercase tracking-widest">Latest Version</p>
                  <p className="text-xl font-bold text-slate-700 mt-1">{latestVersion}</p>
                </div>
                {publishDate && (
                  <div>
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest">Published</p>
                    <div className="flex items-center gap-2 mt-1">
                      <Calendar className="w-4 h-4 text-slate-400" />
                      <p className="text-sm font-semibold text-slate-600">{publishDate}</p>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="flex flex-col items-end gap-3">
            <div className={`flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-bold ${stateDisplay.color} bg-slate-50`}>
              <StateIcon className={`w-4 h-4 ${updateState.type === 'Checking' || updateState.type === 'Downloading' || updateState.type === 'Verifying' || updateState.type === 'Installing' ? 'animate-spin' : ''}`} />
              {stateDisplay.text}
            </div>

            <div className="flex gap-2">
              <button
                onClick={handleCheckForUpdates}
                disabled={isChecking || updateState.type === 'Downloading' || updateState.type === 'Installing'}
                className="flex items-center gap-2 px-5 py-2.5 bg-slate-800 text-white rounded-xl text-sm font-bold hover:bg-slate-900 transition disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <RefreshCw className={`w-4 h-4 ${isChecking ? 'animate-spin' : ''}`} />
                Check Now
              </button>

              {updateState.type === 'UpdateAvailable' && (
                <button
                  onClick={handleDownload}
                  className="flex items-center gap-2 px-5 py-2.5 bg-indigo-600 text-white rounded-xl text-sm font-bold hover:bg-indigo-700 transition"
                >
                  <Download className="w-4 h-4" />
                  Download
                </button>
              )}

              {updateState.type === 'ReadyToInstall' && (
                <button
                  onClick={handleInstall}
                  className="flex items-center gap-2 px-5 py-2.5 bg-green-600 text-white rounded-xl text-sm font-bold hover:bg-green-700 transition"
                >
                  <CheckCircle className="w-4 h-4" />
                  Install
                </button>
              )}

              {updateState.type === 'Failed' && (
                <button
                  onClick={handleRetry}
                  className="flex items-center gap-2 px-5 py-2.5 bg-rose-600 text-white rounded-xl text-sm font-bold hover:bg-rose-700 transition"
                >
                  <RefreshCw className="w-4 h-4" />
                  Retry
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Download Progress */}
        {updateState.type === 'Downloading' && (
          <div className="mt-6 pt-6 border-t border-slate-100">
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm font-bold text-slate-600">Downloading update...</span>
              <span className="text-sm font-bold text-indigo-600">{updateState.data.progress.toFixed(1)}%</span>
            </div>
            <div className="w-full h-3 bg-slate-100 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-indigo-500 to-indigo-600 transition-all duration-300"
                style={{ width: `${updateState.data.progress}%` }}
              />
            </div>
            <div className="flex items-center justify-between mt-2">
              <span className="text-xs text-slate-400">
                {(updateState.data.bytes_downloaded / 1024 / 1024).toFixed(2)} MB / {(updateState.data.total_bytes / 1024 / 1024).toFixed(2)} MB
              </span>
            </div>
          </div>
        )}

        {/* Error Display */}
        {updateState.type === 'Failed' && (
          <div className="mt-6 pt-6 border-t border-slate-100">
            <div className="flex items-start gap-3 p-4 bg-rose-50 border border-rose-200 rounded-2xl">
              <AlertCircle className="w-5 h-5 text-rose-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-bold text-rose-900">Update Failed</p>
                <p className="text-sm text-rose-700 mt-1">{updateState.data.error}</p>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Changelog Display */}
      {(updateState.type === 'UpdateAvailable' || updateInfo) && (
        <div className="bg-white rounded-3xl border border-slate-200 shadow-sm overflow-hidden">
          <button
            onClick={() => setShowChangelog(!showChangelog)}
            className="w-full p-6 flex items-center justify-between hover:bg-slate-50 transition"
          >
            <div className="flex items-center gap-3">
              <div className="p-2 bg-green-50 rounded-xl">
                <FileText className="w-5 h-5 text-green-600" />
              </div>
              <div className="text-left">
                <h2 className="text-lg font-bold text-slate-800">Release Notes</h2>
                <p className="text-sm text-slate-500">
                  What's new in version {updateState.type === 'UpdateAvailable' ? updateState.data.version : updateInfo?.version}
                </p>
              </div>
            </div>
            {showChangelog ? (
              <ChevronUp className="w-5 h-5 text-slate-400" />
            ) : (
              <ChevronDown className="w-5 h-5 text-slate-400" />
            )}
          </button>

          {showChangelog && (
            <div className="px-6 pb-6 border-t border-slate-100">
              <div className="prose prose-slate max-w-none mt-4">
                <ReactMarkdown
                  components={{
                    h1: ({ children }) => (
                      <h1 className="text-2xl font-bold text-slate-800 mb-4">{children}</h1>
                    ),
                    h2: ({ children }) => (
                      <h2 className="text-xl font-bold text-slate-800 mb-3 mt-6">{children}</h2>
                    ),
                    h3: ({ children }) => (
                      <h3 className="text-lg font-bold text-slate-700 mb-2 mt-4">{children}</h3>
                    ),
                    p: ({ children }) => (
                      <p className="text-slate-600 mb-3 leading-relaxed">{children}</p>
                    ),
                    ul: ({ children }) => (
                      <ul className="list-disc list-inside space-y-2 mb-4 text-slate-600">{children}</ul>
                    ),
                    ol: ({ children }) => (
                      <ol className="list-decimal list-inside space-y-2 mb-4 text-slate-600">{children}</ol>
                    ),
                    li: ({ children }) => (
                      <li className="ml-4">{children}</li>
                    ),
                    code: ({ children }) => (
                      <code className="px-2 py-1 bg-slate-100 text-slate-800 rounded text-sm font-mono">
                        {children}
                      </code>
                    ),
                    pre: ({ children }) => (
                      <pre className="bg-slate-900 text-slate-100 p-4 rounded-xl overflow-x-auto mb-4">
                        {children}
                      </pre>
                    ),
                    blockquote: ({ children }) => (
                      <blockquote className="border-l-4 border-indigo-500 pl-4 italic text-slate-600 my-4">
                        {children}
                      </blockquote>
                    ),
                    a: ({ href, children }) => (
                      <a
                        href={href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-indigo-600 hover:text-indigo-700 underline"
                      >
                        {children}
                      </a>
                    ),
                  }}
                >
                  {updateState.type === 'UpdateAvailable'
                    ? updateState.data.release_notes
                    : updateInfo?.release_notes || 'No release notes available.'}
                </ReactMarkdown>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Client Status Table */}
      <div className="bg-white rounded-3xl border border-slate-200 shadow-sm overflow-hidden">
        <div className="p-6 border-b border-slate-100 bg-slate-50/50">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-indigo-50 rounded-xl">
              <Monitor className="w-5 h-5 text-indigo-600" />
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-800">Connected Students</h2>
              <p className="text-sm text-slate-500">Monitor update status across all clients</p>
            </div>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="text-[10px] uppercase font-black text-slate-400 border-b border-slate-100 bg-slate-50/30">
                <th className="px-6 py-4">Machine Name</th>
                <th className="px-6 py-4">IP Address</th>
                <th className="px-6 py-4">Version</th>
                <th className="px-6 py-4">Status</th>
                <th className="px-6 py-4">Progress</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {clientStatuses.length === 0 ? (
                <tr>
                  <td colSpan={5} className="px-6 py-12 text-center">
                    <div className="flex flex-col items-center gap-3">
                      <div className="p-4 bg-slate-50 rounded-2xl">
                        <Monitor className="w-8 h-8 text-slate-300" />
                      </div>
                      <p className="text-sm font-semibold text-slate-400">No students connected</p>
                    </div>
                  </td>
                </tr>
              ) : (
                clientStatuses.map((client) => (
                  <tr key={client.client_id} className="hover:bg-slate-50 transition">
                    <td className="px-6 py-4">
                      <div className="flex items-center gap-2">
                        <Activity className="w-4 h-4 text-slate-400" />
                        <span className="font-bold text-slate-700">{client.machine_name || 'Unknown'}</span>
                      </div>
                    </td>
                    <td className="px-6 py-4">
                      <span className="text-sm font-mono text-slate-600">{client.ip}</span>
                    </td>
                    <td className="px-6 py-4">
                      <span className="text-sm font-semibold text-slate-700">{client.current_version || 'Unknown'}</span>
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold ${
                          client.status === 'UpToDate'
                            ? 'bg-green-50 text-green-700'
                            : client.status === 'Downloading'
                            ? 'bg-blue-50 text-blue-700'
                            : client.status === 'Failed'
                            ? 'bg-rose-50 text-rose-700'
                            : 'bg-slate-50 text-slate-700'
                        }`}
                      >
                        {client.status}
                      </span>
                    </td>
                    <td className="px-6 py-4">
                      {client.progress !== undefined && client.progress !== null ? (
                        <div className="flex items-center gap-2">
                          <div className="flex-1 h-2 bg-slate-100 rounded-full overflow-hidden max-w-[100px]">
                            <div
                              className="h-full bg-indigo-600 transition-all"
                              style={{ width: `${client.progress}%` }}
                            />
                          </div>
                          <span className="text-xs font-bold text-slate-600">{client.progress.toFixed(0)}%</span>
                        </div>
                      ) : (
                        <span className="text-xs text-slate-400">—</span>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

export default UpdatesPage;
