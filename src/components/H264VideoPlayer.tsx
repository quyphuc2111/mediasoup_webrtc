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
  onStats?: (stats: PlayerStats) => void;
}

export interface PlayerStats {
  fps: number;
  width: number;
  height: number;
  codec: string;
  bitrateMbps: number;
  errorCount: number;
  decoder: string;
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

// Convert Annex-B (start codes) to AVCC (length-prefixed) format
// This is required when using config.description (avcC format)
function annexBToAvcc(annexB: Uint8Array): Uint8Array {
  const nals: Uint8Array[] = [];
  let i = 0;

  const isStartCode3 = (d: Uint8Array, p: number) =>
    p + 3 <= d.length && d[p] === 0 && d[p + 1] === 0 && d[p + 2] === 1;
  const isStartCode4 = (d: Uint8Array, p: number) =>
    p + 4 <= d.length && d[p] === 0 && d[p + 1] === 0 && d[p + 2] === 0 && d[p + 3] === 1;

  // Find first start code
  while (i < annexB.length && !isStartCode3(annexB, i) && !isStartCode4(annexB, i)) i++;

  while (i < annexB.length) {
    const scSize = isStartCode4(annexB, i) ? 4 : 3;
    i += scSize;
    const nalStart = i;

    // Find next start code
    while (i < annexB.length && !isStartCode3(annexB, i) && !isStartCode4(annexB, i)) i++;
    const nalEnd = i;

    if (nalEnd > nalStart) nals.push(annexB.subarray(nalStart, nalEnd));
  }

  // Calculate total size (4 bytes length + NAL data for each NAL)
  let total = 0;
  for (const nal of nals) total += 4 + nal.length;

  const out = new Uint8Array(total);
  let o = 0;
  for (const nal of nals) {
    const len = nal.length;
    // Write 4-byte length prefix (big-endian)
    out[o++] = (len >>> 24) & 0xff;
    out[o++] = (len >>> 16) & 0xff;
    out[o++] = (len >>> 8) & 0xff;
    out[o++] = len & 0xff;
    // Write NAL data
    out.set(nal, o);
    o += len;
  }
  return out;
}

export function H264VideoPlayer({ frame, className, connectionId, onStats }: H264VideoPlayerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const decoderRef = useRef<VideoDecoder | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [useFallback, setUseFallback] = useState(false);
  const [forceSoftware, setForceSoftware] = useState(false);
  const lastKeyframeRef = useRef<Uint8Array | null>(null);
  const pendingFramesRef = useRef<ScreenFrame[]>([]);
  const errorCountRef = useRef(0);
  const lastProcessedTimestampRef = useRef<number>(-1);
  const isInitializingRef = useRef(false);
  const usingDescriptionRef = useRef(false); // Track if decoder is using AVCC format (description set)

  // Stats refs
  const frameCountRef = useRef(0);
  const byteCountRef = useRef(0);
  const lastStatsTimeRef = useRef(performance.now());
  const currentCodecRef = useRef('avc1.42E01f');
  const currentResolutionRef = useRef({ width: 0, height: 0 });

  // Initialize decoder
  const initDecoder = useCallback(async (width: number, height: number, description?: Uint8Array) => {
    if (!isWebCodecsSupported()) {
      setError('WebCodecs không được hỗ trợ trong browser này');
      return false;
    }

    if (isInitializingRef.current) return false;
    isInitializingRef.current = true;

    try {
      // Close existing decoder
      if (decoderRef.current && decoderRef.current.state !== 'closed') {
        decoderRef.current.close();
      }

      // Prepare config with description (AVCC format)
      // RECOMMENDATION: Always use 'avc1.42E01f' (Baseline 3.1) as the base codec string.
      // The actual profile/level is strictly defined in the 'description' (avcC) we provide.
      let codecStr = 'avc1.42E01f';
      let hasDescription = false;
      let realProfileLevel = '';

      if (description && description.length >= 4) {
        const profile = description[1].toString(16).padStart(2, '0').toUpperCase();
        const compat = description[2].toString(16).padStart(2, '0').toUpperCase();
        const level = description[3].toString(16).padStart(2, '0').toUpperCase();
        realProfileLevel = `avc1.${profile}${compat}${level}`;
        // We stick to standard baseline string for compatibility
        hasDescription = true;
      }

      const config: VideoDecoderConfig = {
        codec: codecStr,
        codedWidth: Math.ceil(width / 16) * 16, // Align to 16 (macroblock size)
        codedHeight: Math.ceil(height / 16) * 16, // Align to 16
        // CRITICAL: Set description for AVCC format (ISO 14496-15)
        // When description is set, decoder expects length-prefixed bitstream, not Annex-B
        ...(hasDescription && description ? { description } : {}),
        optimizeForLatency: !forceSoftware,
        hardwareAcceleration: forceSoftware ? 'prefer-software' : 'prefer-hardware',
      };

      // Track if we're using AVCC format (will need to convert Annex-B → AVCC)
      usingDescriptionRef.current = hasDescription;

      console.log(`[H264Player] Decoder config attempt:`, {
        codec: codecStr,
        realProfile: realProfileLevel,
        originalSize: `${width}x${height}`,
        codedSize: `${config.codedWidth}x${config.codedHeight}`,
        hasDescription,
        hardwareAcceleration: config.hardwareAcceleration,
        optimizeForLatency: config.optimizeForLatency,
      });

      // Check support proactively and adjust config as needed
      // Check support proactively
      try {
        const support = await VideoDecoder.isConfigSupported(config);

        if (!support.supported) {
          // Do NOT automatically switch to software here. 
          // Windows D3D11 often reports "false" for isConfigSupported but configure() works fine.
          // We only switch to software if the USER forces it (via error-triggered retry).
          console.warn(`[H264Player] isConfigSupported reports FALSE for ${codecStr}, but proceeding anyway (potential false negative).`);

          // Optional: If we are not forcing software yet, we might want to try checking software support just to know
          if (!forceSoftware) {
            const softConfig = { ...config, hardwareAcceleration: 'prefer-software' as const };
            VideoDecoder.isConfigSupported(softConfig).then(s => {
              if (s.supported) console.log('[H264Player] Software decoder WOULD be supported.');
            });
          }
        } else {
          console.log('[H264Player] Configuration verified as supported:', config);
        }

      } catch (checkErr) {
        console.warn("[H264Player] isConfigSupported check failed:", checkErr);
      }

      // Create Decoder
      const decoder = new VideoDecoder({
        output: (videoFrame) => {
          requestAnimationFrame(() => {
            const canvas = canvasRef.current;
            if (canvas) {
              const ctx = canvas.getContext('2d', {
                alpha: false,
                desynchronized: true,
              });
              if (ctx) {
                if (canvas.width !== videoFrame.displayWidth ||
                  canvas.height !== videoFrame.displayHeight) {
                  canvas.width = videoFrame.displayWidth;
                  canvas.height = videoFrame.displayHeight;
                }
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                ctx.drawImage(videoFrame, 0, 0);
                errorCountRef.current = 0;
              }
            }
            videoFrame.close();
          });
        },
        error: (e) => {
          errorCountRef.current++;
          console.error(`[H264Player] Decoder error (${errorCountRef.current}):`, e);

          // AGGRESSIVE FALLBACK: If any error occurs and we are using hardware, switch to software immediately.
          // This covers "Unsupported configuration", "Decode error", and unknown errors on Windows.
          if (!forceSoftware) {
            console.warn('[H264Player] Decoder error detected. Switching to persistent software decoding & disabling latency opt.');
            setForceSoftware(true);
            // Reset error count to give software a chance
            errorCountRef.current = 0;
            return;
          }

          if (connectionId) {
            invoke('send_remote_keyframe_request', { connectionId }).catch(console.error);
          }

          if (errorCountRef.current >= 5) {
            console.warn('[H264Player] Too many errors, falling back to JPEG');
            setUseFallback(true);
            setError('H.264 decode failed, using JPEG fallback');
            return;
          }

          if (e.message && e.message.includes('decode')) {
            console.warn('[H264Player] Decode error, will retry with next keyframe');
          } else {
            setError(`Decoder error: ${e.message || 'Unknown error'}`);
          }
        },
      });

      // Update current resolution for stats
      currentResolutionRef.current = { width, height };

      // Configure decoder with verified config
      try {
        decoder.configure(config);
      } catch (e: any) {
        console.error(`[H264Player] Configure failed even after verification: ${e.message}`);
        setError(`Failed to configure decoder: ${e.message}`);
        return false;
      }

      setIsInitialized(true);
      setError(null);
      errorCountRef.current = 0;
      decoderRef.current = decoder;
      console.log(`[H264Player] Decoder initialized for ${width}x${height} ${config.hardwareAcceleration === 'prefer-software' ? '[SOFTWARE]' : '[HARDWARE]'}`);
      return true;
    } catch (e: any) {
      console.error('[H264Player] Failed to init decoder:', e);
      setError(`Init failed: ${e?.message || e}`);
      return false;
    } finally {
      isInitializingRef.current = false;
    }
  }, [connectionId, forceSoftware]);

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

        // Wait for async init
        initDecoder(frameData.width, frameData.height, description).then((success) => {
          if (success) {
            // Retry decode after init
            decodeFrame(frameData);
          }
        });
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
      }

