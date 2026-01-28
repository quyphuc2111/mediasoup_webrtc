import { useMemo, useState, useCallback } from 'react';
import { H264VideoPlayer } from './H264VideoPlayer';
import { ContextMenu, ContextMenuItem } from './ContextMenu';

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
  onRemoteControl?: () => void;  // Callback để mở chế độ điều khiển từ xa
}

export function StudentThumbnail({
  student,
  statusText,
  screenFrame,
  onClick,
  onConnect,
  onDisconnect,
  onRemoteControl,
}: StudentThumbnailProps) {
  const isConnected = student.status === 'Connected' || student.status === 'Viewing';
  const isConnecting = student.status === 'Connecting' || student.status === 'Authenticating';
  const hasError = typeof student.status === 'object' && 'Error' in student.status;
  const isDisconnected = student.status === 'Disconnected';
  const isViewing = student.status === 'Viewing';

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const statusClass = useMemo(() => {
    if (student.status === 'Viewing') return 'viewing';
    if (isConnected) return 'connected';
    if (isConnecting) return 'connecting';
    if (hasError) return 'error';
    return 'disconnected';
  }, [student.status, isConnected, isConnecting, hasError]);

  const displayName = student.name || `Student ${student.ip.split('.').pop()}`;

  // Handle right-click to show context menu
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  // Close context menu
  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  // Handle context menu item selection
  const handleContextMenuSelect = useCallback((id: string) => {
    switch (id) {
      case 'view':
        if (isConnected) onClick();
        break;
      case 'remote-control':
        if (onRemoteControl && isViewing) onRemoteControl();
        break;
      case 'connect':
        onConnect();
        break;
      case 'disconnect':
        onDisconnect();
        break;
      default:
        break;
    }
  }, [isConnected, isViewing, onClick, onConnect, onDisconnect, onRemoteControl]);

  // Build context menu items based on connection status
  const contextMenuItems: ContextMenuItem[] = useMemo(() => {
    const items: ContextMenuItem[] = [];

    if (isConnected) {
      items.push({
        id: 'view',
        label: 'Xem màn hình',
        icon: '👁️',
      });
    }

    if (isViewing && onRemoteControl) {
      items.push({
        id: 'remote-control',
        label: 'Điều khiển từ xa',
        icon: '🖱️',
      });
    }

    if (items.length > 0 && (isDisconnected || hasError || isConnected)) {
      items.push({ id: 'sep1', label: '', separator: true });
    }

    if (isDisconnected || hasError) {
      items.push({
        id: 'connect',
        label: 'Kết nối',
        icon: '🔗',
      });
    }

    if (isConnected) {
      items.push({
        id: 'disconnect',
        label: 'Ngắt kết nối',
        icon: '🔌',
        danger: true,
      });
    }

    return items;
  }, [isConnected, isViewing, isDisconnected, hasError, onRemoteControl]);

  return (
    <>
      <div 
        className={`student-thumbnail ${statusClass}`}
        onClick={isConnected ? onClick : undefined}
        onContextMenu={handleContextMenu}
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

      {/* Context Menu */}
      {contextMenu && contextMenuItems.length > 0 && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems}
          onSelect={handleContextMenuSelect}
          onClose={handleCloseContextMenu}
        />
      )}
    </>
  );
}

export default StudentThumbnail;
