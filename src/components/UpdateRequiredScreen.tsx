import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Download, AlertCircle, CheckCircle, Loader2, RefreshCw } from 'lucide-react';

// Student update state types matching Rust backend
interface StudentUpdateState {
  type: 'Idle' | 'UpdateRequired' | 'Downloading' | 'Verifying' | 'ReadyToInstall' | 'Installing' | 'Restarting' | 'Done' | 'Failed';
  data?: {
    current_version?: string;
    required_version?: string;
    update_url?: string;
    sha256?: string;
    progress?: number;
    bytes_downloaded?: number;
    total_bytes?: number;
    retry_count?: number;
    download_path?: string;
    error?: string;
    can_retry?: boolean;
  };
}

interface UpdateRequiredScreenProps {
  onUpdateComplete?: () => void;
}

const UpdateRequiredScreen: React.FC<UpdateRequiredScreenProps> = ({ onUpdateComplete }) => {
  const [updateState, setUpdateState] = useState<StudentUpdateState>({ type: 'Idle' });
  const [downloadSpeed, setDownloadSpeed] = useState<string>('');
  const [lastBytesDownloaded, setLastBytesDownloaded] = useState<number>(0);
  const [lastUpdateTime, setLastUpdateTime] = useState<number>(Date.now());

  // Calculate download speed
  useEffect(() => {
    if (updateState.type === 'Downloading' && updateState.data?.bytes_downloaded) {
      const now = Date.now();
      const timeDiff = (now - lastUpdateTime) / 1000; // seconds
      const bytesDiff = updateState.data.bytes_downloaded - lastBytesDownloaded;

      if (timeDiff > 0 && bytesDiff > 0) {
        const speedBps = bytesDiff / timeDiff;
        const speedMbps = (speedBps / (1024 * 1024)).toFixed(2);
        setDownloadSpeed(`${speedMbps} MB/s`);
      }

      setLastBytesDownloaded(updateState.data.bytes_downloaded);
      setLastUpdateTime(now);
    }
  }, [updateState]);

  // Listen for update state changes
  useEffect(() => {
    const unlisten = listen<{ state: StudentUpdateState; timestamp: number }>('student-update-state-changed', (event) => {
      console.log('[UpdateRequiredScreen] State changed:', event.payload);
      const newState = event.payload.state;
      setUpdateState(newState);
    });

    // Get initial state
    invoke<StudentUpdateState>('get_student_update_state')
      .then((state) => {
        console.log('[UpdateRequiredScreen] Initial state:', state);
        setUpdateState(state);
      })
      .catch((error) => {
        console.error('[UpdateRequiredScreen] Failed to get initial state:', error);
      });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Auto-start download when update is required
  useEffect(() => {
    if (updateState.type === 'UpdateRequired') {
      console.log('[UpdateRequiredScreen] Auto-starting download...');
      handleStartDownload();
    }
  }, [updateState.type]);

  // Auto-install when ready
  useEffect(() => {
    if (updateState.type === 'ReadyToInstall') {
      console.log('[UpdateRequiredScreen] Auto-starting installation...');
      handleInstall();
    }
  }, [updateState.type]);

  // Handle update completion
  useEffect(() => {
    if (updateState.type === 'Done' && onUpdateComplete) {
      onUpdateComplete();
    }
  }, [updateState.type, onUpdateComplete]);

  const handleStartDownload = async () => {
    try {
      console.log('[UpdateRequiredScreen] Starting download...');
      await invoke('download_student_update');
    } catch (error) {
      console.error('[UpdateRequiredScreen] Failed to start download:', error);
    }
  };

  const handleRetry = async () => {
    try {
      console.log('[UpdateRequiredScreen] Retrying download...');
      await invoke('retry_student_update');
    } catch (error) {
      console.error('[UpdateRequiredScreen] Failed to retry download:', error);
    }
  };

  const handleInstall = async () => {
    try {
      console.log('[UpdateRequiredScreen] Installing update...');
      await invoke('install_student_update');
    } catch (error) {
      console.error('[UpdateRequiredScreen] Failed to install update:', error);
    }
  };

  // Calculate progress percentage
  const getProgressPercentage = (): number => {
    if (updateState.type === 'Downloading' && updateState.data) {
      return updateState.data.progress || 0;
    }
    return 0;
  };

  // Format bytes to human-readable
  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  // Render different states
  const renderContent = () => {
    switch (updateState.type) {
      case 'UpdateRequired':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-amber-100 rounded-full flex items-center justify-center">
              <Download className="w-12 h-12 text-amber-600" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Cập nhật bắt buộc</h2>
            <p className="text-slate-600 mb-6">
              Phiên bản hiện tại: <span className="font-bold">{updateState.data?.current_version}</span>
            </p>
            <p className="text-slate-600 mb-8">
              Phiên bản yêu cầu: <span className="font-bold text-indigo-600">{updateState.data?.required_version}</span>
            </p>
            <div className="flex items-center justify-center gap-2 text-slate-500">
              <Loader2 className="w-5 h-5 animate-spin" />
              <span>Đang chuẩn bị tải xuống...</span>
            </div>
          </div>
        );

      case 'Downloading':
        const progress = getProgressPercentage();
        const bytesDownloaded = updateState.data?.bytes_downloaded || 0;
        const totalBytes = updateState.data?.total_bytes || 0;
        const retryCount = updateState.data?.retry_count || 0;

        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-indigo-100 rounded-full flex items-center justify-center">
              <Download className="w-12 h-12 text-indigo-600 animate-bounce" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Đang tải xuống cập nhật</h2>
            
            {retryCount > 0 && (
              <p className="text-amber-600 mb-4 font-semibold">
                Lần thử lại: {retryCount}/3
              </p>
            )}

            <div className="mb-6">
              <div className="flex justify-between text-sm text-slate-600 mb-2">
                <span>{progress.toFixed(1)}%</span>
                <span>{downloadSpeed}</span>
              </div>
              <div className="w-full h-4 bg-slate-200 rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-indigo-500 to-indigo-600 transition-all duration-300 ease-out"
                  style={{ width: `${progress}%` }}
                />
              </div>
              {totalBytes > 0 && (
                <p className="text-sm text-slate-500 mt-2">
                  {formatBytes(bytesDownloaded)} / {formatBytes(totalBytes)}
                </p>
              )}
            </div>

            <p className="text-slate-500 text-sm">
              Vui lòng chờ trong khi hệ thống tải xuống bản cập nhật...
            </p>
          </div>
        );

      case 'Verifying':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-purple-100 rounded-full flex items-center justify-center">
              <Loader2 className="w-12 h-12 text-purple-600 animate-spin" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Đang xác minh</h2>
            <p className="text-slate-600">
              Đang kiểm tra tính toàn vẹn của tệp cập nhật...
            </p>
          </div>
        );

      case 'ReadyToInstall':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-emerald-100 rounded-full flex items-center justify-center">
              <CheckCircle className="w-12 h-12 text-emerald-600" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Sẵn sàng cài đặt</h2>
            <p className="text-slate-600 mb-8">
              Bản cập nhật đã được tải xuống và xác minh thành công.
            </p>
            <button
              onClick={handleInstall}
              className="px-8 py-4 bg-indigo-600 text-white rounded-2xl font-black uppercase tracking-widest hover:bg-indigo-700 transition-all hover:scale-105 active:scale-95"
            >
              Cài đặt ngay
            </button>
          </div>
        );

      case 'Installing':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-indigo-100 rounded-full flex items-center justify-center">
              <Loader2 className="w-12 h-12 text-indigo-600 animate-spin" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Đang cài đặt...</h2>
            <p className="text-slate-600">
              Vui lòng chờ trong khi hệ thống cài đặt bản cập nhật.
            </p>
            <p className="text-slate-500 text-sm mt-4">
              Ứng dụng sẽ tự động khởi động lại sau khi hoàn tất.
            </p>
          </div>
        );

      case 'Restarting':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-indigo-100 rounded-full flex items-center justify-center">
              <RefreshCw className="w-12 h-12 text-indigo-600 animate-spin" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Đang khởi động lại...</h2>
            <p className="text-slate-600">
              Cập nhật hoàn tất. Ứng dụng đang khởi động lại...
            </p>
          </div>
        );

      case 'Failed':
        const canRetry = updateState.data?.can_retry !== false;
        const errorMessage = updateState.data?.error || 'Đã xảy ra lỗi không xác định';
        const failedRetryCount = updateState.data?.retry_count || 0;

        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-rose-100 rounded-full flex items-center justify-center">
              <AlertCircle className="w-12 h-12 text-rose-600" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Cập nhật thất bại</h2>
            
            {failedRetryCount > 0 && (
              <p className="text-slate-600 mb-2">
                Đã thử {failedRetryCount} lần
              </p>
            )}

            <div className="bg-rose-50 border border-rose-200 rounded-2xl p-6 mb-8 max-w-md mx-auto">
              <p className="text-rose-700 text-sm font-medium break-words">
                {errorMessage}
              </p>
            </div>

            {canRetry && (
              <button
                onClick={handleRetry}
                className="px-8 py-4 bg-indigo-600 text-white rounded-2xl font-black uppercase tracking-widest hover:bg-indigo-700 transition-all hover:scale-105 active:scale-95 flex items-center gap-3 mx-auto"
              >
                <RefreshCw className="w-5 h-5" />
                Thử lại
              </button>
            )}

            {!canRetry && (
              <p className="text-slate-500 text-sm">
                Vui lòng liên hệ giáo viên để được hỗ trợ.
              </p>
            )}
          </div>
        );

      case 'Done':
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-emerald-100 rounded-full flex items-center justify-center">
              <CheckCircle className="w-12 h-12 text-emerald-600" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Cập nhật hoàn tất!</h2>
            <p className="text-slate-600">
              Hệ thống đã được cập nhật thành công.
            </p>
          </div>
        );

      default:
        return (
          <div className="text-center">
            <div className="w-24 h-24 mx-auto mb-6 bg-slate-100 rounded-full flex items-center justify-center">
              <Loader2 className="w-12 h-12 text-slate-400 animate-spin" />
            </div>
            <h2 className="text-3xl font-black text-slate-800 mb-4">Đang kiểm tra...</h2>
            <p className="text-slate-600">
              Vui lòng chờ...
            </p>
          </div>
        );
    }
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-50 via-indigo-50 to-purple-50 flex items-center justify-center p-8">
      <div className="max-w-2xl w-full bg-white rounded-[40px] shadow-2xl p-12">
        {renderContent()}
      </div>
    </div>
  );
};

export default UpdateRequiredScreen;