      // CRITICAL: Convert Annex-B to AVCC if decoder is using description
      // When config.description is set, decoder expects AVCC (length-prefixed) format
      let dataForDecoder = h264Data;
      if (usingDescriptionRef.current) {
        dataForDecoder = annexBToAvcc(h264Data);
        if (frameData.is_keyframe) {
          console.log(`[H264Player] Converted Annex-B → AVCC: ${h264Data.length} → ${dataForDecoder.length} bytes`);
        }
      }

      const chunk = new EncodedVideoChunk({
        type: frameData.is_keyframe ? 'key' : 'delta',
        timestamp: frameData.timestamp * 1000, // Convert to microseconds
        duration: undefined,
        data: dataForDecoder,
      });

      decoder.decode(chunk);

      // Update stats
      frameCountRef.current++;
      byteCountRef.current += h264Data.byteLength;

      const now = performance.now();
      if (now - lastStatsTimeRef.current >= 1000) {
        const elapsed = (now - lastStatsTimeRef.current) / 1000;
        const fps = Math.round(frameCountRef.current / elapsed);
        const bitrate = (byteCountRef.current * 8) / (1024 * 1024) / elapsed; // Mbps

        if (onStats) {
          onStats({
            fps,
            width: currentResolutionRef.current.width,
            height: currentResolutionRef.current.height,
            codec: currentCodecRef.current,
            bitrateMbps: parseFloat(bitrate.toFixed(2)),
            errorCount: errorCountRef.current,
            decoder: 'Hardware (WebCodecs)',
          });
        }

        frameCountRef.current = 0;
        byteCountRef.current = 0;
        lastStatsTimeRef.current = now;
      }

