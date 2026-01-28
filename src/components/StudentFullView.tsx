import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
  data?: string | null;  // Base64 encoded (for JPEG fallback only)
  data_binary?: number[] | null;  // Binary H.264 Annex-B data (serialized as array from Rust)
  sps_pps?: number[] | null;  // AVCC format description for WebCodecs (serialized as array from Rust)
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;
}

// Remote control input types
interface MouseInputEvent {
  event_type: 'move' | 'click' | 'scroll' | 'down' | 'up';
  x: number;  // Normalized 0-1
  y: number;  // Normalized 0-1
  button?: 'left' | 'right' | 'middle';
  delta_x?: number;  // For scroll
  delta_y?: number;  // For scroll
}

interface KeyboardInputEvent {
  event_type: 'keydown' | 'keyup';
  key: string;
  code: string;
  modifiers: {
    ctrl: boolean;
    alt: boolean;
    shift: boolean;
    meta: boolean;
  };
}

interface StudentFullViewProps {
  student: StudentConnection;
  screenFrame?: ScreenFrame | null;
  onClose: () => void;
  onStopScreen: () => void;
  remoteControlEnabled?: boolean;
  onToggleRemoteControl?: () => void;
}

export function StudentFullView({
  student,
  screenFrame,
  onClose,
  onStopScreen,
  remoteControlEnabled = false,
  onToggleRemoteControl,
}: StudentFullViewProps) {
  const displayName = student.name || `Student ${student.ip.split('.').pop()}`;
  const isViewing = student.status === 'Viewing';

  // Remote control state
  const [isRemoteControlActive, setIsRemoteControlActive] = useState(remoteControlEnabled);
  const screenContainerRef = useRef<HTMLDivElement>(null);
  const keyboardInputRef = useRef<HTMLInputElement>(null);

  // Throttling for mouse move events to reduce network traffic and improve responsiveness
  const lastMouseMoveTimeRef = useRef<number>(0);
  const pendingMouseMoveRef = useRef<{ x: number; y: number } | null>(null);
  const mouseMoveTimeoutRef = useRef<number | null>(null);
  const MOUSE_MOVE_THROTTLE_MS = 16; // ~60fps for smooth movement

  // Update local state when prop changes
  useEffect(() => {
    setIsRemoteControlActive(remoteControlEnabled);
  }, [remoteControlEnabled]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (mouseMoveTimeoutRef.current !== null) {
        clearTimeout(mouseMoveTimeoutRef.current);
      }
    };
  }, []);

  // Focus keyboard input when remote control is active
  useEffect(() => {
    if (isRemoteControlActive && keyboardInputRef.current) {
      keyboardInputRef.current.focus();
    }
  }, [isRemoteControlActive]);

  // Calculate normalized mouse position
  const getNormalizedPosition = useCallback((e: React.MouseEvent): { x: number; y: number } | null => {
    const container = screenContainerRef.current;
    if (!container || !screenFrame) return null;

    const rect = container.getBoundingClientRect();

    // Calculate the actual image position within the container
    const containerAspect = rect.width / rect.height;
    const imageAspect = screenFrame.width / screenFrame.height;

    let imageWidth: number, imageHeight: number, offsetX: number, offsetY: number;

    if (containerAspect > imageAspect) {
      // Container is wider - image is constrained by height
      imageHeight = rect.height;
      imageWidth = imageHeight * imageAspect;
      offsetX = (rect.width - imageWidth) / 2;
      offsetY = 0;
    } else {
      // Container is taller - image is constrained by width
      imageWidth = rect.width;
      imageHeight = imageWidth / imageAspect;
      offsetX = 0;
      offsetY = (rect.height - imageHeight) / 2;
    }

    const localX = e.clientX - rect.left - offsetX;
    const localY = e.clientY - rect.top - offsetY;

    // Check if click is within the image bounds
    if (localX < 0 || localX > imageWidth || localY < 0 || localY > imageHeight) {
      return null;
    }

    return {
      x: localX / imageWidth,
      y: localY / imageHeight,
    };
  }, [screenFrame]);

  // Send mouse event to student
  const sendMouseEvent = useCallback(async (event: MouseInputEvent) => {
    if (!isRemoteControlActive) return;

    try {
      await invoke('send_remote_mouse_event', {
        studentId: student.id,
        event,
      });
    } catch (err) {
      console.error('Failed to send mouse event:', err);
    }
  }, [isRemoteControlActive, student.id]);

  // Send keyboard event to student
  const sendKeyboardEvent = useCallback(async (event: KeyboardInputEvent) => {
    if (!isRemoteControlActive) return;

    try {
      await invoke('send_remote_keyboard_event', {
        studentId: student.id,
        event,
      });
    } catch (err) {
      console.error('Failed to send keyboard event:', err);
    }
  }, [isRemoteControlActive, student.id]);

  // Throttled mouse move handler - only sends events at a controlled rate
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const pos = getNormalizedPosition(e);
    if (!pos) return;

    const now = Date.now();
    const timeSinceLastMove = now - lastMouseMoveTimeRef.current;

    // Store the latest position
    pendingMouseMoveRef.current = pos;

    // If enough time has passed, send immediately
    if (timeSinceLastMove >= MOUSE_MOVE_THROTTLE_MS) {
      lastMouseMoveTimeRef.current = now;
      pendingMouseMoveRef.current = null;

      // Clear any pending timeout
      if (mouseMoveTimeoutRef.current !== null) {
        clearTimeout(mouseMoveTimeoutRef.current);
        mouseMoveTimeoutRef.current = null;
      }

      sendMouseEvent({
        event_type: 'move',
        x: pos.x,
        y: pos.y,
      });
    } else {
      // Schedule a delayed send if not already scheduled
      if (mouseMoveTimeoutRef.current === null) {
        const delay = MOUSE_MOVE_THROTTLE_MS - timeSinceLastMove;
        mouseMoveTimeoutRef.current = window.setTimeout(() => {
          const pending = pendingMouseMoveRef.current;
          if (pending) {
            lastMouseMoveTimeRef.current = Date.now();
            pendingMouseMoveRef.current = null;
            mouseMoveTimeoutRef.current = null;

            sendMouseEvent({
              event_type: 'move',
              x: pending.x,
              y: pending.y,
            });
          }
        }, delay);
      }
    }
  }, [getNormalizedPosition, sendMouseEvent]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const pos = getNormalizedPosition(e);
    if (pos) {
      // Clear any pending mouse move to ensure click position is accurate
      if (mouseMoveTimeoutRef.current !== null) {
        clearTimeout(mouseMoveTimeoutRef.current);
        mouseMoveTimeoutRef.current = null;
      }
      pendingMouseMoveRef.current = null;

      const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle';
      sendMouseEvent({
        event_type: 'down',
        x: pos.x,
        y: pos.y,
        button,
      });
    }
    // Focus keyboard input for key events
    keyboardInputRef.current?.focus();
  }, [getNormalizedPosition, sendMouseEvent]);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const pos = getNormalizedPosition(e);
    if (pos) {
      // Clear any pending mouse move to ensure click position is accurate
      if (mouseMoveTimeoutRef.current !== null) {
        clearTimeout(mouseMoveTimeoutRef.current);
        mouseMoveTimeoutRef.current = null;
      }
      pendingMouseMoveRef.current = null;

      const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle';
      sendMouseEvent({
        event_type: 'up',
        x: pos.x,
        y: pos.y,
        button,
      });
    }
  }, [getNormalizedPosition, sendMouseEvent]);


  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    if (isRemoteControlActive) {
      e.preventDefault();
      // Do not send explicit click, rely on mousedown/up
    }
  }, [isRemoteControlActive]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!isRemoteControlActive) return;
    e.preventDefault();

    const pos = getNormalizedPosition(e);
    if (pos) {
      sendMouseEvent({
        event_type: 'scroll',
        x: pos.x,
        y: pos.y,
        delta_x: e.deltaX,
        delta_y: e.deltaY,
      });
    }
  }, [isRemoteControlActive, getNormalizedPosition, sendMouseEvent]);

  // Keyboard event handlers
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!isRemoteControlActive) return;
    e.preventDefault();

    sendKeyboardEvent({
      event_type: 'keydown',
      key: e.key,
      code: e.code,
      modifiers: {
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      },
    });
  }, [isRemoteControlActive, sendKeyboardEvent]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    if (!isRemoteControlActive) return;
    e.preventDefault();

    sendKeyboardEvent({
      event_type: 'keyup',
      key: e.key,
      code: e.code,
      modifiers: {
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      },
    });
  }, [isRemoteControlActive, sendKeyboardEvent]);

  // Toggle remote control
  const handleToggleRemoteControl = useCallback(() => {
    const newState = !isRemoteControlActive;
    setIsRemoteControlActive(newState);
    onToggleRemoteControl?.();

    if (newState) {
      keyboardInputRef.current?.focus();
    }
  }, [isRemoteControlActive, onToggleRemoteControl]);

  return (
    <div className={`student-full-view ${isRemoteControlActive ? 'remote-control-mode' : ''}`}>
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
          {isRemoteControlActive && (
            <div className="remote-control-indicator">
              <span className="dot"></span>
              <span>Đang điều khiển</span>
            </div>
          )}
        </div>
        <div className="header-actions">
          <button
            onClick={handleToggleRemoteControl}
            className={`btn remote-control-btn ${isRemoteControlActive ? 'active' : ''}`}
            title={isRemoteControlActive ? 'Tắt điều khiển từ xa' : 'Bật điều khiển từ xa'}
          >
            {isRemoteControlActive ? '🖱️ Đang điều khiển' : '🖱️ Điều khiển'}
          </button>
          <button onClick={onStopScreen} className="btn danger">
            ⏹️ Dừng xem
          </button>
        </div>
      </div>

      {/* Hidden input for keyboard capture */}
      <input
        ref={keyboardInputRef}
        type="text"
        className="keyboard-capture"
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        tabIndex={-1}
        aria-hidden="true"
      />

      {/* Screen View */}
      <div className="full-view-screen">
        {isViewing && screenFrame ? (
          <div
            ref={screenContainerRef}
            className={`screen-container ${isRemoteControlActive ? 'remote-control-screen' : ''}`}
            onMouseMove={isRemoteControlActive ? handleMouseMove : undefined}
            onMouseDown={isRemoteControlActive ? handleMouseDown : undefined}
            onMouseUp={isRemoteControlActive ? handleMouseUp : undefined}
            onContextMenu={handleContextMenu}
            onWheel={isRemoteControlActive ? handleWheel : undefined}
          >
            <H264VideoPlayer
              frame={screenFrame}
              className="screen-image-full"
              connectionId={student.id}
            />
            {isRemoteControlActive && (
              <div className="remote-control-overlay" />
            )}
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
