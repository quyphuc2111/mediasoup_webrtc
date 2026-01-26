import { useMemo } from 'react';
import { H264VideoPlayer } from './H264VideoPlayer';

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

interface ScreenFrame {
  data?: string | null;  // Base64 encoded (for JPEG fallback only)
  data_binary?: number[] | null;  // Binary H.264 Annex-B data (serialized as array from Rust)
  sps_pps?: number[] | null;  // AVCC format description for WebCodecs (serialized as array from Rust)
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;
}

interface StudentThumbnailProps {
  student: StudentConnection;
  statusText: string;
  screenFrame?: ScreenFrame | null;
  onClick: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
}

export function StudentThumbnail({
  student,
  statusText,
  screenFrame,
  onClick,
  onConnect,
  onDisconnect,
}: StudentThumbnailProps) {
  const isConnected = student.status === 'Connected' || student.status === 'Viewing';
  const isConnecting = student.status === 'Connecting' || student.status === 'Authenticating';
  const hasError = typeof student.status === 'object' && 'Error' in student.status;
  const isDisconnected = student.status === 'Disconnected';
  const isViewing = student.status === 'Viewing';

  const statusClass = useMemo(() => {
    if (student.status === 'Viewing') return 'viewing';
    if (isConnected) return 'connected';
    if (isConnecting) return 'connecting';
    if (hasError) return 'error';
    return 'disconnected';
  }, [student.status, isConnected, isConnecting, hasError]);

  const displayName = student.name || `Student ${student.ip.split('.').pop()}`;

  return (
    <div 
      className={`student-thumbnail ${statusClass}`}
      onClick={isConnected ? onClick : undefined}
    >
      {/* Screen Preview Area */}
      <div className="thumbnail-screen">
        {isViewing && screenFrame ? (
          <div className="screen-preview">
            <H264VideoPlayer 
              frame={screenFrame}
              className="screen-image"
            />
          </div>
        ) : isViewing ? (
          <div className="screen-preview">
            <div className="preview-placeholder">
              <span className="spinner">🔄</span>
              <p>Đang tải màn hình...</p>
            </div>
          </div>
        ) : isConnected ? (
          <div className="screen-placeholder connected">
            <span>✅</span>
            <p>Click để xem</p>
          </div>
        ) : isConnecting ? (
          <div className="screen-placeholder connecting">
            <span className="spinner">⏳</span>
            <p>Đang kết nối...</p>
          </div>
        ) : hasError ? (
          <div className="screen-placeholder error">
            <span>❌</span>
            <p>Lỗi kết nối</p>
          </div>
        ) : (
          <div className="screen-placeholder offline">
            <span>💤</span>
            <p>Offline</p>
          </div>
        )}
      </div>

      {/* Info Bar */}
      <div className="thumbnail-info">
        <div className="student-name">{displayName}</div>
        <div className="student-ip">{student.ip}:{student.port}</div>
        <div className={`student-status ${statusClass}`}>{statusText}</div>
      </div>

      {/* Actions */}
      <div className="thumbnail-actions">
        {isDisconnected || hasError ? (
          <button onClick={(e) => { e.stopPropagation(); onConnect(); }} className="btn small primary">
            🔗 Kết nối
          </button>
        ) : isConnected ? (
          <button onClick={(e) => { e.stopPropagation(); onDisconnect(); }} className="btn small danger">
            🔌 Ngắt
          </button>
        ) : null}
      </div>
    </div>
  );
}

export default StudentThumbnail;
