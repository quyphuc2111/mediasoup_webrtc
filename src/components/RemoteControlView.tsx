import { useRef, useState } from 'react';
import { VideoPlayer } from './VideoPlayer';

interface RemoteControlViewProps {
  studentId: string;
  studentName: string;
  studentStream: MediaStream | null;
  onMouseControl: (event: any) => void;
  onKeyboardControl: (event: any) => void;
  onClose: () => void;
}

export function RemoteControlView({
  studentName,
  studentStream,
  onMouseControl,
  onKeyboardControl,
  onClose,
}: RemoteControlViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isControlling, setIsControlling] = useState(false);
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!isControlling || !containerRef.current) return;
    
    const rect = containerRef.current.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    
    setMousePos({ x, y });
    
    // Get actual screen coordinates (need to know student's screen size)
    // For now, send relative coordinates
    onMouseControl({
      action: 'move',
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    });
  };

  const handleMouseClick = (e: React.MouseEvent<HTMLDivElement>, button: 'left' | 'right' = 'left') => {
    if (!isControlling) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    onMouseControl({
      action: button === 'left' ? 'click' : 'rightClick',
      x: mousePos.x,
      y: mousePos.y,
    });
  };

  const handleMouseWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    e.preventDefault();
    onMouseControl({
      action: 'scroll',
      delta_x: e.deltaX,
      delta_y: e.deltaY,
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    e.preventDefault();
    onKeyboardControl({
      action: 'key',
      key: e.key,
    });
  };

  const handleKeyPress = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    e.preventDefault();
    if (e.key.length === 1) {
      onKeyboardControl({
        action: 'text',
        text: e.key,
      });
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: '#000',
        zIndex: 1000,
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div style={{
        padding: '1rem',
        background: 'var(--bg-secondary)',
        borderBottom: '1px solid var(--border)',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}>
        <div>
          <h3 style={{ margin: 0 }}>🖥️ Điều khiển máy học sinh: {studentName}</h3>
          <p style={{ margin: '0.5rem 0 0 0', fontSize: '0.9rem', color: 'var(--text-secondary)' }}>
            {isControlling ? '🟢 Đang điều khiển' : '🔴 Chưa bật điều khiển'}
          </p>
        </div>
        <div style={{ display: 'flex', gap: '1rem' }}>
          <button
            onClick={() => setIsControlling(!isControlling)}
            className={`btn ${isControlling ? 'danger' : 'primary'}`}
          >
            {isControlling ? '⏸️ Tạm dừng' : '▶️ Bắt đầu điều khiển'}
          </button>
          <button onClick={onClose} className="btn danger">
            ✕ Đóng
          </button>
        </div>
      </div>

      <div
        ref={containerRef}
        style={{
          flex: 1,
          position: 'relative',
          overflow: 'hidden',
          cursor: isControlling ? 'crosshair' : 'default',
        }}
        onMouseMove={handleMouseMove}
        onClick={(e) => handleMouseClick(e, 'left')}
        onContextMenu={(e) => {
          e.preventDefault();
          handleMouseClick(e, 'right');
        }}
        onWheel={handleMouseWheel}
        onKeyDown={handleKeyDown}
        onKeyPress={handleKeyPress}
        tabIndex={0}
      >
        {studentStream ? (
          <div style={{ width: '100%', height: '100%' }}>
            <VideoPlayer
              stream={studentStream}
              muted={false}
              className="remote-control-video"
            />
          </div>
        ) : (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            color: 'var(--text-secondary)',
          }}>
            <p>Học sinh chưa chia sẻ màn hình</p>
          </div>
        )}
        
        {isControlling && (
          <div style={{
            position: 'absolute',
            top: '1rem',
            left: '1rem',
            background: 'rgba(0, 0, 0, 0.7)',
            padding: '0.5rem 1rem',
            borderRadius: '4px',
            color: 'white',
            fontSize: '0.9rem',
          }}>
            Chuột: ({Math.round(mousePos.x)}, {Math.round(mousePos.y)})
          </div>
        )}
      </div>
    </div>
  );
}
