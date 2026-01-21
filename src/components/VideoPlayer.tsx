import { useEffect, useRef } from 'react';

interface VideoPlayerProps {
  stream: MediaStream | null;
  muted?: boolean;
  className?: string;
  label?: string;
}

export function VideoPlayer({ stream, muted = false, className = '', label }: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    if (stream) {
      console.log(`[VideoPlayer] Setting stream with ${stream.getTracks().length} tracks:`, 
        stream.getTracks().map(t => `${t.kind} (${t.id})`));
      
      video.srcObject = stream;
      
      // Đảm bảo video play (quan trọng cho Student)
      video.play().catch((error) => {
        console.warn('[VideoPlayer] Auto-play prevented, user interaction required:', error);
      });
    } else {
      video.srcObject = null;
    }

    // Cleanup
    return () => {
      if (video.srcObject) {
        const tracks = (video.srcObject as MediaStream).getTracks();
        tracks.forEach(track => {
          // Don't stop tracks here - let useMediasoup handle cleanup
          console.log(`[VideoPlayer] Track ${track.id} still active`);
        });
      }
    };
  }, [stream]);

  return (
    <div className={`video-container ${className}`}>
      {label && <div className="video-label">{label}</div>}
      <video
        ref={videoRef}
        autoPlay
        playsInline
        muted={muted}
        style={{
          width: '100%',
          height: '100%',
          objectFit: 'contain',
          backgroundColor: '#1a1a1a',
          borderRadius: '8px',
        }}
      />
      {!stream && (
        <div className="video-placeholder">
          <span>Đang chờ stream...</span>
        </div>
      )}
    </div>
  );
}
