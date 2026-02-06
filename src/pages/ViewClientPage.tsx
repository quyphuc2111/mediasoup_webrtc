import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { StudentThumbnail } from '../components/StudentThumbnail';
import { StudentFullView } from '../components/StudentFullView';
import { KeyManager } from '../components/KeyManager';
import { DebugPanel } from '../components/DebugPanel';
import { FileManager } from '../components/FileManager';

type ConnectionStatus =
  | 'Disconnected'
  | 'Connecting'
  | 'Authenticating'
  | 'Connected'
  | 'Viewing'
  | { Error: { message: string } };

interface StudentConnection {
  id: string;
  ip: string;
  port: number;
  name: string | null;
  status: ConnectionStatus;
}

interface SavedDevice {
  id: number;
  ip: string;
  name: string;
  port: number;
  last_used: number;
}

interface ScreenFrame {
  data?: string | null;  // Base64 encoded (for JPEG fallback only)
  data_binary?: number[] | null;  // Binary H.264 Annex-B data (serialized as array from Rust)
  sps_pps?: number[] | null;  // AVCC format description for WebCodecs (serialized as array from Rust)
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;  // "h264" or "jpeg"
}

// File transfer types
type TransferStatus = 
  | 'Pending'
  | 'Connecting'
  | 'Transferring'
  | 'Completed'
  | 'Cancelled'
  | { Failed: { error: string } };

interface FileTransferProgress {
  job_id: string;
  file_name: string;
  file_size: number;
  transferred: number;
  progress: number;
  status: TransferStatus;
  student_id: string;
}

interface ViewClientPageProps {
  onBack?: () => void;
}

