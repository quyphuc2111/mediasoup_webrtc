import { useState, useCallback, useRef, useEffect } from 'react';
import { MediasoupClient, ConnectionState, MediaKind } from '../lib/mediasoup-client';

interface Peer {
  id: string;
  name: string;
  isTeacher: boolean;
}

// Helper function to check MediaDevices availability
const checkMediaDevicesSupport = () => {
  const nav = typeof navigator !== 'undefined' ? navigator : (window as any).navigator;
  
  if (!nav) {
    return { available: false, error: 'Navigator không tồn tại' };
  }

  // Try to access mediaDevices - may be undefined in some WebView contexts
  let mediaDevices = nav.mediaDevices;
  
  // If mediaDevices doesn't exist, try to initialize it (for older browsers/WebViews)
  if (!mediaDevices) {
    // Try legacy APIs as fallback
    if (nav.getUserMedia) {
      console.warn('Using legacy getUserMedia API');
    }
    
    // For Tauri WebView, mediaDevices might not be initialized immediately
    // Try to access it via a getter or wait
    try {
      // Check if we're in a secure context (required for mediaDevices)
      if (typeof window !== 'undefined' && (window.location.protocol === 'https:' || window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1')) {
        // MediaDevices should be available in secure context
        // It might be lazily initialized
        mediaDevices = nav.mediaDevices;
      }
    } catch (e) {
      console.warn('Error checking secure context:', e);
    }

    if (!mediaDevices) {
      return { 
        available: false, 
        error: 'navigator.mediaDevices không tồn tại. Có thể WebView chưa hỗ trợ MediaDevices API hoặc chưa được cấu hình đúng.',
        userAgent: nav.userAgent,
        isSecureContext: typeof window !== 'undefined' ? (window.location.protocol === 'https:' || window.location.hostname === 'localhost') : false
      };
    }
  }

  const md = mediaDevices;
  
  return {
    available: true,
    hasGetDisplayMedia: typeof md.getDisplayMedia === 'function',
    hasGetUserMedia: typeof md.getUserMedia === 'function',
    methods: Object.keys(md),
    userAgent: nav.userAgent,
    isSecureContext: typeof window !== 'undefined' ? (window.location.protocol === 'https:' || window.location.hostname === 'localhost') : false
  };
};

export function useMediasoup() {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [remoteStream, setRemoteStream] = useState<MediaStream | null>(null);
  const [isSharing, setIsSharing] = useState(false);
  const [localStream, setLocalStream] = useState<MediaStream | null>(null);
  const [isMicActive, setIsMicActive] = useState(false);
  const [micStream, setMicStream] = useState<MediaStream | null>(null);
  const micProducerIdRef = useRef<string | null>(null);
  const [isPushToTalkActive, setIsPushToTalkActive] = useState(false);
  const [studentAudioStream, setStudentAudioStream] = useState<MediaStream | null>(null);
  const [studentVideoStreams, setStudentVideoStreams] = useState<Map<string, MediaStream>>(new Map());

  const clientRef = useRef<MediasoupClient | null>(null);

  useEffect(() => {
    // Check MediaDevices availability on mount for debugging
    const checkSupport = () => {
      const support = checkMediaDevicesSupport();
      console.log('[useMediasoup] MediaDevices Support on Mount:', support);
      
      if (support.available && support.hasGetDisplayMedia) {
        console.log('[useMediasoup] ✅ getDisplayMedia is available');
      } else {
        console.warn('[useMediasoup] ⚠️ getDisplayMedia may not be available:', support);
      }
    };

    // Check immediately
    checkSupport();

    // Also check after a short delay (in case WebView needs time to initialize)
    const timeoutId = setTimeout(checkSupport, 500);

    return () => {
      clearTimeout(timeoutId);
      clientRef.current?.disconnect();
    };
  }, []);

  const connect = useCallback(async (
    serverUrl: string,
    roomId: string,
    name: string,
    isTeacher: boolean
  ) => {
    const peerId = crypto.randomUUID();

    const client = new MediasoupClient({
      onConnectionStateChange: setConnectionState,
      onError: setError,
      onPeerJoined: (id, peerName, peerIsTeacher) => {
        setPeers(prev => [...prev, { id, name: peerName, isTeacher: peerIsTeacher }]);
      },
      onPeerLeft: (id, wasTeacher) => {
        setPeers(prev => prev.filter(p => p.id !== id));
        if (wasTeacher) {
          setRemoteStream(null);
        }
      },
      onNewProducer: async (producerId: string, kind: MediaKind, peerId?: string) => {
        // Students: consume teacher's producers (video + audio)
        if (!client.isTeacher) {
          console.log(`[Student] New producer detected: ${producerId}, kind: ${kind}`);
          console.log(`[Student] Client connection state: ${clientRef.current ? 'exists' : 'null'}`);
          
          // Wait a bit to ensure transport is ready (in case of race condition)
          await new Promise(resolve => setTimeout(resolve, 100));
          
          if (!clientRef.current) {
            console.error(`[Student] ❌ Client is null, cannot consume producer ${producerId}`);
            return;
          }
          
          try {
            const consumer = await clientRef.current.consume(producerId);
            if (consumer) {
              console.log(`[Student] ✅ Successfully consumed producer ${producerId}, track:`, consumer.track);
              setRemoteStream(prev => {
                const stream = prev || new MediaStream();
                
                // Remove old track of the same kind to avoid duplicates
                const existingTracks = stream.getTracks().filter(t => t.kind === consumer.track.kind);
                existingTracks.forEach(track => {
                  stream.removeTrack(track);
                  track.stop();
                  console.log(`[Student] Removed old ${track.kind} track`);
                });
                
                // Add new track
                stream.addTrack(consumer.track);
                console.log(`[Student] Stream now has ${stream.getTracks().length} tracks (kind: ${stream.getTracks().map(t => t.kind).join(', ')})`);
                return stream;
              });
            } else {
              console.error(`[Student] ❌ Failed to consume producer ${producerId} - consumer is null`);
              setError(`Không thể nhận được stream từ producer ${producerId}`);
            }
          } catch (err) {
            console.error(`[Student] ❌ Error consuming producer ${producerId}:`, err);
            setError(err instanceof Error ? err.message : `Failed to consume producer ${producerId}`);
          }
        } else {
          // Teacher: consume student producers (audio or video for remote control)
          if (peerId && peerId !== client.peerId) {
            console.log(`[Teacher] New student ${kind} producer detected: ${producerId} from peer ${peerId}`);
            
            await new Promise(resolve => setTimeout(resolve, 100));
            
            if (!clientRef.current) {
              console.error(`[Teacher] ❌ Client is null, cannot consume producer ${producerId}`);
              return;
            }
            
            try {
              const consumer = await clientRef.current.consume(producerId);
              if (consumer) {
                console.log(`[Teacher] ✅ Successfully consumed student ${kind} producer ${producerId}`);
                
                if (kind === 'audio') {
                  setStudentAudioStream(prev => {
                    const stream = prev || new MediaStream();
                    // Remove old tracks
                    const existingTracks = stream.getTracks();
                    existingTracks.forEach(track => {
                      stream.removeTrack(track);
                      track.stop();
                    });
                    // Add new track
                    stream.addTrack(consumer.track);
                    return stream;
                  });
                } else if (kind === 'video') {
                  // Store student video stream for remote control
                  setStudentVideoStreams(prev => {
                    const newMap = new Map(prev);
                    const stream = newMap.get(peerId) || new MediaStream();
                    // Remove old video tracks from this peer
                    const existingVideoTracks = stream.getVideoTracks();
                    existingVideoTracks.forEach(track => {
                      stream.removeTrack(track);
                      track.stop();
                    });
                    // Add new track
                    stream.addTrack(consumer.track);
                    newMap.set(peerId, stream);
                    return newMap;
                  });
                }
              }
            } catch (err) {
              console.error(`[Teacher] ❌ Error consuming student ${kind} producer ${producerId}:`, err);
            }
          }
        }
      },
      onStreamReady: setRemoteStream,
      onControl: async (action: string) => {
        console.log('[useMediasoup] onControl called, action:', action, 'isTeacher:', isTeacher);
        // Only handle control for students
        if (!isTeacher) {
          console.log('[useMediasoup] Student received control command:', action);
          try {
            const { invoke } = await import('@tauri-apps/api/core');
            console.log('[useMediasoup] Calling control_computer command with action:', action);
            const result = await invoke<string>('control_computer', { action });
            console.log('[useMediasoup] ✅ Control command executed successfully:', result);
          } catch (err) {
            console.error('[useMediasoup] ❌ Failed to execute control command:', err);
            setError('Không thể thực thi lệnh điều khiển. Lỗi: ' + (err instanceof Error ? err.message : String(err)));
          }
        } else {
          console.log('[useMediasoup] Teacher received control command (ignored)');
        }
      },
      onMouseControl: async (event: any) => {
        console.log('[useMediasoup] onMouseControl called, event:', event, 'isTeacher:', isTeacher);
        if (!isTeacher) {
          console.log('[useMediasoup] Student received mouse control event');
          try {
            const { invoke } = await import('@tauri-apps/api/core');
            console.log('[useMediasoup] Calling control_mouse command');
            const result = await invoke<string>('control_mouse', { event });
            console.log('[useMediasoup] ✅ Mouse control executed successfully:', result);
          } catch (err) {
            console.error('[useMediasoup] ❌ Failed to execute mouse control:', err);
            setError('Không thể điều khiển chuột. Lỗi: ' + (err instanceof Error ? err.message : String(err)));
          }
        }
      },
      onKeyboardControl: async (event: any) => {
        console.log('[useMediasoup] onKeyboardControl called, event:', event, 'isTeacher:', isTeacher);
        if (!isTeacher) {
          console.log('[useMediasoup] Student received keyboard control event');
          try {
            const { invoke } = await import('@tauri-apps/api/core');
            console.log('[useMediasoup] Calling control_keyboard command');
            const result = await invoke<string>('control_keyboard', { event });
            console.log('[useMediasoup] ✅ Keyboard control executed successfully:', result);
          } catch (err) {
            console.error('[useMediasoup] ❌ Failed to execute keyboard control:', err);
            setError('Không thể điều khiển bàn phím. Lỗi: ' + (err instanceof Error ? err.message : String(err)));
          }
        }
      },
      onRequestScreenShare: async () => {
        console.log('[useMediasoup] onRequestScreenShare called, isTeacher:', isTeacher);
        if (!isTeacher) {
          console.log('[useMediasoup] Student received screen share request');
          // Automatically start screen sharing
          try {
            await startScreenShare(false); // Start without audio
            console.log('[useMediasoup] ✅ Student started screen sharing for remote control');
            
            // Send screen size to teacher after screen share starts
            try {
              const { invoke } = await import('@tauri-apps/api/core');
              const screenSize = await invoke<{ width: number; height: number }>('get_screen_size');
              console.log('[useMediasoup] Student screen size:', screenSize);
              
              // Send screen size via WebSocket
              if (clientRef.current) {
                clientRef.current.sendScreenSize(screenSize);
                console.log('[useMediasoup] ✅ Sent screen size to teacher');
              }
            } catch (err) {
              console.warn('[useMediasoup] Failed to get/send screen size:', err);
            }
          } catch (err) {
            console.error('[useMediasoup] ❌ Failed to start screen sharing:', err);
            setError('Không thể chia sẻ màn hình. Lỗi: ' + (err instanceof Error ? err.message : String(err)));
          }
        }
      },
    });

    clientRef.current = client;

    try {
      await client.connect(serverUrl, roomId, peerId, name, isTeacher);

      // Students: create recv transport and consume existing producers
      if (!isTeacher) {
        try {
          console.log('[Student] Attempting to consume all existing producers...');
          const stream = await client.consumeAll();
          console.log('[Student] ConsumeAll result:', stream, 'tracks:', stream?.getTracks().length);
          
          // Ensure remoteStream is set even if consumeAll returns empty stream
          if (stream && stream.getTracks().length > 0) {
            setRemoteStream(stream);
            console.log('[Student] ✅ Remote stream set with', stream.getTracks().length, 'tracks');
          } else {
            console.log('[Student] No producers yet, waiting for teacher to share...');
          }
          
          // Initialize microphone for push-to-talk
          setTimeout(() => {
            initializeStudentMicrophone();
          }, 1000);
        } catch (consumeErr) {
          // Không có producer nào - teacher chưa share, không phải lỗi
          console.log('[Student] No producers yet or error consuming:', consumeErr);
          setError(consumeErr instanceof Error ? consumeErr.message : 'Failed to consume producers');
        }
      } else {
        // Teacher: consume existing student audio producers
        try {
          const stream = await client.consumeAll();
          if (stream && stream.getAudioTracks().length > 0) {
            setStudentAudioStream(stream);
            console.log('[Teacher] ✅ Student audio stream set');
          }
        } catch (err) {
          console.log('[Teacher] No student audio producers yet');
        }
      }
    } catch (err) {
      console.error('Connection error:', err);
      setError(err instanceof Error ? err.message : 'Connection failed');
    }
  }, []);

  const stopScreenShare = useCallback(() => {
    setLocalStream(prevStream => {
      if (prevStream) {
        prevStream.getTracks().forEach(track => track.stop());
      }
      return null;
    });
    clientRef.current?.stopProducing();
    setIsSharing(false);
  }, []);

  const startScreenShare = useCallback(async (withAudio: boolean = true) => {
    if (!clientRef.current) return;

    try {
      // Check MediaDevices support and log detailed info for debugging
      const support = checkMediaDevicesSupport();
      console.log('MediaDevices Support Check:', support);
      
      if (!support.available || !support.hasGetDisplayMedia) {
        const errorMsg = support.error || 
          'getDisplayMedia không khả dụng. Vui lòng kiểm tra:\n' +
          '- Tauri đang sử dụng WebView hỗ trợ MediaDevices API\n' +
          '- Cấu hình Tauri cho phép truy cập MediaDevices\n' +
          '- Ứng dụng có quyền truy cập screen recording (macOS)';
        
        console.error('MediaDevices Debug Info:', {
          support,
          navigatorType: typeof navigator,
          windowNavigator: typeof (window as any).navigator,
          mediaDevices: (typeof navigator !== 'undefined' ? navigator : (window as any).navigator)?.mediaDevices
        });
        
        throw new Error(errorMsg);
      }

      // Get navigator and mediaDevices
      const nav = typeof navigator !== 'undefined' ? navigator : (window as any).navigator;
      const mediaDevices = nav.mediaDevices;

      // Get screen with system audio - chất lượng siêu nét 4K 60fps
      const displayMediaOptions: any = {
        video: {
          width: { ideal: 1920, max: 1920 }, // 1080p Full HD
          height: { ideal: 1080, max: 1080 }, // 1080p Full HD
          frameRate: { ideal: 30, max: 60 }, // 30fps để tiết kiệm băng thông
        },
      };

      // Cải thiện audio capture cho system audio
      if (withAudio) {
        displayMediaOptions.audio = {
          echoCancellation: false, // Tắt echo cancellation cho system audio
          noiseSuppression: false, // Tắt noise suppression cho system audio
          autoGainControl: false, // Tắt auto gain control cho system audio
          suppressLocalAudioPlayback: false, // Không suppress local audio
          // Thử các constraints khác để capture system audio tốt hơn
          sampleRate: { ideal: 48000 },
          channelCount: { ideal: 2 }, // Stereo
        };
      }

      console.log('[ScreenShare] Requesting display media with options:', displayMediaOptions);
      const screenStream = await mediaDevices.getDisplayMedia(displayMediaOptions);

      // Log thông tin về tracks được capture
      const videoTracks = screenStream.getVideoTracks();
      const audioTracks = screenStream.getAudioTracks();
      console.log('[ScreenShare] Video tracks:', videoTracks.length);
      console.log('[ScreenShare] Audio tracks:', audioTracks.length);
      
      if (withAudio && audioTracks.length === 0) {
        console.warn('[ScreenShare] ⚠️ Audio track không được capture! Có thể do:');
        console.warn('  - macOS chưa cấp quyền Screen Recording');
        console.warn('  - Trình duyệt/WebView không hỗ trợ system audio capture');
        console.warn('  - Người dùng chưa chọn "Share system audio" trong dialog');
        
        // Hiển thị warning nhưng vẫn tiếp tục chia sẻ màn hình
        // Người dùng có thể dùng microphone riêng nếu cần
        const warningMsg = '⚠️ Âm thanh hệ thống không được capture.\n\n' +
          'Cách khắc phục trên macOS:\n' +
          '1. Khi hộp thoại chia sẻ màn hình xuất hiện, đảm bảo đã tích vào "Share system audio"\n' +
          '2. Kiểm tra System Settings > Privacy & Security > Screen Recording - đảm bảo ứng dụng có quyền\n' +
          '3. Nếu vẫn không được, bạn có thể dùng nút "Bật Microphone" để chia sẻ âm thanh từ microphone\n\n' +
          'Màn hình vẫn được chia sẻ bình thường.';
        
        setError(warningMsg);
        
        // Tự động clear error sau 10 giây để không làm phiền người dùng
        setTimeout(() => {
          setError(null);
        }, 10000);
      } else if (audioTracks.length > 0) {
        console.log('[ScreenShare] ✅ Audio track được capture:', {
          id: audioTracks[0].id,
          label: audioTracks[0].label,
          enabled: audioTracks[0].enabled,
          muted: audioTracks[0].muted,
          readyState: audioTracks[0].readyState,
          settings: audioTracks[0].getSettings(),
        });
      }

      setLocalStream(screenStream);
      await clientRef.current.produceScreen(screenStream);
      setIsSharing(true);

      // Handle stream end (user clicks "Stop sharing")
      const videoTrack = screenStream.getVideoTracks()[0];
      if (videoTrack) {
        videoTrack.onended = () => {
          stopScreenShare();
        };
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to share screen');
    }
  }, [stopScreenShare]);

  const startMicrophone = useCallback(async () => {
    if (!clientRef.current) return;

    try {
      // Check if navigator exists and mediaDevices API is available
      const nav = typeof navigator !== 'undefined' ? navigator : (window as any).navigator;
      if (!nav || !nav.mediaDevices || typeof nav.mediaDevices.getUserMedia !== 'function') {
        throw new Error('Microphone API không khả dụng. Vui lòng đảm bảo bạn đang sử dụng phiên bản trình duyệt/Tauri hỗ trợ getUserMedia.');
      }

      const mediaDevices = nav.mediaDevices;

      // Check permission state if available (not all browsers support this)
      try {
        // TypeScript may not recognize 'microphone' as PermissionName, so we use any
        const permissions = (mediaDevices as any).permissions;
        if (permissions && typeof permissions.query === 'function') {
          const permissionStatus = await permissions.query({ name: 'microphone' });
          console.log('[Microphone] Permission status:', permissionStatus.state);
          
          if (permissionStatus.state === 'denied') {
            throw new Error('Quyền truy cập microphone đã bị từ chối. Vui lòng cấp quyền trong System Settings > Privacy & Security > Microphone trên macOS.');
          }
        }
      } catch (permErr) {
        // Permission query không khả dụng, tiếp tục thử request
        // (This is normal - not all browsers/WebViews support permission query API)
        console.warn('[Microphone] Could not query permission (this is normal):', permErr);
      }

      // Request microphone access
      console.log('[Microphone] Requesting microphone access...');
      const micStream = await mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
          // Có thể thử constraints đơn giản hơn nếu bị từ chối
          sampleRate: { ideal: 48000 },
          channelCount: { ideal: 1 },
        },
        video: false,
      });

      console.log('[Microphone] ✅ Microphone access granted');
      
      // Store microphone stream
      setMicStream(micStream);
      setIsMicActive(true);
      
      // Produce microphone and track producer ID
      const producerId = await clientRef.current.produceMicrophone(micStream);
      if (producerId) {
        micProducerIdRef.current = producerId;
        console.log('[Microphone] Microphone producer ID:', producerId);
        
        // For students: disable track initially (push-to-talk mode)
        if (!clientRef.current.isTeacher) {
          clientRef.current.disableProducerTrack(producerId);
          console.log('[Microphone] Student microphone disabled (push-to-talk mode)');
        }
      }
    } catch (err) {
      console.error('[Microphone] ❌ Error accessing microphone:', err);
      
      let errorMessage = 'Không thể truy cập microphone.';
      
      if (err instanceof Error) {
        const errName = err.name || '';
        const errMsg = err.message || '';
        
        // Handle specific error cases
        if (errName === 'NotAllowedError' || errMsg.includes('not allowed') || errMsg.includes('permission denied') || errMsg.includes('denied')) {
          errorMessage = 'Quyền truy cập microphone bị từ chối. Vui lòng:\n' +
            '1. Kiểm tra System Settings > Privacy & Security > Microphone\n' +
            '2. Đảm bảo ứng dụng có quyền truy cập microphone\n' +
            '3. Thử lại sau khi cấp quyền';
        } else if (errName === 'NotFoundError' || errMsg.includes('not found')) {
          errorMessage = 'Không tìm thấy microphone. Vui lòng kiểm tra thiết bị microphone đã được kết nối chưa.';
        } else if (errName === 'NotReadableError' || errMsg.includes('not readable')) {
          errorMessage = 'Microphone đang được sử dụng bởi ứng dụng khác. Vui lòng đóng các ứng dụng khác đang sử dụng microphone.';
        } else {
          errorMessage = errMsg || 'Không thể truy cập microphone.';
        }
      }
      
      setError(errorMessage);
    }
  }, []);

  // Initialize microphone for students (for push-to-talk)
  const initializeStudentMicrophone = useCallback(async () => {
    if (!clientRef.current || clientRef.current.isTeacher) return;
    if (micStream || micProducerIdRef.current) return; // Already initialized

    try {
      const nav = typeof navigator !== 'undefined' ? navigator : (window as any).navigator;
      if (!nav || !nav.mediaDevices || typeof nav.mediaDevices.getUserMedia !== 'function') {
        return; // Silently fail - will show error when user tries to use push-to-talk
      }

      const mediaDevices = nav.mediaDevices;
      const newMicStream = await mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
          sampleRate: { ideal: 48000 },
          channelCount: { ideal: 1 },
        },
        video: false,
      });

      setMicStream(newMicStream);
      
      // Produce microphone but disable track initially
      const producerId = await clientRef.current.produceMicrophone(newMicStream);
      if (producerId) {
        micProducerIdRef.current = producerId;
        clientRef.current.disableProducerTrack(producerId);
        console.log('[Student] Microphone initialized for push-to-talk (disabled)');
      }
    } catch (err) {
      console.warn('[Student] Could not initialize microphone:', err);
      // Don't set error - user will see it when they try to use push-to-talk
    }
  }, []);

  const enablePushToTalk = useCallback(async () => {
    if (!clientRef.current) return;
    
    if (!micProducerIdRef.current) {
      // Try to initialize if not already done
      await initializeStudentMicrophone();
      // Wait a bit for producer to be created
      await new Promise(resolve => setTimeout(resolve, 200));
    }

    if (micProducerIdRef.current && clientRef.current) {
      clientRef.current.enableProducerTrack(micProducerIdRef.current);
      setIsPushToTalkActive(true);
      console.log('[PushToTalk] ✅ Enabled');
    } else {
      setError('Không thể kích hoạt microphone. Vui lòng kiểm tra quyền truy cập microphone.');
    }
  }, [initializeStudentMicrophone]);

  const disablePushToTalk = useCallback(() => {
    if (!clientRef.current || !micProducerIdRef.current) return;

    clientRef.current.disableProducerTrack(micProducerIdRef.current);
    setIsPushToTalkActive(false);
    console.log('[PushToTalk] ❌ Disabled');
  }, []);

  const stopMicrophone = useCallback(() => {
    // Stop microphone producer in mediasoup client first
    if (clientRef.current && micProducerIdRef.current) {
      clientRef.current.stopProducer(micProducerIdRef.current);
      micProducerIdRef.current = null;
    }
    
    if (micStream) {
      // Stop all audio tracks from microphone stream
      micStream.getAudioTracks().forEach(track => {
        track.stop();
        console.log('[Microphone] Stopped audio track');
      });
      setMicStream(null);
    }
    
    setIsMicActive(false);
    
    console.log('[Microphone] ✅ Microphone stopped');
  }, [micStream]);

  const disconnect = useCallback(() => {
    stopScreenShare();
    stopMicrophone();
    clientRef.current?.disconnect();
    clientRef.current = null;
    setConnectionState('disconnected');
    setPeers([]);
    setRemoteStream(null);
    setIsMicActive(false);
    setMicStream(null);
  }, [stopScreenShare, stopMicrophone]);

  const controlStudent = useCallback((studentId: string, action: string) => {
    console.log('[useMediasoup] controlStudent called with studentId:', studentId, 'action:', action);
    console.log('[useMediasoup] clientRef.current:', clientRef.current ? 'exists' : 'null');
    
    if (clientRef.current) {
      console.log('[useMediasoup] Calling sendControlCommand...');
      clientRef.current.sendControlCommand(studentId, action);
    } else {
      console.error('[useMediasoup] clientRef.current is null, cannot send control command');
      setError('Không thể gửi lệnh điều khiển: chưa kết nối');
    }
  }, []);

  const controlMouse = useCallback((studentId: string, event: any) => {
    console.log('[useMediasoup] controlMouse called with studentId:', studentId, 'event:', event);
    
    if (clientRef.current) {
      clientRef.current.sendMouseControl(studentId, event);
    } else {
      console.error('[useMediasoup] clientRef.current is null, cannot send mouse control');
      setError('Không thể gửi lệnh điều khiển chuột: chưa kết nối');
    }
  }, []);

  const controlKeyboard = useCallback((studentId: string, event: any) => {
    console.log('[useMediasoup] controlKeyboard called with studentId:', studentId, 'event:', event);
    
    if (clientRef.current) {
      clientRef.current.sendKeyboardControl(studentId, event);
    } else {
      console.error('[useMediasoup] clientRef.current is null, cannot send keyboard control');
      setError('Không thể gửi lệnh điều khiển bàn phím: chưa kết nối');
    }
  }, []);

  const requestStudentScreenShare = useCallback((studentId: string) => {
    console.log('[useMediasoup] requestStudentScreenShare called with studentId:', studentId);
    
    if (clientRef.current) {
      clientRef.current.requestStudentScreenShare(studentId);
    } else {
      console.error('[useMediasoup] clientRef.current is null, cannot request screen share');
      setError('Không thể yêu cầu chia sẻ màn hình: chưa kết nối');
    }
  }, []);
  
  // Store student screen sizes - will be set when students send them
  const [studentScreenSizes, setStudentScreenSizes] = useState<Map<string, { width: number; height: number }>>(new Map());

  return {
    connectionState,
    error,
    peers,
    remoteStream,
    localStream,
    isSharing,
    isMicActive,
    isPushToTalkActive,
    studentAudioStream,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopScreenShare,
    stopMicrophone,
    enablePushToTalk,
    disablePushToTalk,
    initializeStudentMicrophone,
    controlStudent,
    controlMouse,
    controlKeyboard,
    requestStudentScreenShare,
    studentVideoStreams,
    studentScreenSizes,
  };
  

  
}
