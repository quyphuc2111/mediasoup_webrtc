import { useRef, useState, useEffect, useCallback } from 'react';
import { VideoPlayer } from './VideoPlayer';

interface RemoteControlViewProps {
  studentId: string;
  studentName: string;
  studentStream: MediaStream | null;
  studentScreenSize?: { width: number; height: number } | null;
  studentIp?: string;
  studentUdpPort?: number;
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
  const isMouseDown = useRef<boolean>(false);
  const mouseDownButton = useRef<'left' | 'right' | 'middle' | null>(null);
  const lastClickTime = useRef<number>(0);
  const lastClickPos = useRef<{ x: number; y: number } | null>(null);
  const DOUBLE_CLICK_THRESHOLD = 300; // ms
  const DOUBLE_CLICK_DISTANCE = 5; // pixels
  const pressedKeys = useRef<Set<string>>(new Set());
  const pendingMouseMove = useRef<{ x: number; y: number } | null>(null);
  const mouseMoveRafId = useRef<number | null>(null);

  // Auto-focus container when mounted to enable keyboard control
  useEffect(() => {
    if (containerRef.current && isControlling) {
      containerRef.current.focus();
    }
  }, [isControlling]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (mouseMoveRafId.current !== null) {
        cancelAnimationFrame(mouseMoveRafId.current);
      }
    };
  }, []);

  // Helper function to calculate screen coordinates from mouse position
  const calculateScreenCoordinates = useCallback((clientX: number, clientY: number): { x: number; y: number } | null => {
    if (!containerRef.current) return null;
    
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
      
      // Use actual video dimensions if available
      if (video.videoWidth > 0 && video.videoHeight > 0) {
        targetWidth = video.videoWidth;
        targetHeight = video.videoHeight;
      }
    }
    
    // Calculate mouse position relative to video (not container)
    const mouseX = clientX - rect.left - videoX;
    const mouseY = clientY - rect.top - videoY;
    
    // Calculate relative position (0-1) within video bounds
    const relativeX = Math.max(0, Math.min(1, mouseX / videoWidth));
    const relativeY = Math.max(0, Math.min(1, mouseY / videoHeight));
    
    // Convert to actual screen coordinates
    const screenX = relativeX * targetWidth;
    const screenY = relativeY * targetHeight;
    
    return { x: screenX, y: screenY };
  }, [screenSize, studentStream]);

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
    
    const coords = calculateScreenCoordinates(e.clientX, e.clientY);
    if (!coords) return;
    
    // Update UI immediately for smooth cursor tracking
    setMousePos({ x: coords.x, y: coords.y });
    
    // Store pending move for throttled sending
    pendingMouseMove.current = coords;
    
    // Throttle actual network sends using requestAnimationFrame
    if (mouseMoveRafId.current === null) {
      mouseMoveRafId.current = requestAnimationFrame(() => {
        if (pendingMouseMove.current) {
          const now = Date.now();
          if (now - lastMouseMoveTime.current >= MOUSE_MOVE_THROTTLE) {
            lastMouseMoveTime.current = now;
            
            // Send move event (always send, even when not dragging, for cursor sync)
            onMouseControl({
              action: 'move',
              x: pendingMouseMove.current.x,
              y: pendingMouseMove.current.y,
            });
            
            pendingMouseMove.current = null;
          }
        }
        mouseMoveRafId.current = null;
      });
    }
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!isControlling || !containerRef.current) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    const coords = calculateScreenCoordinates(e.clientX, e.clientY);
    if (!coords) return;
    
    let button: 'left' | 'right' | 'middle' = 'left';
    if (e.button === 2) button = 'right';
    else if (e.button === 1) button = 'middle';
    
    isMouseDown.current = true;
    mouseDownButton.current = button;
    
    onMouseControl({
      action: 'mouseDown',
      x: coords.x,
      y: coords.y,
      button,
    });
  };

  const handleMouseUp = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!isControlling || !containerRef.current) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    const coords = calculateScreenCoordinates(e.clientX, e.clientY);
    if (!coords) return;
    
    let button: 'left' | 'right' | 'middle' = 'left';
    if (e.button === 2) button = 'right';
    else if (e.button === 1) button = 'middle';
    
    const wasDown = isMouseDown.current;
    const wasSameButton = mouseDownButton.current === button;
    
    isMouseDown.current = false;
    mouseDownButton.current = null;
    
    // Check for double-click
    const now = Date.now();
    const timeSinceLastClick = now - lastClickTime.current;
    const isDoubleClick = wasDown && wasSameButton && 
      timeSinceLastClick < DOUBLE_CLICK_THRESHOLD &&
      lastClickPos.current &&
      Math.abs(coords.x - lastClickPos.current.x) < DOUBLE_CLICK_DISTANCE &&
      Math.abs(coords.y - lastClickPos.current.y) < DOUBLE_CLICK_DISTANCE;
    
    if (isDoubleClick && button === 'left') {
      onMouseControl({
        action: 'doubleClick',
        x: coords.x,
        y: coords.y,
      });
      lastClickTime.current = 0; // Reset to prevent triple-click
      lastClickPos.current = null;
    } else {
      // Regular click or mouse up
      if (wasDown && wasSameButton) {
        // It was a click (down + up)
        onMouseControl({
          action: button === 'left' ? 'click' : button === 'right' ? 'rightClick' : 'middleClick',
          x: coords.x,
          y: coords.y,
        });
        lastClickTime.current = now;
        lastClickPos.current = coords;
      } else {
        // Just mouse up (for drag end)
        onMouseControl({
          action: 'mouseUp',
          x: coords.x,
          y: coords.y,
          button,
        });
      }
    }
  };

  const handleMouseWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    // Normalize scroll delta (different browsers use different units)
    const normalizeDelta = (delta: number): number => {
      // Chrome uses 100, Firefox uses 3, Safari uses different values
      // Normalize to a consistent scale
      if (Math.abs(delta) < 10) {
        return delta * 10; // Fine scroll (trackpad)
      }
      return delta; // Coarse scroll (mouse wheel)
    };
    
    onMouseControl({
      action: 'scroll',
      delta_x: normalizeDelta(e.deltaX),
      delta_y: normalizeDelta(e.deltaY),
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    // Prevent default browser behavior
    e.preventDefault();
    e.stopPropagation();
    
    // Track pressed keys
    if (!pressedKeys.current.has(e.key)) {
      pressedKeys.current.add(e.key);
      
      // Get modifier keys
      const modifiers: string[] = [];
      if (e.ctrlKey || e.metaKey) modifiers.push(e.metaKey ? 'Meta' : 'Control');
      if (e.altKey) modifiers.push('Alt');
      if (e.shiftKey) modifiers.push('Shift');
      
      // Handle special keys
      const specialKeys = ['Enter', 'Backspace', 'Tab', 'Escape', 'Delete', 
                          'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
                          'Home', 'End', 'PageUp', 'PageDown',
                          'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12',
                          'Control', 'Alt', 'Shift', 'Meta'];
      
      if (specialKeys.includes(e.key) || modifiers.length > 0) {
        onKeyboardControl({
          action: 'keyDown',
          key: e.key,
          modifiers: modifiers.length > 0 ? modifiers : undefined,
          code: e.code,
        });
      } else if (e.key.length === 1) {
        // Regular character - send as text with modifiers if any
        if (modifiers.length > 0) {
          // Send key combination
          onKeyboardControl({
            action: 'keyDown',
            key: e.key,
            modifiers,
            code: e.code,
          });
        } else {
          // Just text
          onKeyboardControl({
            action: 'text',
            text: e.key,
          });
        }
      }
    }
  };

  const handleKeyUp = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!isControlling) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    // Remove from pressed keys
    pressedKeys.current.delete(e.key);
    
    // Send keyUp for modifier keys
    const modifierKeys = ['Control', 'Alt', 'Shift', 'Meta', 'Ctrl'];
    if (modifierKeys.includes(e.key)) {
      onKeyboardControl({
        action: 'keyUp',
        key: e.key,
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
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onContextMenu={(e) => {
          e.preventDefault();
          // Right click is handled by mouseDown/mouseUp
        }}
        onWheel={handleMouseWheel}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
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
          <>
            <div style={{
              position: 'absolute',
              top: '1rem',
              left: '1rem',
              background: 'rgba(0, 0, 0, 0.7)',
              padding: '0.5rem 1rem',
              borderRadius: '4px',
              color: 'white',
              fontSize: '0.9rem',
              zIndex: 1001,
            }}>
              <div>🖱️ Chuột: ({Math.round(mousePos.x)}, {Math.round(mousePos.y)})</div>
              {screenSize && (
                <div style={{ fontSize: '0.8rem', marginTop: '0.25rem', opacity: 0.8 }}>
                  Màn hình: {screenSize.width} × {screenSize.height}
                </div>
              )}
              {isMouseDown.current && (
                <div style={{ fontSize: '0.8rem', marginTop: '0.25rem', color: '#4ade80' }}>
                  ⬇️ Đang nhấn chuột ({mouseDownButton.current})
                </div>
              )}
            </div>
            <div style={{
              position: 'absolute',
              bottom: '1rem',
              left: '1rem',
              background: 'rgba(0, 0, 0, 0.7)',
              padding: '0.5rem 1rem',
              borderRadius: '4px',
              color: 'white',
              fontSize: '0.85rem',
              zIndex: 1001,
            }}>
              <div>💡 Mẹo: Double-click để mở, kéo thả để di chuyển</div>
              <div style={{ fontSize: '0.75rem', marginTop: '0.25rem', opacity: 0.7 }}>
                Hỗ trợ: Ctrl/Cmd, Alt, Shift, Scroll, Middle click
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
