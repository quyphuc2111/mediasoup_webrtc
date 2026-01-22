import { useEffect, useRef } from 'react';
import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from './VideoPlayer';

interface StudentViewProps {
  serverUrl: string;
  roomId: string;
  name: string;
  onDisconnect: () => void;
}

export function StudentView({ serverUrl, roomId, name, onDisconnect }: StudentViewProps) {
  const {
    connectionState,
    error,
    remoteStream,
    isPushToTalkActive,
    connect,
    disconnect,
    enablePushToTalk,
    disablePushToTalk,
  } = useMediasoup();

  const handleConnect = async () => {
    await connect(serverUrl, roomId, name, false);
  };

  const handleDisconnect = () => {
    disconnect();
    onDisconnect();
  };

  const isViewingStream = connectionState === 'connected' && remoteStream !== null;
  const videoSectionRef = useRef<HTMLDivElement>(null);

  // Auto fullscreen when stream starts
  useEffect(() => {
    if (isViewingStream && videoSectionRef.current) {
      const element = videoSectionRef.current;
      
      const requestFullscreen = async () => {
        try {
          if (element.requestFullscreen) {
            await element.requestFullscreen();
          } else if ((element as any).webkitRequestFullscreen) {
            // Safari
            await (element as any).webkitRequestFullscreen();
          } else if ((element as any).mozRequestFullScreen) {
            // Firefox
            await (element as any).mozRequestFullScreen();
          } else if ((element as any).msRequestFullscreen) {
            // IE/Edge
            await (element as any).msRequestFullscreen();
          }
          console.log('[StudentView] ✅ Entered fullscreen mode');
        } catch (err) {
          console.error('[StudentView] ❌ Failed to enter fullscreen:', err);
        }
      };

      requestFullscreen();
    } else {
      // Exit fullscreen when stream ends
      const exitFullscreen = () => {
        try {
          if (document.exitFullscreen) {
            document.exitFullscreen();
          } else if ((document as any).webkitExitFullscreen) {
            (document as any).webkitExitFullscreen();
          } else if ((document as any).mozCancelFullScreen) {
            (document as any).mozCancelFullScreen();
          } else if ((document as any).msExitFullscreen) {
            (document as any).msExitFullscreen();
          }
          console.log('[StudentView] ✅ Exited fullscreen mode');
        } catch (err) {
          console.error('[StudentView] ❌ Failed to exit fullscreen:', err);
        }
      };

      // Only exit if we're actually in fullscreen
      if (
        document.fullscreenElement ||
        (document as any).webkitFullscreenElement ||
        (document as any).mozFullScreenElement ||
        (document as any).msFullscreenElement
      ) {
        exitFullscreen();
      }
    }
  }, [isViewingStream]);

  // Handle fullscreen change events
  useEffect(() => {
    const handleFullscreenChange = () => {
      const isFullscreen = !!(
        document.fullscreenElement ||
        (document as any).webkitFullscreenElement ||
        (document as any).mozFullScreenElement ||
        (document as any).msFullscreenElement
      );
      console.log('[StudentView] Fullscreen state changed:', isFullscreen);
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    document.addEventListener('webkitfullscreenchange', handleFullscreenChange);
    document.addEventListener('mozfullscreenchange', handleFullscreenChange);
    document.addEventListener('MSFullscreenChange', handleFullscreenChange);

    return () => {
      document.removeEventListener('fullscreenchange', handleFullscreenChange);
      document.removeEventListener('webkitfullscreenchange', handleFullscreenChange);
      document.removeEventListener('mozfullscreenchange', handleFullscreenChange);
      document.removeEventListener('MSFullscreenChange', handleFullscreenChange);
    };
  }, []);

  return (
    <div className={`student-view ${isViewingStream ? 'viewing-stream' : ''}`}>
      {!isViewingStream && (
        <>
          <div className="header">
            <h2>👨‍🎓 Học sinh: {name}</h2>
            <div className="status">
              <span className={`connection-status ${connectionState}`}>
                {connectionState === 'connected' ? '🟢 Đã kết nối' : 
                 connectionState === 'connecting' ? '🟡 Đang kết nối...' : 
                 '🔴 Chưa kết nối'}
              </span>
            </div>
          </div>

          {error && <div className="error-message">❌ {error}</div>}
        </>
      )}

      <div ref={videoSectionRef} className="video-section">
        <VideoPlayer 
          stream={remoteStream} 
          muted={false}
          label={isViewingStream ? undefined : "Màn hình giáo viên"}
          className="main-video"
          disableInteraction={isViewingStream}
        />
        {isViewingStream && (
          <div className="stream-overlay">
            <div className="stream-controls-overlay">
              <button
                onMouseDown={enablePushToTalk}
                onMouseUp={disablePushToTalk}
                onMouseLeave={disablePushToTalk}
                onTouchStart={(e) => {
                  e.preventDefault();
                  enablePushToTalk();
                }}
                onTouchEnd={(e) => {
                  e.preventDefault();
                  disablePushToTalk();
                }}
                className={`btn push-to-talk-overlay ${isPushToTalkActive ? 'active' : ''}`}
              >
                {isPushToTalkActive ? '🎤 Đang nói...' : '🎤 Nhấn để nói'}
              </button>
              <button onClick={handleDisconnect} className="btn danger-overlay">
                🚪 Rời lớp
              </button>
            </div>
          </div>
        )}
      </div>

      {!isViewingStream && (
        <>
          <div className="controls">
            {connectionState === 'disconnected' && (
              <button onClick={handleConnect} className="btn primary">
                🔌 Kết nối vào lớp
              </button>
            )}

            {connectionState === 'connected' && (
              <>
                <button
                  onMouseDown={enablePushToTalk}
                  onMouseUp={disablePushToTalk}
                  onMouseLeave={disablePushToTalk}
                  onTouchStart={(e) => {
                    e.preventDefault();
                    enablePushToTalk();
                  }}
                  onTouchEnd={(e) => {
                    e.preventDefault();
                    disablePushToTalk();
                  }}
                  className={`btn push-to-talk ${isPushToTalkActive ? 'active' : ''}`}
                >
                  {isPushToTalkActive ? '🎤 Đang nói...' : '🎤 Nhấn để nói'}
                </button>
                <button onClick={handleDisconnect} className="btn danger">
                  🚪 Rời lớp
                </button>
              </>
            )}
          </div>

          <div className="room-info">
            <p><strong>Room ID:</strong> {roomId}</p>
          </div>

          {connectionState === 'connected' && !remoteStream && (
            <div className="waiting-message">
              ⏳ Đang chờ giáo viên chia sẻ màn hình...
            </div>
          )}
        </>
      )}
    </div>
  );
}
