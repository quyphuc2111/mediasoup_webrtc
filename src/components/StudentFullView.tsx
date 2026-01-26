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
  data: string;  // Base64 encoded frame data
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;
}

interface StudentFullViewProps {
  student: StudentConnection;
  screenFrame?: ScreenFrame | null;
  onClose: () => void;
  onStopScreen: () => void;
}

export function StudentFullView({ student, screenFrame, onClose, onStopScreen }: StudentFullViewProps) {
  const displayName = student.name || `Student ${student.ip.split('.').pop()}`;
  const isViewing = student.status === 'Viewing';

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
          {screenFrame && (
            <span className="screen-resolution">{screenFrame.width}x{screenFrame.height}</span>
          )}
        </div>
        <div className="header-actions">
          <button onClick={onStopScreen} className="btn danger">
            ⏹️ Dừng xem
          </button>
        </div>
      </div>

      {/* Screen View */}
      <div className="full-view-screen">
        {isViewing && screenFrame ? (
          <div className="screen-container">
            <H264VideoPlayer 
              frame={screenFrame}
              className="screen-image-full"
            />
          </div>
        ) : isViewing ? (
          <div className="screen-container">
            <div className="screen-placeholder full">
              <span className="spinner">🔄</span>
              <p>Đang tải màn hình từ {displayName}...</p>
            </div>
          </div>
        ) : (
          <div className="screen-container">
            <div className="screen-placeholder full">
              <span>⏳</span>
              <p>Đang kết nối...</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default StudentFullView;
