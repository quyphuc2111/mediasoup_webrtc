import { useEffect, useRef } from 'react';

interface VideoPlayerProps {
  stream: MediaStream | null;
  muted?: boolean;
  className?: string;
  label?: string;
  disableInteraction?: boolean;
}

export function VideoPlayer({ stream, muted = false, className = '', label, disableInteraction = false }: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (videoRef.current && stream) {
      videoRef.current.srcObject = stream;
    }
  }, [stream]);

  return (
    <div className={`video-container ${className} ${disableInteraction ? 'no-interaction' : ''}`}>
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
          borderRadius: disableInteraction ? '0' : '8px',
        }}
      />
      {disableInteraction && <div className="interaction-blocker" />}
      {!stream && (
        <div className="video-placeholder">
          <span>Đang chờ stream...</span>
        </div>
      )}
    </div>
  );
}
