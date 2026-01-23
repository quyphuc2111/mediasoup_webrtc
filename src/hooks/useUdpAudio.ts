import { useState, useCallback, useRef, useEffect } from 'react';

interface DiscoveredDevice {
  ip: string;
  name: string;
  port: number;
  last_seen: number;
}

interface SavedDevice {
  id?: number;
  ip: string;
  name: string;
  port: number;
  last_used: number;
}

export function useUdpAudio() {
  const [isServerRunning, setIsServerRunning] = useState(false);
  const [isClientConnected, setIsClientConnected] = useState(false);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const [savedDevices, setSavedDevices] = useState<SavedDevice[]>([]);
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [serverPort, setServerPort] = useState(5000);
  const [clientIp, setClientIp] = useState('');
  const [clientPort, setClientPort] = useState(5000);
  
  const audioContextRef = useRef<AudioContext | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const intervalRef = useRef<number | null>(null);

  // Initialize database
  useEffect(() => {
    const initDb = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('init_db');
        await loadSavedDevices();
      } catch (err) {
        console.error('Failed to init database:', err);
      }
    };
    initDb();
  }, []);

  const loadSavedDevices = useCallback(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const devices = await invoke<SavedDevice[]>('get_saved_devices');
      setSavedDevices(devices);
    } catch (err) {
      console.error('Failed to load saved devices:', err);
    }
  }, []);

  const startUdpAudioServer = useCallback(async (port: number = 5000) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('start_udp_audio_server', { port });
      setServerPort(port);
      setIsServerRunning(true);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start UDP server');
    }
  }, []);

  const stopUdpAudioServer = useCallback(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('stop_udp_audio_server');
      setIsServerRunning(false);
      
      if (mediaStreamRef.current) {
        mediaStreamRef.current.getTracks().forEach(track => track.stop());
        mediaStreamRef.current = null;
      }
      
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to stop UDP server');
    }
  }, []);

  const startCapturingAudio = useCallback(async () => {
    try {
      const nav = typeof navigator !== 'undefined' ? navigator : (window as any).navigator;
      if (!nav?.mediaDevices?.getUserMedia) {
        throw new Error('getUserMedia not available');
      }

      const stream = await nav.mediaDevices.getUserMedia({
        audio: {
          sampleRate: 48000,
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      });

      mediaStreamRef.current = stream;

      // Create AudioContext for processing
      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)({
        sampleRate: 48000,
      });
      audioContextRef.current = audioContext;

      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);

      processor.onaudioprocess = async (e) => {
        const inputData = e.inputBuffer.getChannelData(0);
        const audioData = new Int16Array(inputData.length);
        
        for (let i = 0; i < inputData.length; i++) {
          audioData[i] = Math.max(-32768, Math.min(32767, inputData[i] * 32768));
        }

        // Send audio data via UDP
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('send_udp_audio', {
            ip: '255.255.255.255', // Broadcast
            port: serverPort,
            audioData: Array.from(audioData),
          });
        } catch (err) {
          console.error('Failed to send audio:', err);
        }
      };

      source.connect(processor);
      processor.connect(audioContext.destination);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to capture audio');
    }
  }, [serverPort]);

  const discoverDevices = useCallback(async (port: number = 5000, timeout: number = 3000) => {
    setIsDiscovering(true);
    setError(null);
    
    console.log(`[UDP Audio] Starting device discovery on port ${port} with timeout ${timeout}ms`);
    
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const devices = await invoke<DiscoveredDevice[]>('discover_lan_devices', {
        port,
        timeoutMs: timeout,
      });
      
      console.log(`[UDP Audio] Discovery completed. Found ${devices.length} device(s):`, devices);
      
      setDiscoveredDevices(devices);
      
      // Auto-save discovered devices
      for (const device of devices) {
        try {
          await invoke('save_device_to_db', {
            ip: device.ip,
            name: device.name,
            port: device.port,
          });
          console.log(`[UDP Audio] Saved device to database: ${device.name} (${device.ip})`);
        } catch (err) {
          console.warn('[UDP Audio] Failed to save device:', err);
        }
      }
      
      await loadSavedDevices();
      
      if (devices.length === 0) {
        setError('Không tìm thấy thiết bị nào. Đảm bảo:\n- Học sinh đã chọn chế độ UDP Streaming\n- Tất cả thiết bị trong cùng mạng LAN\n- Firewall không chặn UDP port');
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to discover devices';
      console.error('[UDP Audio] Discovery error:', err);
      setError(errorMsg);
    } finally {
      setIsDiscovering(false);
    }
  }, [loadSavedDevices]);

  const connectToDevice = useCallback(async (ip: string, port: number) => {
    setClientIp(ip);
    setClientPort(port);
    setIsClientConnected(true);
    
    // Update last used
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_device_to_db', { ip, name: `Device ${ip}`, port });
      await loadSavedDevices();
    } catch (err) {
      console.warn('Failed to update device:', err);
    }
  }, [loadSavedDevices]);

  const disconnectFromDevice = useCallback(() => {
    setIsClientConnected(false);
    setClientIp('');
    setClientPort(5000);
  }, []);

  const removeDevice = useCallback(async (id: number) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('remove_device_from_db', { id });
      await loadSavedDevices();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to remove device');
    }
  }, [loadSavedDevices]);

  const startDiscoveryListener = useCallback(async (name: string, port: number = 5000) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('start_discovery_listener', { name, port });
      console.log(`[UDP Audio] Discovery listener started for ${name} on port ${port}`);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to start discovery listener';
      console.error('[UDP Audio] Failed to start discovery listener:', err);
      setError(errorMsg);
    }
  }, []);

  const stopDiscoveryListener = useCallback(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('stop_discovery_listener');
      console.log('[UDP Audio] Discovery listener stopped');
    } catch (err) {
      console.error('[UDP Audio] Failed to stop discovery listener:', err);
    }
  }, []);

  return {
    isServerRunning,
    isClientConnected,
    discoveredDevices,
    savedDevices,
    isDiscovering,
    error,
    serverPort,
    clientIp,
    clientPort,
    startUdpAudioServer,
    stopUdpAudioServer,
    startCapturingAudio,
    discoverDevices,
    connectToDevice,
    disconnectFromDevice,
    removeDevice,
    startDiscoveryListener,
    stopDiscoveryListener,
    loadSavedDevices,
  };
}
