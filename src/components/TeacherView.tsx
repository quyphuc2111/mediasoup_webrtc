import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from './VideoPlayer';

interface TeacherViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

export function TeacherView({ serverUrl, roomId, name, onDisconnect }: TeacherViewProps) {
  const {
    connectionState,
    error,
    peers,
    localStream,
    isSharing,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopScreenShare,
  } = useMediasoup();

  const handleConnect = async () => {
    await connect(serverUrl, roomId, name, true);
  };

  const handleDisconnect = () => {
    disconnect();
    onDisconnect();
  };

  const studentCount = peers.filter(p => !p.isTeacher).length;

  return (
    <div className="teacher-view">
      <div className="header">
        <h2>👨‍🏫 Giáo viên: {name}</h2>
        <div className="status">
          <span className={`connection-status ${connectionState}`}>
            {connectionState === 'connected' ? '🟢 Đã kết nối' : 
             connectionState === 'connecting' ? '🟡 Đang kết nối...' : 
             '🔴 Chưa kết nối'}
          </span>
          <span className="student-count">👥 {studentCount} học sinh</span>
        </div>
      </div>

      {error && <div className="error-message">❌ {error}</div>}

      <div className="preview-section">
        <VideoPlayer 
          stream={localStream} 
          muted={true} 
          label="Màn hình của bạn"
          className="preview-video"
        />
      </div>

      <div className="controls">
        {connectionState === 'disconnected' && (
          <button onClick={handleConnect} className="btn primary">
            🔌 Kết nối Server
          </button>
        )}

        {connectionState === 'connected' && !isSharing && (
          <>
            <button onClick={() => startScreenShare(true)} className="btn primary">
              🖥️ Chia sẻ màn hình + Âm thanh
            </button>
            <button onClick={() => startScreenShare(false)} className="btn secondary">
              🖥️ Chỉ chia sẻ màn hình
            </button>
          </>
        )}

        {connectionState === 'connected' && isSharing && (
          <>
            <button onClick={startMicrophone} className="btn secondary">
              🎤 Bật Microphone
            </button>
            <button onClick={stopScreenShare} className="btn danger">
              ⏹️ Dừng chia sẻ
            </button>
          </>
        )}

        {connectionState !== 'disconnected' && (
          <button onClick={handleDisconnect} className="btn danger">
            🚪 Ngắt kết nối
          </button>
        )}
      </div>

      <div className="room-info">
        <p><strong>Room ID:</strong> {roomId}</p>
        <p><strong>Server:</strong> {serverUrl}</p>
      </div>

      {peers.length > 0 && (
        <div className="peers-list">
          <h3>Danh sách học sinh:</h3>
          <ul>
            {peers.filter(p => !p.isTeacher).map(peer => (
              <li key={peer.id}>👤 {peer.name}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
