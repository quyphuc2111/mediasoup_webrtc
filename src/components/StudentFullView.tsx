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

interface StudentFullViewProps {
  student: StudentConnection;
  onClose: () => void;
  onStopScreen: () => void;
}

export function StudentFullView({ student, onClose, onStopScreen }: StudentFullViewProps) {
  const displayName = student.name || `Student ${student.ip.split('.').pop()}`;

  return (
    <div className="student-full-view">
      {/* Header */}
      <div className="full-view-header">
        <button onClick={onClose} className="btn back-btn">
          ← Quay lại Grid
        </button>
        <div className="student-info">
          <h2>{displayName}</h2>
          <span className="student-ip">{student.ip}:{student.port}</span>
        </div>
        <div className="header-actions">
          <button onClick={onStopScreen} className="btn danger">
            ⏹️ Dừng xem
          </button>
        </div>
      </div>

      {/* Screen View */}
      <div className="full-view-screen">
        {student.status === 'Viewing' ? (
          <div className="screen-container">
            {/* TODO: Add actual WebRTC video stream here */}
            <div className="screen-placeholder full">
              <span>🖥️</span>
              <p>Màn hình {displayName}</p>
              <p className="hint">WebRTC stream sẽ hiển thị ở đây</p>
            </div>
          </div>
        ) : (
          <div className="screen-container">
            <div className="screen-placeholder full">
              <span>⏳</span>
              <p>Đang tải màn hình...</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default StudentFullView;
