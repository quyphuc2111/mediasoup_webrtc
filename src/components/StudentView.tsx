import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from './VideoPlayer';

interface StudentViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

export function StudentView({ serverUrl, roomId, name, onDisconnect }: StudentViewProps) {
  const {
    connectionState,
    error,
    remoteStream,
    connect,
    disconnect,
  } = useMediasoup();

  const handleConnect = async () => {
    await connect(serverUrl, roomId, name, false);
  };

  const handleDisconnect = () => {
    disconnect();
    onDisconnect();
  };

  return (
    <div className="student-view">
      <div className="header">
        <h2>👨‍🎓 Học sinh: {name}</h2>
        <div className="status">
          <span className={`connection-status ${connectionState}`}>
            {connectionState === 'connected' ? '🟢 Đã kết nối' : 
             connectionState === 'connecting' ? '🟡 Đang kết nối...' : 
             '🔴 Chưa kết nối'}
          </span>
        </div>
      </div>

      {error && <div className="error-message">❌ {error}</div>}

      <div className="video-section">
        <VideoPlayer 
          stream={remoteStream} 
          muted={false}
          label="Màn hình giáo viên"
          className="main-video"
        />
      </div>

      <div className="controls">
        {connectionState === 'disconnected' && (
          <button onClick={handleConnect} className="btn primary">
            🔌 Kết nối vào lớp
          </button>
        )}

        {connectionState !== 'disconnected' && (
          <button onClick={handleDisconnect} className="btn danger">
            🚪 Rời lớp
          </button>
        )}
      </div>

      <div className="room-info">
        <p><strong>Room ID:</strong> {roomId}</p>
      </div>

      {connectionState === 'connected' && !remoteStream && (
        <div className="waiting-message">
          ⏳ Đang chờ giáo viên chia sẻ màn hình...
        </div>
      )}
    </div>
  );
}
