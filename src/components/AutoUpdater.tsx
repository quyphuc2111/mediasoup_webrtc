import React, { useState, useEffect } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { Download, RefreshCw, CheckCircle, AlertCircle, X } from 'lucide-react';

const AutoUpdater: React.FC = () => {
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [showDialog, setShowDialog] = useState(false);

  // Check for updates on mount
  useEffect(() => {
    checkForUpdates();
  }, []);

  const checkForUpdates = async () => {
    setChecking(true);
    setError(null);
    
    try {
      const update = await check();
      
      if (update) {
        setUpdateAvailable(true);
        setUpdateInfo(update);
        setShowDialog(true);
        console.log(
          `Update available: ${update.version}, ${update.date}, ${update.body}`
        );
      } else {
        console.log('No updates available');
        setUpdateAvailable(false);
      }
    } catch (err) {
      console.error('Failed to check for updates:', err);
      setError('Không thể kiểm tra cập nhật: ' + err);
    } finally {
      setChecking(false);
    }
  };

  const downloadAndInstall = async () => {
    if (!updateInfo) return;

    setDownloading(true);
    setError(null);

    try {
      // Download and install the update
      await updateInfo.downloadAndInstall((event: any) => {
        switch (event.event) {
          case 'Started':
            console.log(`Started downloading ${event.data.contentLength} bytes`);
            break;
          case 'Progress':
            const percent = (event.data.chunkLength / event.data.contentLength) * 100;
            setProgress(percent);
            console.log(`Downloaded ${event.data.chunkLength} bytes`);
            break;
          case 'Finished':
            console.log('Download finished');
            break;
        }
      });

      // Relaunch the app to apply the update
      console.log('Update installed, relaunching...');
      await relaunch();
    } catch (err) {
      console.error('Failed to install update:', err);
      setError('Không thể cài đặt cập nhật: ' + err);
      setDownloading(false);
    }
  };

  if (!showDialog && !updateAvailable) {
    return (
      <button
        onClick={checkForUpdates}
        disabled={checking}
        className="flex items-center gap-2 px-4 py-2 text-sm text-slate-600 hover:bg-slate-100 rounded-xl transition"
        title="Kiểm tra cập nhật"
      >
        <RefreshCw className={`w-4 h-4 ${checking ? 'animate-spin' : ''}`} />
        {checking ? 'Đang kiểm tra...' : 'Kiểm tra cập nhật'}
      </button>
    );
  }

  if (!showDialog) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-3xl shadow-2xl max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="bg-gradient-to-r from-indigo-500 to-purple-500 p-6 text-white">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-white/20 rounded-xl">
                <Download className="w-6 h-6" />
              </div>
              <div>
                <h3 className="font-bold text-lg">Cập nhật mới</h3>
                <p className="text-sm text-white/80">
                  Phiên bản {updateInfo?.version}
                </p>
              </div>
            </div>
            <button
              onClick={() => setShowDialog(false)}
              className="p-2 hover:bg-white/20 rounded-xl transition"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4">
          {/* Release Notes */}
          {updateInfo?.body && (
            <div className="bg-slate-50 rounded-2xl p-4">
              <h4 className="font-bold text-slate-800 mb-2 text-sm">
                📝 Nội dung cập nhật:
              </h4>
              <div className="text-sm text-slate-600 whitespace-pre-wrap">
                {updateInfo.body}
              </div>
            </div>
          )}

          {/* Release Date */}
          {updateInfo?.date && (
            <div className="flex items-center gap-2 text-sm text-slate-500">
              <CheckCircle className="w-4 h-4" />
              <span>Ngày phát hành: {updateInfo.date}</span>
            </div>
          )}

          {/* Progress Bar */}
          {downloading && (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-slate-600">Đang tải xuống...</span>
                <span className="font-bold text-indigo-600">
                  {Math.round(progress)}%
                </span>
              </div>
              <div className="h-2 bg-slate-200 rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-indigo-500 to-purple-500 transition-all duration-300"
                  style={{ width: `${progress}%` }}
                />
              </div>
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="flex items-center gap-2 p-3 bg-rose-50 text-rose-600 rounded-xl text-sm">
              <AlertCircle className="w-4 h-4 flex-shrink-0" />
              <span>{error}</span>
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="p-6 pt-0 flex gap-3">
          <button
            onClick={() => setShowDialog(false)}
            disabled={downloading}
            className="flex-1 px-4 py-3 bg-slate-100 text-slate-700 rounded-xl font-medium hover:bg-slate-200 transition disabled:opacity-50"
          >
            Để sau
          </button>
          <button
            onClick={downloadAndInstall}
            disabled={downloading}
            className="flex-1 px-4 py-3 bg-gradient-to-r from-indigo-500 to-purple-500 text-white rounded-xl font-bold hover:shadow-lg transition disabled:opacity-50 flex items-center justify-center gap-2"
          >
            {downloading ? (
              <>
                <RefreshCw className="w-5 h-5 animate-spin" />
                Đang cài đặt...
              </>
            ) : (
              <>
                <Download className="w-5 h-5" />
                Cập nhật ngay
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default AutoUpdater;
