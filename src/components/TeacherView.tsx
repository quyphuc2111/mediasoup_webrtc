import { useState } from 'react';
import { useMediasoup } from '../hooks/useMediasoup';
import { useUdpAudio } from '../hooks/useUdpAudio';
import { VideoPlayer } from './VideoPlayer';
import { LanDiscovery } from './LanDiscovery';
import { DebugPanel } from './DebugPanel';

interface TeacherViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

export function TeacherView({ serverUrl, roomId, name, onDisconnect }: TeacherViewProps) {
  const [audioMode, setAudioMode] = useState<'webrtc' | 'udp'>('webrtc');
  const [showLanDiscovery, setShowLanDiscovery] = useState(false);

  const {
    connectionState,
    error,
    peers,
    localStream,
    isSharing,
    isMicActive,
    studentAudioStream,
    isScreenAudioEnabled,
    hasScreenAudio,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopMicrophone,
    stopScreenShare,
    toggleScreenAudio,
  } = useMediasoup();

  const {
    isServerRunning,
    startUdpAudioServer,
    stopUdpAudioServer,
    serverPort,
    error: udpError,
  } = useUdpAudio();

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

      {(error || udpError) && (
        <div className={`error-message ${(error || udpError)?.startsWith('⚠️') ? 'warning-message' : ''}`}>
          {(error || udpError)?.split('\n').map((line, i) => (
            <div key={i}>{line || '\u00A0'}</div>
          ))}
        </div>
      )}

      <div className="audio-mode-selector">
        <label>Chế độ Audio:</label>
        <select
          value={audioMode}
          onChange={async (e) => {
            setAudioMode(e.target.value as 'webrtc' | 'udp');
            if (e.target.value === 'udp' && !isServerRunning) {
              // Broadcast to all devices in LAN (RustDesk approach)
              await startUdpAudioServer(5000, '255.255.255.255');
            } else if (e.target.value === 'webrtc' && isServerRunning) {
              await stopUdpAudioServer();
            }
          }}
          className="mode-select"
        >
          <option value="webrtc">WebRTC (Mặc định)</option>
          <option value="udp">UDP Streaming</option>
        </select>
        {audioMode === 'udp' && (
          <button
            onClick={() => setShowLanDiscovery(!showLanDiscovery)}
            className="btn secondary"
          >
            {showLanDiscovery ? 'Ẩn' : 'Hiện'} LAN Discovery
          </button>
        )}
      </div>

      {audioMode === 'udp' && showLanDiscovery && (
        <div className="lan-discovery-section">
          <LanDiscovery />
        </div>
      )}

      {audioMode === 'udp' && isServerRunning && (
        <div className="udp-audio-controls">
          <p>✅ UDP Audio đang chạy (RustDesk approach)</p>
          <p className="info-text">
            🎤 Audio được capture tự động trong Rust và gửi qua UDP broadcast (255.255.255.255:{serverPort})
          </p>
          <button
            onClick={stopUdpAudioServer}
            className="btn danger"
          >
            ⏹️ Dừng UDP Audio
          </button>
        </div>
      )}

      <div className="preview-section">
        <VideoPlayer 
          stream={localStream} 
          muted={true} 
          label="Màn hình của bạn"
          className="preview-video"
        />
      </div>

      {studentAudioStream && (
        <div className="student-audio-section">
          <VideoPlayer 
            stream={studentAudioStream} 
            muted={false} 
            label="Âm thanh học sinh"
            className="student-audio"
          />
        </div>
      )}

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
            {hasScreenAudio && (
              <button 
                onClick={toggleScreenAudio} 
                className={`btn ${isScreenAudioEnabled ? 'secondary' : 'secondary'}`}
                title={isScreenAudioEnabled ? 'Tắt âm thanh màn hình' : 'Bật âm thanh màn hình'}
              >
                {isScreenAudioEnabled ? '🔊 Âm thanh màn hình: Bật' : '🔇 Âm thanh màn hình: Tắt'}
              </button>
            )}
            {!isMicActive ? (
              <button onClick={startMicrophone} className="btn secondary">
                🎤 Bật Microphone
              </button>
            ) : (
              <button onClick={stopMicrophone} className="btn secondary">
                🎤 Tắt Microphone
              </button>
            )}
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

      <DebugPanel />
    </div>
  );
}
