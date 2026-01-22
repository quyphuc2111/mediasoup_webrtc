import { useState } from 'react';
import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from './VideoPlayer';
import { RemoteControlView } from './RemoteControlView';

interface TeacherViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

type ControlAction = 'shutdown' | 'restart' | 'lock' | 'sleep' | 'logout';

const controlActions: { action: ControlAction; label: string; icon: string; description: string }[] = [
  { action: 'shutdown', label: 'Tắt máy', icon: '🔴', description: 'Tắt máy tính học sinh' },
  { action: 'restart', label: 'Khởi động lại', icon: '🔄', description: 'Khởi động lại máy tính' },
  { action: 'lock', label: 'Khóa màn hình', icon: '🔒', description: 'Khóa màn hình học sinh' },
  { action: 'sleep', label: 'Ngủ', icon: '😴', description: 'Đưa máy vào chế độ ngủ' },
  { action: 'logout', label: 'Đăng xuất', icon: '🚪', description: 'Đăng xuất tài khoản học sinh' },
];

export function TeacherView({ serverUrl, roomId, name, onDisconnect }: TeacherViewProps) {
  const [controlConfirm, setControlConfirm] = useState<{ studentId: string; studentName: string; action: ControlAction } | null>(null);
  const [remoteControlStudent, setRemoteControlStudent] = useState<{ studentId: string; studentName: string } | null>(null);
  
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
    controlStudent,
    controlMouse,
    controlKeyboard,
    requestStudentScreenShare,
    studentVideoStreams,
    studentScreenSizes,
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
                  <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                    <button
                      onClick={() => {
                        // Request student to share screen first
                        requestStudentScreenShare(peer.id);
                        setRemoteControlStudent({ studentId: peer.id, studentName: peer.name });
                      }}
                      className="btn primary small"
                      title="Điều khiển máy học sinh từ xa"
                    >
                      🖥️ Điều khiển máy
                    </button>
                    {controlActions.map(({ action, label, icon }) => (
                      <button
                        key={action}
                        onClick={() => {
                          setControlConfirm({ 
                            studentId: peer.id, 
                            studentName: peer.name,
                            action: action as ControlAction
                          });
                        }}
                        className={`btn small ${action === 'shutdown' || action === 'logout' ? 'danger' : 'secondary'}`}
                        title={controlActions.find(a => a.action === action)?.description}
                      >
                        {icon} {label}
                      </button>
                    ))}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Control Confirmation Dialog */}
      {controlConfirm && (
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
            <h3 style={{ marginTop: 0 }}>
              {controlActions.find(a => a.action === controlConfirm.action)?.icon} 
              {' '}
              Xác nhận điều khiển
            </h3>
            <p>
              Bạn có chắc muốn <strong>{controlActions.find(a => a.action === controlConfirm.action)?.label.toLowerCase()}</strong> máy của học sinh <strong>"{controlConfirm.studentName}"</strong>?
            </p>
            <div style={{ display: 'flex', gap: '1rem', justifyContent: 'flex-end', marginTop: '1.5rem' }}>
              <button
                onClick={() => setControlConfirm(null)}
                className="btn secondary"
              >
                Hủy
              </button>
              <button
                onClick={() => {
                  console.log('[TeacherView] ✅ User confirmed control command:', controlConfirm);
                  controlStudent(controlConfirm.studentId, controlConfirm.action);
                  setControlConfirm(null);
                }}
                className={`btn ${controlConfirm.action === 'shutdown' || controlConfirm.action === 'logout' ? 'danger' : 'primary'}`}
              >
                {controlActions.find(a => a.action === controlConfirm.action)?.icon} Xác nhận
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Remote Control View */}
      {remoteControlStudent && (
        <RemoteControlView
          studentId={remoteControlStudent.studentId}
          studentName={remoteControlStudent.studentName}
          studentStream={studentVideoStreams.get(remoteControlStudent.studentId) || null}
          studentScreenSize={studentScreenSizes.get(remoteControlStudent.studentId) || null}
          onMouseControl={(event) => controlMouse(remoteControlStudent.studentId, event)}
          onKeyboardControl={(event) => controlKeyboard(remoteControlStudent.studentId, event)}
          onClose={() => setRemoteControlStudent(null)}
        />
      )}
    </div>
  );
}
