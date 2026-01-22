import { useState } from 'react';
import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from './VideoPlayer';

interface TeacherViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

export function TeacherView({ serverUrl, roomId, name, onDisconnect }: TeacherViewProps) {
  const [shutdownConfirm, setShutdownConfirm] = useState<{ studentId: string; studentName: string } | null>(null);
  const {
    connectionState,
    error,
    peers,
    localStream,
    isSharing,
    isMicActive,
    studentAudioStream,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopMicrophone,
    stopScreenShare,
    shutdownStudent,
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

      {error && (
        <div className={`error-message ${error.startsWith('⚠️') ? 'warning-message' : ''}`}>
          {error.split('\n').map((line, i) => (
            <div key={i}>{line || '\u00A0'}</div>
          ))}
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
              <li key={peer.id}>
                <span>👤 {peer.name}</span>
                {connectionState === 'connected' && (
                  <button
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      console.log('[TeacherView] Button clicked for student:', peer.id, peer.name);
                      
                      // For testing: hold Shift key to skip confirmation
                      const skipConfirm = e.shiftKey || e.metaKey || e.ctrlKey;
                      
                      if (skipConfirm) {
                        console.log('[TeacherView] ✅ Skipping confirmation (key held), sending shutdown command');
                        if (shutdownStudent) {
                          shutdownStudent(peer.id);
                        }
                      } else {
                        // Show confirmation dialog
                        setShutdownConfirm({ studentId: peer.id, studentName: peer.name });
                      }
                    }}
                    className="btn danger small"
                    title="Tắt máy học sinh (Giữ Shift/Cmd/Ctrl để bỏ qua xác nhận)"
                  >
                    🔴 Tắt máy
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      
      {/* Shutdown Confirmation Dialog */}
      {shutdownConfirm && (
        <div style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'rgba(0, 0, 0, 0.7)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 1000
        }}>
          <div style={{
            background: 'var(--bg-secondary)',
            padding: '2rem',
            borderRadius: '12px',
            border: '1px solid var(--border)',
            maxWidth: '400px',
            width: '90%'
          }}>
            <h3 style={{ marginTop: 0 }}>⚠️ Xác nhận tắt máy</h3>
            <p>Bạn có chắc muốn tắt máy của học sinh <strong>"{shutdownConfirm.studentName}"</strong>?</p>
            <div style={{ display: 'flex', gap: '1rem', justifyContent: 'flex-end', marginTop: '1.5rem' }}>
              <button
                onClick={() => {
                  console.log('[TeacherView] User cancelled shutdown');
                  setShutdownConfirm(null);
                }}
                className="btn secondary"
              >
                Hủy
              </button>
              <button
                onClick={() => {
                  console.log('[TeacherView] ✅ User confirmed, sending shutdown command to student:', shutdownConfirm.studentId, shutdownConfirm.studentName);
                  if (shutdownStudent) {
                    try {
                      shutdownStudent(shutdownConfirm.studentId);
                      console.log('[TeacherView] ✅ shutdownStudent called successfully');
                    } catch (error) {
                      console.error('[TeacherView] ❌ Error calling shutdownStudent:', error);
                    }
                  } else {
                    console.error('[TeacherView] ❌ shutdownStudent is undefined!');
                  }
                  setShutdownConfirm(null);
                }}
                className="btn danger"
              >
                🔴 Xác nhận tắt máy
              </button>
            </div>
          </div>
        </div>
      )}
      
      {/* Debug info */}
      <div style={{ marginTop: '1rem', padding: '0.5rem', background: '#1a1a1a', borderRadius: '8px', fontSize: '0.8rem' }}>
        <strong>Debug Info:</strong>
        <div>Peers count: {peers.length}</div>
        <div>Students: {peers.filter(p => !p.isTeacher).length}</div>
        <div>Connection state: {connectionState}</div>
        <div>shutdownStudent type: {typeof shutdownStudent}</div>
      </div>
    </div>
  );
}
