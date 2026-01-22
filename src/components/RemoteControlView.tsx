import { useRef, useState, useEffect } from 'react';
import { VideoPlayer } from './VideoPlayer';

interface RemoteControlViewProps {
  studentId: string;
  studentName: string;
  studentStream: MediaStream | null;
  studentScreenSize?: { width: number; height: number } | null;
  onMouseControl: (event: any) => void;
  onKeyboardControl: (event: any) => void;
  onClose: () => void;
}

export function RemoteControlView({
  studentName,
  studentStream,
  studentScreenSize,
  onMouseControl,
  onKeyboardControl,
  onClose,
}: RemoteControlViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isControlling, setIsControlling] = useState(true); // Auto-enable control
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });
  const videoRef = useRef<HTMLVideoElement>(null);
  const [screenSize, setScreenSize] = useState<{ width: number; height: number } | null>(null);
  const lastMouseMoveTime = useRef<number>(0);
  const MOUSE_MOVE_THROTTLE = 16; // ~60fps

  // Auto-focus container when mounted to enable keyboard control
  useEffect(() => {
    if (containerRef.current && isControlling) {
      containerRef.current.focus();
    }
  }, [isControlling]);

  // Get screen size from student when available
  useEffect(() => {
    if (studentScreenSize) {
      setScreenSize(studentScreenSize);
    } else if (studentStream && !screenSize && videoRef.current) {
      // Fallback: use video dimensions
      const video = videoRef.current;
      const updateSize = () => {
        if (video.videoWidth > 0 && video.videoHeight > 0) {
          setScreenSize({ width: video.videoWidth, height: video.videoHeight });
        }
      };
      video.addEventListener('loadedmetadata', updateSize);
      updateSize();
      return () => video.removeEventListener('loadedmetadata', updateSize);
    }
  }, [studentStream, studentScreenSize, screenSize]);


  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!isControlling || !containerRef.current) return;
    
    // Throttle mouse move events for better performance
    const now = Date.now();
    if (now - lastMouseMoveTime.current < MOUSE_MOVE_THROTTLE) {
      return;
    }
    lastMouseMoveTime.current = now;
    
    const rect = containerRef.current.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    
    setMousePos({ x, y });
    
    // Calculate actual screen coordinates like RustDesk
    let targetWidth = screenSize?.width || 1920;
    let targetHeight = screenSize?.height || 1080;
    
    // Get video element if available
    let videoWidth = rect.width;
    let videoHeight = rect.height;
    let videoX = 0;
    let videoY = 0;
    
    if (videoRef.current && studentStream) {
      const video = videoRef.current;
      const videoRect = video.getBoundingClientRect();
      videoWidth = videoRect.width;
      videoHeight = videoRect.height;
      videoX = videoRect.left - rect.left;
      videoY = videoRect.top - rect.top;
      
      // Use actual video dimensions if available
      if (video.videoWidth > 0 && video.videoHeight > 0) {
        targetWidth = video.videoWidth;
        targetHeight = video.videoHeight;
      }
    }
    
    // Calculate mouse position relative to video (not container)
    const mouseX = e.clientX - rect.left - videoX;
    const mouseY = e.clientY - rect.top - videoY;
    
    // Calculate relative position (0-1) within video bounds
    const relativeX = Math.max(0, Math.min(1, mouseX / videoWidth));
    const relativeY = Math.max(0, Math.min(1, mouseY / videoHeight));
    
    // Convert to actual screen coordinates
    const screenX = relativeX * targetWidth;
    const screenY = relativeY * targetHeight;
    
    onMouseControl({
      action: 'move',
      x: screenX,
      y: screenY,
    });
  };

  const handleMouseClick = (e: React.MouseEvent<HTMLDivElement>, button: 'left' | 'right' = 'left') => {
    if (!isControlling || !containerRef.current) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    // Calculate coordinates same way as mouse move
    const rect = containerRef.current.getBoundingClientRect();
    let targetWidth = screenSize?.width || 1920;
    let targetHeight = screenSize?.height || 1080;
    
    let videoWidth = rect.width;
    let videoHeight = rect.height;
    let videoX = 0;
    let videoY = 0;
    
    if (videoRef.current && studentStream) {
      const video = videoRef.current;
      const videoRect = video.getBoundingClientRect();
      videoWidth = videoRect.width;
      videoHeight = videoRect.height;
      videoX = videoRect.left - rect.left;
      videoY = videoRect.top - rect.top;
      
      if (video.videoWidth > 0 && video.videoHeight > 0) {
        targetWidth = video.videoWidth;
        targetHeight = video.videoHeight;
      }
    }
    
    const mouseX = e.clientX - rect.left - videoX;
    const mouseY = e.clientY - rect.top - videoY;
    
    const relativeX = Math.max(0, Math.min(1, mouseX / videoWidth));
    const relativeY = Math.max(0, Math.min(1, mouseY / videoHeight));
    
    const screenX = relativeX * targetWidth;
    const screenY = relativeY * targetHeight;
    
    onMouseControl({
      action: button === 'left' ? 'click' : 'rightClick',
      x: screenX,
      y: screenY,
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
            {/* Video ref for getting dimensions - keep visible but overlay with container */}
            <video
              ref={videoRef}
              {...({ srcObject: studentStream } as any)}
              style={{ 
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: '100%',
                objectFit: 'contain',
                pointerEvents: 'none', // Let container handle events
              }}
              autoPlay
              playsInline
            />
          </div>
        ) : (
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            color: 'var(--text-secondary)',
            gap: '1rem',
          }}>
            <p>⏳ Đang chờ học sinh chia sẻ màn hình...</p>
            <p style={{ fontSize: '0.9rem' }}>
              {isControlling && 'Bạn vẫn có thể điều khiển máy học sinh ngay bây giờ'}
            </p>
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
