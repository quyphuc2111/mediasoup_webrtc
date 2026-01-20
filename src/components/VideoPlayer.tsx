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
    if (videoRef.current && stream) {
      videoRef.current.srcObject = stream;
    }
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
