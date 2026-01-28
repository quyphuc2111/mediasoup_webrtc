import { useEffect, useRef, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

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

interface H264VideoPlayerProps {
  frame?: ScreenFrame | null;
  className?: string;
  connectionId?: string;
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

// Convert number array (from Rust Vec<u8> serialization) to Uint8Array
function arrayToUint8Array(arr: number[]): Uint8Array {
  return new Uint8Array(arr);
}

export function H264VideoPlayer({ frame, className, connectionId }: H264VideoPlayerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const decoderRef = useRef<VideoDecoder | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [decodedFrameCount, setDecodedFrameCount] = useState(0);
  const [useFallback, setUseFallback] = useState(false);
  const lastKeyframeRef = useRef<Uint8Array | null>(null);
  const pendingFramesRef = useRef<ScreenFrame[]>([]);
  const errorCountRef = useRef(0);
  const lastProcessedTimestampRef = useRef<number>(-1);

  // Initialize decoder
  const initDecoder = useCallback((width: number, height: number, description?: Uint8Array) => {
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
          // Use requestAnimationFrame for smooth rendering
          requestAnimationFrame(() => {
            // Draw frame to canvas
            const canvas = canvasRef.current;
            if (canvas) {
              const ctx = canvas.getContext('2d', {
                alpha: false,  // Disable alpha for better performance
                desynchronized: true,  // Allow async rendering
              });
              if (ctx) {
                // Resize canvas if needed
                if (canvas.width !== videoFrame.displayWidth ||
                  canvas.height !== videoFrame.displayHeight) {
                  canvas.width = videoFrame.displayWidth;
                  canvas.height = videoFrame.displayHeight;
                }

                // Clear canvas before drawing new frame
                ctx.clearRect(0, 0, canvas.width, canvas.height);

                // Draw frame
                ctx.drawImage(videoFrame, 0, 0);
                setDecodedFrameCount(prev => prev + 1);
                errorCountRef.current = 0; // Reset error count on success
              }
            }
            videoFrame.close();
          });
        },
        error: (e) => {
          errorCountRef.current++;
          console.error(`[H264Player] Decoder error (${errorCountRef.current}):`, e);

          // Request a new keyframe from the student if we get errors
          if (connectionId) {
            invoke('send_remote_keyframe_request', { connectionId }).catch(console.error);
          }

          // After 5 consecutive errors, fallback to JPEG
          if (errorCountRef.current >= 5) {
            console.warn('[H264Player] Too many errors, falling back to JPEG');
            setUseFallback(true);
            setError('H.264 decode failed, using JPEG fallback');
            return;
          }

          // Don't set error state immediately - might be recoverable
          if (e.message && e.message.includes('decode')) {
            console.warn('[H264Player] Decode error, will retry with next keyframe');
          } else {
            setError(`Decoder error: ${e.message || 'Unknown error'}`);
          }
        },
      });

      // Build codec string from description if available
      let codecStr = 'avc1.42E01f'; // Default Baseline
      if (description && description.length >= 4) {
        // avc1.PPCCLL where PP=profile, CC=compat, LL=level (in hex)
        const profile = description[1].toString(16).padStart(2, '0').toUpperCase();
        const compat = description[2].toString(16).padStart(2, '0').toUpperCase();
        const level = description[3].toString(16).padStart(2, '0').toUpperCase();
        codecStr = `avc1.${profile}${compat}${level}`;
        console.log(`[H264Player] Detected codec from SPS: ${codecStr}`);
      }

      const config: VideoDecoderConfig = {
        codec: codecStr,
        codedWidth: width,
        codedHeight: height,
        optimizeForLatency: true,
        hardwareAcceleration: 'prefer-hardware',
      };

      // NOTE: We do NOT pass description to configure() because our stream is Annex B (start codes).
      // If we pass description (AVCC), WebCodecs expects length-prefixed NALUs.
      // We only use description to extract the correct codec string.
      /* 
      if (description) {
        config.description = description; 
      }
      */

      decoder.configure(config);
      setIsInitialized(true);
      setError(null);
      errorCountRef.current = 0;
      decoderRef.current = decoder;
      console.log(`[H264Player] Decoder initialized for ${width}x${height}`, description ? 'with description' : '');
      return true;
    } catch (e: any) {
      console.error('[H264Player] Failed to init decoder:', e);
      setError(`Init failed: ${e?.message || e}`);
      return false;
    }
  }, []);

  // Decode a frame
  const decodeFrame = useCallback((frameData: ScreenFrame) => {
    const decoder = decoderRef.current;
    if (!decoder) {
      pendingFramesRef.current.push(frameData);
      return;
    }

    // Skip if we've already processed this exact timestamp
    if (frameData.timestamp === lastProcessedTimestampRef.current) {
      return;
    }

    // Get binary data (H.264 uses data_binary, JPEG uses data)
    let h264Data: Uint8Array | null = null;
    if (frameData.codec === 'h264' && frameData.data_binary) {
      h264Data = arrayToUint8Array(frameData.data_binary);
      // Only log keyframes to reduce noise
      if (frameData.is_keyframe) {
        console.log(`[H264Player] Frame received: size=${h264Data.length}, keyframe=true, has_sps_pps=${!!frameData.sps_pps}`);
      }
    } else if (frameData.codec === 'h264' && frameData.data) {
      // Fallback: if data_binary not available, use base64 data
      h264Data = base64ToUint8Array(frameData.data);
      if (frameData.is_keyframe) {
        console.log(`[H264Player] Frame received (base64): size=${h264Data.length}, keyframe=true`);
      }
    }

    if (!h264Data || h264Data.length === 0) {
      console.warn('[H264Player] No valid H.264 data');
      return;
    }

    // If decoder is not configured, try to initialize with AVCC description from backend
    if (decoder.state !== 'configured') {
      if (frameData.is_keyframe && frameData.sps_pps) {
        console.log('[H264Player] Initializing decoder with keyframe and SPS/PPS');
        const description = arrayToUint8Array(frameData.sps_pps);
        if (initDecoder(frameData.width, frameData.height, description)) {
          // Retry decode after init
          decodeFrame(frameData);
        } else {
          console.error('[H264Player] Failed to initialize decoder');
        }
      } else {
        if (frameData.is_keyframe && !frameData.sps_pps) {
          console.warn('[H264Player] Keyframe received but no SPS/PPS description');
        } else if (!frameData.is_keyframe) {
          // Only log this once
          if (pendingFramesRef.current.length === 0) {
            console.log('[H264Player] Waiting for keyframe before initializing decoder');
          }
        }
        pendingFramesRef.current.push(frameData);
        // Keep only last 10 frames to avoid memory issues
        if (pendingFramesRef.current.length > 10) {
          pendingFramesRef.current.shift();
        }
      }
      return;
    }

    try {
      // Log first few bytes to check format (only for keyframes)
      if (frameData.is_keyframe && h264Data.length >= 8) {
        const preview = Array.from(h264Data.slice(0, 8)).map(b => `0x${b.toString(16).padStart(2, '0')}`).join(' ');
        console.log(`[H264Player] Keyframe data preview (${h264Data.length} bytes):`, preview);
      }

      // Store keyframes for recovery
      if (frameData.is_keyframe) {
        lastKeyframeRef.current = h264Data;

        // Don't reconfigure if already configured - just decode
        // Reconfiguring mid-stream can cause decoder errors
      }

      const chunk = new EncodedVideoChunk({
        type: frameData.is_keyframe ? 'key' : 'delta',
        timestamp: frameData.timestamp * 1000, // Convert to microseconds
        duration: undefined,
        data: h264Data,
      });

      decoder.decode(chunk);

      // Mark this timestamp as processed
      lastProcessedTimestampRef.current = frameData.timestamp;
    } catch (e: any) {
      console.error('[H264Player] Decode error:', e, 'Frame size:', h264Data.length, 'isKeyQQrame:', frameData.is_keyframe);

      // If decode fails and we have a keyframe, try to reset decoder
      if (frameData.is_keyframe && lastKeyframeRef.current && frameData.sps_pps) {
        console.log('[H264Player] Attempting decoder reset with stored keyframe');
        setIsInitialized(false);
        const description = arrayToUint8Array(frameData.sps_pps);
        if (initDecoder(frameData.width, frameData.height, description)) {
          // Retry with the keyframe
          try {
            const chunk = new EncodedVideoChunk({
              type: 'key',
              timestamp: frameData.timestamp * 1000,
              duration: undefined,
              data: lastKeyframeRef.current,
            });
            decoderRef.current?.decode(chunk);
            lastProcessedTimestampRef.current = frameData.timestamp;
          } catch (retryError) {
            console.error('[H264Player] Retry decode also failed:', retryError);
          }
        }
      }
    }
  }, [initDecoder]);

  // Handle new frames
  useEffect(() => {
    if (!frame) return;

    // For JPEG codec, fall back to img display (handled elsewhere)
    if (frame.codec !== 'h264') return;

    // Initialize decoder if needed
    if (!isInitialized || decoderRef.current?.state === 'closed') {
      if (frame.is_keyframe && frame.sps_pps) {
        const description = arrayToUint8Array(frame.sps_pps);
        if (initDecoder(frame.width, frame.height, description)) {
          // Clear pending frames - they're stale without this keyframe
          pendingFramesRef.current = [];
          // Decode the keyframe first
          decodeFrame(frame);
        }
      } else {
        // Need keyframe with description first
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

  // Fallback for JPEG or when WebCodecs not available or H.264 fails
  if (frame && (frame.codec === 'jpeg' || useFallback)) {
    if (!frame.data) {
      return (
        <div className={className} style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#000',
          color: '#fff'
        }}>
          No JPEG data available
        </div>
      );
    }
    return (
      <img
        src={`data:image/jpeg;base64,${frame.data}`}
        alt="Screen"
        className={className}
        style={{ width: '100%', height: '100%', objectFit: 'contain' }}
      />
    );
  }

  // If no frame or not H.264, show placeholder
  if (!frame || frame.codec !== 'h264') {
    return (
      <div className={className} style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#000',
        color: '#fff'
      }}>
        {!frame ? 'Chờ frame...' : `Codec: ${frame.codec}`}
      </div>
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
