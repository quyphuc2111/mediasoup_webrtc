import { useEffect, useRef, useState, useCallback } from 'react';

interface ScreenFrame {
  data: string;  // Base64 encoded H.264 NAL units
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;
}

interface H264VideoPlayerProps {
  frame?: ScreenFrame | null;
  className?: string;
}

// Check if WebCodecs is supported
const isWebCodecsSupported = () => {
  return typeof VideoDecoder !== 'undefined' && typeof VideoFrame !== 'undefined';
};

// Convert base64 to Uint8Array
function base64ToUint8Array(base64: string): Uint8Array {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

export function H264VideoPlayer({ frame, className }: H264VideoPlayerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const decoderRef = useRef<VideoDecoder | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [decodedFrameCount, setDecodedFrameCount] = useState(0);
  const lastKeyframeRef = useRef<Uint8Array | null>(null);
  const pendingFramesRef = useRef<ScreenFrame[]>([]);

  // Initialize decoder
  const initDecoder = useCallback((width: number, height: number) => {
    if (!isWebCodecsSupported()) {
      setError('WebCodecs không được hỗ trợ trong browser này');
      return false;
    }

    try {
      // Close existing decoder
      if (decoderRef.current && decoderRef.current.state !== 'closed') {
        decoderRef.current.close();
      }

      const decoder = new VideoDecoder({
        output: (videoFrame) => {
          // Draw frame to canvas
          const canvas = canvasRef.current;
          if (canvas) {
            const ctx = canvas.getContext('2d');
            if (ctx) {
              // Resize canvas if needed
              if (canvas.width !== videoFrame.displayWidth || 
                  canvas.height !== videoFrame.displayHeight) {
                canvas.width = videoFrame.displayWidth;
                canvas.height = videoFrame.displayHeight;
              }
              ctx.drawImage(videoFrame, 0, 0);
              setDecodedFrameCount(prev => prev + 1);
            }
          }
          videoFrame.close();
        },
        error: (e) => {
          console.error('[H264Player] Decoder error:', e);
          setError(`Decoder error: ${e.message}`);
        },
      });

      // Configure for H.264 Baseline profile
      decoder.configure({
        codec: 'avc1.42001f', // H.264 Baseline Level 3.1
        codedWidth: width,
        codedHeight: height,
        optimizeForLatency: true,
      });

      decoderRef.current = decoder;
      setIsInitialized(true);
      setError(null);
      console.log(`[H264Player] Decoder initialized for ${width}x${height}`);
      return true;
    } catch (e) {
      console.error('[H264Player] Failed to init decoder:', e);
      setError(`Init failed: ${e}`);
      return false;
    }
  }, []);

  // Decode a frame
  const decodeFrame = useCallback((frameData: ScreenFrame) => {
    const decoder = decoderRef.current;
    if (!decoder || decoder.state !== 'configured') {
      // Queue frame for later
      pendingFramesRef.current.push(frameData);
      return;
    }

    try {
      const data = base64ToUint8Array(frameData.data);
      
      // Store keyframes for recovery
      if (frameData.is_keyframe) {
        lastKeyframeRef.current = data;
      }

      const chunk = new EncodedVideoChunk({
        type: frameData.is_keyframe ? 'key' : 'delta',
        timestamp: frameData.timestamp * 1000, // Convert to microseconds
        data: data,
      });

      decoder.decode(chunk);
    } catch (e) {
      console.error('[H264Player] Decode error:', e);
    }
  }, []);

  // Handle new frames
  useEffect(() => {
    if (!frame) return;

    // For JPEG codec, fall back to img display (handled elsewhere)
    if (frame.codec !== 'h264') return;

    // Initialize decoder if needed
    if (!isInitialized || decoderRef.current?.state === 'closed') {
      if (frame.is_keyframe) {
        if (initDecoder(frame.width, frame.height)) {
          // Process any pending frames
          while (pendingFramesRef.current.length > 0) {
            const pending = pendingFramesRef.current.shift();
            if (pending) decodeFrame(pending);
          }
          decodeFrame(frame);
        }
      } else {
        // Need keyframe first
        pendingFramesRef.current.push(frame);
      }
      return;
    }

    decodeFrame(frame);
  }, [frame, isInitialized, initDecoder, decodeFrame]);

  // Cleanup
  useEffect(() => {
    return () => {
      if (decoderRef.current && decoderRef.current.state !== 'closed') {
        decoderRef.current.close();
      }
    };
  }, []);

  // Fallback for JPEG or when WebCodecs not available
  if (frame && frame.codec === 'jpeg') {
    return (
      <img
        src={`data:image/jpeg;base64,${frame.data}`}
        alt="Screen"
        className={className}
        style={{ width: '100%', height: '100%', objectFit: 'contain' }}
      />
    );
  }

  return (
    <div className={`h264-player ${className || ''}`} style={{ position: 'relative', width: '100%', height: '100%' }}>
      <canvas
        ref={canvasRef}
        style={{ 
          width: '100%', 
          height: '100%', 
          objectFit: 'contain',
          backgroundColor: '#000',
        }}
      />
      {error && (
        <div style={{
          position: 'absolute',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          color: 'red',
          background: 'rgba(0,0,0,0.8)',
          padding: '1rem',
          borderRadius: '8px',
        }}>
          {error}
        </div>
      )}
      {!isInitialized && !error && frame && (
        <div style={{
          position: 'absolute',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          color: '#fff',
        }}>
          Đang chờ keyframe...
        </div>
      )}
      <div style={{
        position: 'absolute',
        bottom: '8px',
        right: '8px',
        color: '#fff',
        fontSize: '12px',
        background: 'rgba(0,0,0,0.5)',
        padding: '2px 6px',
        borderRadius: '4px',
      }}>
        H.264 | {decodedFrameCount} frames
      </div>
    </div>
  );
}

export default H264VideoPlayer;