      // Mark this timestamp as processed
      lastProcessedTimestampRef.current = frameData.timestamp;
    } catch (e: any) {
      console.error('[H264Player] Decode error:', e, 'Frame size:', h264Data.length, 'isKeyQQrame:', frameData.is_keyframe);

      // If decode fails and we have a keyframe, try to reset decoder
      if (frameData.is_keyframe && lastKeyframeRef.current && frameData.sps_pps) {
        console.log('[H264Player] Attempting decoder reset with stored keyframe');
        setIsInitialized(false);
        const description = arrayToUint8Array(frameData.sps_pps);
        initDecoder(frameData.width, frameData.height, description).then((success) => {
          if (success) {
            // Retry with the keyframe (convert if needed)
            try {
              let retryData = lastKeyframeRef.current!;
              if (usingDescriptionRef.current) {
                retryData = annexBToAvcc(retryData);
                console.log('[H264Player] Retry: Converted keyframe to AVCC');
              }
              const chunk = new EncodedVideoChunk({
                type: 'key',
                timestamp: frameData.timestamp * 1000,
                duration: undefined,
                data: retryData,
              });
              decoderRef.current?.decode(chunk);
              lastProcessedTimestampRef.current = frameData.timestamp;
            } catch (retryError) {
              console.error('[H264Player] Retry decode also failed:', retryError);
            }
          }
        });
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
        initDecoder(frame.width, frame.height, description).then((success) => {
          if (success) {
            // Clear pending frames - they're stale without this keyframe
            pendingFramesRef.current = [];
            // Decode the keyframe first
            decodeFrame(frame);
          }
        });
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
        H.264
      </div>
    </div>
  );
}

export default H264VideoPlayer;
