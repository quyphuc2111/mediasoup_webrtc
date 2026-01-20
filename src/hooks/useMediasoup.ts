import { useState, useCallback, useRef, useEffect } from 'react';
import { MediasoupClient, ConnectionState, MediaKind } from '../lib/mediasoup-client';

interface Peer {
  id: string;
  name: string;
  isTeacher: boolean;
}

export function useMediasoup() {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [remoteStream, setRemoteStream] = useState<MediaStream | null>(null);
  const [isSharing, setIsSharing] = useState(false);
  const [localStream, setLocalStream] = useState<MediaStream | null>(null);
  
  const clientRef = useRef<MediasoupClient | null>(null);

  useEffect(() => {
    return () => {
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
      onNewProducer: async (producerId: string, _kind: MediaKind) => {
        // Auto-consume new producers (for students)
        if (!client.isTeacher) {
          const consumer = await client.consume(producerId);
          if (consumer) {
            setRemoteStream(prev => {
              const stream = prev || new MediaStream();
              stream.addTrack(consumer.track);
              return stream;
            });
          }
        }
      },
      onStreamReady: setRemoteStream,
    });

    clientRef.current = client;

    try {
      await client.connect(serverUrl, roomId, peerId, name, isTeacher);
      
      // Students: create recv transport and consume existing producers
      if (!isTeacher) {
        try {
          await client.consumeAll();
        } catch (consumeErr) {
          // Không có producer nào - teacher chưa share, không phải lỗi
          console.log('No producers yet, waiting for teacher to share...');
        }
      }
    } catch (err) {
      console.error('Connection error:', err);
      setError(err instanceof Error ? err.message : 'Connection failed');
    }
  }, []);

  const startScreenShare = useCallback(async (withAudio: boolean = true) => {
    if (!clientRef.current) return;

    try {
      // Get screen with system audio - chất lượng siêu nét 4K 60fps
      const screenStream = await navigator.mediaDevices.getDisplayMedia({
        video: {
          width: { ideal: 3840, max: 3840 }, // 4K UHD
          height: { ideal: 2160, max: 2160 }, // 4K UHD
          frameRate: { ideal: 60, max: 60 }, // 60fps cho mượt mà
        },
        audio: withAudio,
      });

      setLocalStream(screenStream);
      await clientRef.current.produceScreen(screenStream);
      setIsSharing(true);

      // Handle stream end (user clicks "Stop sharing")
      screenStream.getVideoTracks()[0].onended = () => {
        stopScreenShare();
      };
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to share screen');
    }
  }, []);

  const startMicrophone = useCallback(async () => {
    if (!clientRef.current) return;

    try {
      const micStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      });

      await clientRef.current.produceMicrophone(micStream);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to access microphone');
    }
  }, []);

  const stopScreenShare = useCallback(() => {
    if (localStream) {
      localStream.getTracks().forEach(track => track.stop());
      setLocalStream(null);
    }
    clientRef.current?.stopProducing();
    setIsSharing(false);
  }, [localStream]);

  const disconnect = useCallback(() => {
    stopScreenShare();
    clientRef.current?.disconnect();
    clientRef.current = null;
    setConnectionState('disconnected');
    setPeers([]);
    setRemoteStream(null);
  }, [stopScreenShare]);

  return {
    connectionState,
    error,
    peers,
    remoteStream,
    localStream,
    isSharing,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopScreenShare,
  };
}