export function ViewClientPage({ onBack }: ViewClientPageProps) {
  const [connections, setConnections] = useState<StudentConnection[]>([]);
  const [savedDevices, setSavedDevices] = useState<SavedDevice[]>([]);
  const [screenFrames, setScreenFrames] = useState<Record<string, ScreenFrame>>({});
  const [hasKeypair, setHasKeypair] = useState(false);
  const [showKeyManager, setShowKeyManager] = useState(false);
  const [showAddStudent, setShowAddStudent] = useState(false);
  const [newStudentIp, setNewStudentIp] = useState('');
  const [newStudentPort, setNewStudentPort] = useState(3017);
  const [selectedStudent, setSelectedStudent] = useState<string | null>(null);
  const [remoteControlEnabled, setRemoteControlEnabled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [dbInitialized, setDbInitialized] = useState(false);
  const [fileTransfers, setFileTransfers] = useState<Record<string, FileTransferProgress>>({});
  const [fileManagerStudent, setFileManagerStudent] = useState<string | null>(null);

  // Initialize database and load saved devices
  useEffect(() => {
    initDatabase();
    checkKeypair();
  }, []);

  // Poll connections every second
  useEffect(() => {
    const interval = setInterval(refreshConnections, 1000);
    return () => clearInterval(interval);
  }, []);

  // Listen for real-time screen frames via Tauri events
  useEffect(() => {
    let unlistenAll: (() => void) | null = null;

    const setupListener = async () => {
      const unlisten = await listen<[string, ScreenFrame]>('screen-frame', (event) => {
        const [connId, frame] = event.payload;
        setScreenFrames(prev => ({
          ...prev,
          [connId]: frame
        }));
      });
      unlistenAll = unlisten;
    };

    setupListener();
    return () => {
      if (unlistenAll) unlistenAll();
    };
  }, []);

  // Listen for file transfer progress events
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupTransferListener = async () => {
      unlisten = await listen<FileTransferProgress>('file-transfer-progress', (event) => {
        const progress = event.payload;
        setFileTransfers(prev => ({
          ...prev,
          [progress.job_id]: progress
        }));

        // Remove completed/cancelled transfers after 5 seconds
        if (progress.status === 'Completed' || progress.status === 'Cancelled' || 
            (typeof progress.status === 'object' && 'Failed' in progress.status)) {
          setTimeout(() => {
            setFileTransfers(prev => {
              const newState = { ...prev };
              delete newState[progress.job_id];
              return newState;
            });
          }, 5000);
        }
      });
    };

    setupTransferListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const initDatabase = async () => {
    try {
      await invoke('init_db');
      setDbInitialized(true);
      loadSavedDevices();
    } catch (e) {
      console.error('Failed to initialize database:', e);
    }
  };

  const loadSavedDevices = async () => {
    try {
      const devices = await invoke<SavedDevice[]>('get_saved_devices');
      setSavedDevices(devices);
    } catch (e) {
      console.error('Failed to load saved devices:', e);
    }
  };

  const saveDevice = async (ip: string, name: string, port: number) => {
    if (!dbInitialized) return;
    try {
      await invoke('save_device_to_db', { ip, name, port });
      loadSavedDevices();
    } catch (e) {
      console.error('Failed to save device:', e);
    }
  };

  const removeDevice = async (id: number) => {
    try {
      await invoke('remove_device_from_db', { id });
      loadSavedDevices();
    } catch (e) {
      console.error('Failed to remove device:', e);
    }
  };

  const checkKeypair = async () => {
    try {
      const has = await invoke<boolean>('crypto_has_keypair');
      setHasKeypair(has);
    } catch (e) {
      console.error('Failed to check keypair:', e);
    }
  };

  const refreshConnections = async () => {
    try {
      const conns = await invoke<StudentConnection[]>('get_student_connections');
      setConnections(conns);
    } catch (e) {
      console.error('Failed to get connections:', e);
    }
  };

  const connectToStudent = async (ip: string, port: number, saveName?: string) => {
    try {
      setError(null);
      console.log(`[ViewClient] Connecting to student at ${ip}:${port}...`);
      const result = await invoke('connect_to_student', { ip, port });
      console.log(`[ViewClient] Connection initiated: ${result}`);
      setShowAddStudent(false);
      setNewStudentIp('');

      // Save the device if not already saved
      if (saveName) {
        await saveDevice(ip, saveName, port);
      }
    } catch (e) {
      console.error(`[ViewClient] Failed to connect to ${ip}:${port}:`, e);
      setError(String(e));
    }
  };

  const disconnectStudent = async (id: string) => {
    try {
      await invoke('disconnect_from_student', { connectionId: id });
    } catch (e) {
      console.error('Failed to disconnect:', e);
    }
  };

  const requestScreen = async (id: string) => {
    try {
      await invoke('request_student_screen', { connectionId: id });
    } catch (e) {
      console.error('Failed to request screen:', e);
    }
  };

  const stopScreen = async (id: string) => {
    try {
      await invoke('stop_student_screen', { connectionId: id });
    } catch (e) {
      console.error('Failed to stop screen:', e);
    }
  };

  const scanLAN = async () => {
    setIsScanning(true);
    setError(null);
    console.log('[ViewClient] Starting LAN scan on port 3017...');
    try {
      const devices = await invoke<Array<{ ip: string; name: string; port: number }>>(
        'discover_lan_devices',
        { port: 3018, timeoutMs: 3000 }
      );

      console.log(`[ViewClient] Found ${devices.length} devices:`, devices);

      // Connect to discovered devices
      for (const device of devices) {
        try {
          console.log(`[ViewClient] Connecting to discovered device: ${device.name} (${device.ip}:${device.port})`);
          await invoke('connect_to_student', { ip: device.ip, port: device.port });
          console.log(`[ViewClient] Connection initiated to ${device.ip}`);
        } catch (e) {
          console.warn(`[ViewClient] Failed to connect to ${device.ip}:`, e);
        }
      }

      if (devices.length === 0) {
        console.log('[ViewClient] No devices found on LAN');
        setError('Không tìm thấy máy học sinh nào trên mạng LAN');
      }
    } catch (e) {
      console.error('[ViewClient] LAN scan failed:', e);
      setError(String(e));
    } finally {
      setIsScanning(false);
    }
  };

  const connectAll = useCallback(async () => {
    const disconnected = connections.filter(
      c => c.status === 'Disconnected' || (typeof c.status === 'object' && 'Error' in c.status)
    );

    for (const conn of disconnected) {
      try {
        await invoke('connect_to_student', { ip: conn.ip, port: conn.port });
      } catch (e) {
        console.warn(`Failed to reconnect to ${conn.ip}:`, e);
      }
    }
  }, [connections]);

  const disconnectAll = useCallback(async () => {
    for (const conn of connections) {
      if (conn.status !== 'Disconnected') {
        try {
          await invoke('disconnect_from_student', { connectionId: conn.id });
        } catch (e) {
          console.warn(`Failed to disconnect ${conn.id}:`, e);
        }
      }
    }
  }, [connections]);

  const sendFileToStudent = useCallback(async (studentId: string) => {
    const student = connections.find(c => c.id === studentId);
    if (!student) {
      setError('Không tìm thấy học sinh');
      return;
    }

    // Open file picker dialog (allow both file and folder)
    const filePath = await open({
      multiple: false,
      directory: false,
      title: `Gửi file cho ${student.name || student.ip}`,
    });

    if (!filePath) {
      return; // User cancelled
    }

    try {
      // Get file info for display
      interface FileInfo {
        name: string;
        path: string;
        is_dir: boolean;
        size: number;
        modified: number;
      }
      
      const fileInfo = await invoke<FileInfo>('get_file_info', {
        path: filePath
      });

      console.log(`[FileTransfer] Starting chunked transfer: "${fileInfo.name}" (${fileInfo.size} bytes) to ${student.name || student.ip}`);
      
      // Start chunked TCP transfer (non-blocking)
      const jobId = await invoke<string>('send_file_to_student', {
        studentId: studentId,
        filePath: filePath,
      });
      
      console.log(`[FileTransfer] Transfer job started: ${jobId}`);
      
    } catch (err) {
      setError(`Lỗi khi gửi file: ${err}`);
      console.error('Failed to send file:', err);
    }
  }, [connections]);

  const sendFolderToStudent = useCallback(async (studentId: string) => {
    const student = connections.find(c => c.id === studentId);
    if (!student) {
      setError('Không tìm thấy học sinh');
      return;
    }

    // Open folder picker dialog
    const folderPath = await open({
      multiple: false,
      directory: true,
      title: `Gửi folder cho ${student.name || student.ip}`,
    });

    if (!folderPath) {
      return; // User cancelled
    }

    try {
      console.log(`[FileTransfer] Starting folder transfer: "${folderPath}" to ${student.name || student.ip}`);
      
      // Start chunked TCP transfer (non-blocking) - same command handles both file and folder
      const jobId = await invoke<string>('send_file_to_student', {
        studentId: studentId,
        filePath: folderPath,
      });
      
      console.log(`[FileTransfer] Folder transfer job started: ${jobId}`);
      
    } catch (err) {
      setError(`Lỗi khi gửi folder: ${err}`);
      console.error('Failed to send folder:', err);
    }
  }, [connections]);

  const openFileManager = useCallback((studentId: string) => {
    setFileManagerStudent(studentId);
  }, []);

  const cancelFileTransfer = useCallback(async (jobId: string) => {
    try {
      await invoke('cancel_file_transfer', { jobId });
    } catch (err) {
      console.error('Failed to cancel transfer:', err);
    }
  }, []);

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const getTransferStatusText = (status: TransferStatus): string => {
    if (typeof status === 'string') {
      switch (status) {
        case 'Pending': return 'Đang chờ...';
        case 'Connecting': return 'Đang kết nối...';
        case 'Transferring': return 'Đang truyền...';
        case 'Completed': return '✅ Hoàn thành';
        case 'Cancelled': return '❌ Đã hủy';
        default: return status;
      }
    }
    if ('Failed' in status) {
      return `❌ Lỗi: ${status.Failed.error}`;
    }
    return 'Unknown';
  };

  const getStatusText = (status: ConnectionStatus): string => {
    if (typeof status === 'string') {
      switch (status) {
        case 'Disconnected': return 'Ngắt kết nối';
        case 'Connecting': return 'Đang kết nối...';
        case 'Authenticating': return 'Đang xác thực...';
        case 'Connected': return 'Đã kết nối';
        case 'Viewing': return 'Đang xem';
        default: return status;
      }
    }
    if ('Error' in status) {
      return `Lỗi: ${status.Error.message}`;
    }
    return 'Unknown';
  };

  const connectedCount = connections.filter(
    c => c.status === 'Connected' || c.status === 'Viewing'
  ).length;

  // If viewing a student fullscreen
  if (selectedStudent) {
    const student = connections.find(c => c.id === selectedStudent);
    if (student) {
      return (
        <StudentFullView
          student={student}
          screenFrame={screenFrames[student.id]}
          onClose={() => {
            setSelectedStudent(null);
            setRemoteControlEnabled(false);
          }}
          onStopScreen={() => stopScreen(student.id)}
          remoteControlEnabled={remoteControlEnabled}
          onToggleRemoteControl={() => setRemoteControlEnabled(!remoteControlEnabled)}
        />
      );
    }
  }

  return (
    <div className="view-client-page">
      {onBack && (
        <button onClick={onBack} className="btn back-btn">
          ← Quay lại
        </button>
      )}

      <div className="page-header">
        <h1>👁️ View Client</h1>
        <p className="subtitle">Xem màn hình máy học sinh</p>
      </div>

      {/* Key Manager */}
      {!hasKeypair && (
        <div className="warning-box">
          <h3>⚠️ Chưa có cặp khóa</h3>
          <p>Bạn cần tạo cặp khóa trước khi có thể kết nối với máy học sinh.</p>
          <button onClick={() => setShowKeyManager(true)} className="btn primary">
            🔑 Tạo cặp khóa
          </button>
        </div>
      )}

      {hasKeypair && (
        <div className="key-info">
          <span>🔑 Đã có cặp khóa</span>
          <button onClick={() => setShowKeyManager(true)} className="btn secondary small">
            Quản lý khóa
          </button>
        </div>
      )}

      {showKeyManager && (
        <KeyManager
          onClose={() => {
            setShowKeyManager(false);
            checkKeypair();
          }}
        />
      )}

      {/* Controls */}
      <div className="controls-bar">
        <button
          onClick={scanLAN}
          className="btn primary"
          disabled={!hasKeypair || isScanning}
        >
          {isScanning ? '🔍 Đang quét...' : '🔍 Quét LAN'}
        </button>

        <button
          onClick={() => setShowAddStudent(true)}
          className="btn secondary"
          disabled={!hasKeypair}
        >
          ➕ Thêm máy
        </button>

        <button
          onClick={connectAll}
          className="btn secondary"
          disabled={!hasKeypair || connections.length === 0}
        >
          🔗 Kết nối tất cả
        </button>

        <button
          onClick={disconnectAll}
          className="btn secondary"
          disabled={connectedCount === 0}
        >
          🔌 Ngắt tất cả
        </button>

        <span className="connection-count">
          {connectedCount} / {connections.length} đã kết nối
        </span>
      </div>

      {/* Add Student Modal */}
      {showAddStudent && (
        <div className="modal-overlay">
          <div className="modal">
            <h3>Thêm máy học sinh</h3>
            <div className="form-group">
              <label>IP Address:</label>
              <input
                type="text"
                value={newStudentIp}
                onChange={(e) => setNewStudentIp(e.target.value)}
                placeholder="192.168.1.xxx"
              />
            </div>
            <div className="form-group">
              <label>Port:</label>
              <input
                type="number"
                value={newStudentPort}
                onChange={(e) => setNewStudentPort(parseInt(e.target.value) || 3017)}
              />
            </div>
            <div className="modal-actions">
              <button onClick={() => setShowAddStudent(false)} className="btn secondary">
                Hủy
              </button>
              <button
                onClick={() => connectToStudent(newStudentIp, newStudentPort)}
                className="btn primary"
                disabled={!newStudentIp.trim()}
              >
                Kết nối
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div className="error-box">
          <p>{error}</p>
          <button onClick={() => setError(null)} className="btn small">✕</button>
        </div>
      )}

      {/* Student Grid */}
      <div className="student-grid">
        {connections.length === 0 && savedDevices.length === 0 ? (
          <div className="empty-state">
            <p>Chưa có máy học sinh nào</p>
            <p className="hint">Nhấn "Quét LAN" hoặc "Thêm máy" để bắt đầu</p>
          </div>
        ) : (
          <>
            {/* Active connections */}
            {connections.map((conn) => (
              <StudentThumbnail
                key={conn.id}
                student={conn}
                statusText={getStatusText(conn.status)}
                screenFrame={screenFrames[conn.id]}
                onClick={() => {
                  if (conn.status === 'Connected' || conn.status === 'Viewing') {
                    setSelectedStudent(conn.id);
                    if (conn.status === 'Connected') {
                      requestScreen(conn.id);
                    }
                  }
                }}
                onConnect={() => connectToStudent(conn.ip, conn.port, conn.name || undefined)}
                onDisconnect={() => disconnectStudent(conn.id)}
                onRemoteControl={() => {
                  // Mở chế độ điều khiển từ xa
                  setSelectedStudent(conn.id);
                  setRemoteControlEnabled(true);
                  if (conn.status === 'Connected') {
                    requestScreen(conn.id);
                  }
                }}
                onSendFile={() => sendFileToStudent(conn.id)}
                onSendFolder={() => sendFolderToStudent(conn.id)}
                onOpenFileManager={() => openFileManager(conn.id)}
              />
            ))}

            {/* Saved devices that are not currently connected */}
            {savedDevices
              .filter(device => !connections.some(c => c.ip === device.ip && c.port === device.port))
              .map((device) => (
                <div key={`saved-${device.id}`} className="student-thumbnail saved">
                  <div className="thumbnail-screen">
                    <div className="screen-placeholder offline">
                      <span>💾</span>
                      <p>Đã lưu</p>
                    </div>
                  </div>
                  <div className="thumbnail-info">
                    <div className="student-name">{device.name}</div>
                    <div className="student-ip">{device.ip}:{device.port}</div>
                    <div className="student-status disconnected">Chưa kết nối</div>
                  </div>
                  <div className="thumbnail-actions">
                    <button
                      onClick={() => connectToStudent(device.ip, device.port)}
                      className="btn small primary"
                    >
                      🔗 Kết nối
                    </button>
                    <button
                      onClick={() => removeDevice(device.id)}
                      className="btn small danger"
                    >
                      🗑️
                    </button>
                  </div>
                </div>
              ))}
          </>
        )}
      </div>

      {/* Saved devices count */}
      {savedDevices.length > 0 && (
        <div className="saved-devices-summary">
          <p>💾 {savedDevices.length} máy đã lưu</p>
        </div>
      )}

      {/* File Transfer Progress */}
      {Object.keys(fileTransfers).length > 0 && (
        <div className="file-transfer-panel">
          <h3>📤 Đang truyền file</h3>
          {Object.values(fileTransfers).map((transfer) => {
            const student = connections.find(c => c.id === transfer.student_id);
            const isActive = transfer.status === 'Transferring' || transfer.status === 'Connecting';
            
            return (
              <div key={transfer.job_id} className={`transfer-item ${typeof transfer.status === 'string' ? transfer.status.toLowerCase() : 'failed'}`}>
                <div className="transfer-info">
                  <span className="file-name">{transfer.file_name}</span>
                  <span className="transfer-target">→ {student?.name || student?.ip || 'Unknown'}</span>
                </div>
                <div className="transfer-progress">
                  <div className="progress-bar">
                    <div 
                      className="progress-fill" 
                      style={{ width: `${transfer.progress}%` }}
                    />
                  </div>
                  <span className="progress-text">
                    {formatFileSize(transfer.transferred)} / {formatFileSize(transfer.file_size)} ({transfer.progress.toFixed(1)}%)
                  </span>
                </div>
                <div className="transfer-status">
                  <span>{getTransferStatusText(transfer.status)}</span>
                  {isActive && (
                    <button 
                      onClick={() => cancelFileTransfer(transfer.job_id)}
                      className="btn small danger"
                    >
                      ✕ Hủy
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Debug Panel */}
      <DebugPanel />

      {/* File Manager Modal */}
      {fileManagerStudent && (() => {
        const student = connections.find(c => c.id === fileManagerStudent);
        if (!student) return null;
        return (
          <FileManager
            student={student}
            onClose={() => setFileManagerStudent(null)}
          />
        );
      })()}
    </div>
  );
}

export default ViewClientPage;
