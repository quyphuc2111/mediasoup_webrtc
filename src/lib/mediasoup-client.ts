import { Device, types, detectDevice } from 'mediasoup-client';

type Transport = types.Transport;
type Producer = types.Producer;
type Consumer = types.Consumer;
type RtpCapabilities = types.RtpCapabilities;
type DtlsParameters = types.DtlsParameters;

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';
export type MediaKind = 'audio' | 'video';

export interface MediasoupClientEvents {
  onConnectionStateChange: (state: ConnectionState) => void;
  onNewProducer: (producerId: string, kind: MediaKind, peerId?: string) => void;
  onProducerClosed: (producerId: string) => void;
  onPeerJoined: (peerId: string, name: string, isTeacher: boolean) => void;
  onPeerLeft: (peerId: string, wasTeacher: boolean) => void;
  onError: (error: string) => void;
  onStreamReady: (stream: MediaStream) => void;
  onChatMessage: (message: { senderId: string; senderName: string; content: string; timestamp: string; isTeacher: boolean }) => void;
}

export class MediasoupClient {
  private ws: WebSocket | null = null;
  private device: Device | null = null;
  private sendTransport: Transport | null = null;
  private recvTransport: Transport | null = null;
  private producers: Map<string, Producer> = new Map();
  private consumers: Map<string, Consumer> = new Map();
  private events: Partial<MediasoupClientEvents> = {};
  private pendingRequests: Map<string, { resolve: (value: any) => void; reject: (reason: any) => void }> = new Map();

  public roomId: string = '';
  public peerId: string = '';
  public isTeacher: boolean = false;
  public rtpCapabilities: RtpCapabilities | null = null;

  constructor(events: Partial<MediasoupClientEvents>) {
    this.events = events;
  }

  async connect(serverUrl: string, roomId: string, peerId: string, name: string, isTeacher: boolean): Promise<void> {
    this.roomId = roomId;
    this.peerId = peerId;
    this.isTeacher = isTeacher;

    // Check device support first
    // Check device support first
    let handlerName = detectDevice();
    if (!handlerName && (window as any).RTCPeerConnection) {
      console.warn('Browser handler not detected, defaulting to Safari12 for Tauri/Mac...');
      handlerName = 'Safari12';
    }

    if (!handlerName) {
      throw new Error('Browser không hỗ trợ WebRTC. Vui lòng sử dụng Chrome, Firefox hoặc Edge.');
    }

    this.events.onConnectionStateChange?.('connecting');

    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(serverUrl);

      this.ws.onopen = async () => {
        try {
          // Join room - response contains rtpCapabilities
          const joinResponse = await this.sendRequest('join', { roomId, peerId, name, isTeacher });

          // Store rtpCapabilities from response
          this.rtpCapabilities = joinResponse.rtpCapabilities;

          if (!this.rtpCapabilities) {
            throw new Error('Server không trả về rtpCapabilities');
          }

          // Load device with rtpCapabilities
          // Load device with rtpCapabilities
          this.device = new Device({ handlerName });
          await this.device.load({ routerRtpCapabilities: this.rtpCapabilities });

          this.events.onConnectionStateChange?.('connected');
          resolve();
        } catch (error) {
          this.events.onError?.(error instanceof Error ? error.message : 'Kết nối thất bại');
          reject(error);
        }
      };

      this.ws.onmessage = (event) => {
        const message = JSON.parse(event.data);
        this.handleMessage(message);
      };

      this.ws.onerror = () => {
        this.events.onConnectionStateChange?.('error');
        this.events.onError?.('Lỗi kết nối WebSocket');
        reject(new Error('WebSocket error'));
      };

      this.ws.onclose = () => {
        this.events.onConnectionStateChange?.('disconnected');
        this.cleanup();
      };
    });
  }

  private handleMessage(message: { type: string; data?: any }): void {
    const { type, data } = message;

    // Handle responses to requests
    if (this.pendingRequests.has(type)) {
      const { resolve } = this.pendingRequests.get(type)!;
      this.pendingRequests.delete(type);
      resolve(data);
      return;
    }

    // Handle server-pushed events
    switch (type) {
      case 'newProducer':
        this.events.onNewProducer?.(data.producerId, data.kind, data.peerId);
        break;
      case 'peerJoined':
        this.events.onPeerJoined?.(data.peerId, data.name, data.isTeacher);
        break;
      case 'peerLeft':
        this.events.onPeerLeft?.(data.peerId, data.wasTeacher);
        if (data.wasTeacher) {
          this.closeAllConsumers();
        }
        break;
      case 'chatMessage':
        this.events.onChatMessage?.(data);
        break;
      case 'error':
        this.events.onError?.(data.message);
        break;
    }
  }

  private sendRequest(type: string, data?: any): Promise<any> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket chưa kết nối'));
        return;
      }

      const responseType = this.getResponseType(type);
      this.pendingRequests.set(responseType, { resolve, reject });

      this.ws.send(JSON.stringify({ type, data }));

      // Timeout 15s
      setTimeout(() => {
        if (this.pendingRequests.has(responseType)) {
          this.pendingRequests.delete(responseType);
          reject(new Error(`Request ${type} timeout`));
        }
      }, 15000);
    });
  }

  private getResponseType(requestType: string): string {
    const mapping: Record<string, string> = {
      join: 'joined',
      createTransport: 'transportCreated',
      connectTransport: 'transportConnected',
      produce: 'produced',
      consume: 'consumed',
      resumeConsumer: 'consumerResumed',
      getProducers: 'producers',
    };
    return mapping[requestType] || requestType;
  }

  async createSendTransport(): Promise<void> {
    if (!this.device) throw new Error('Device chưa được khởi tạo');

    const params = await this.sendRequest('createTransport', { direction: 'send' });

    this.sendTransport = this.device.createSendTransport({
      id: params.id,
      iceParameters: params.iceParameters,
      iceCandidates: params.iceCandidates,
      dtlsParameters: params.dtlsParameters,
    });

    this.sendTransport.on('connect', async ({ dtlsParameters }: { dtlsParameters: DtlsParameters }, callback: () => void, errback: (error: Error) => void) => {
      try {
        await this.sendRequest('connectTransport', { direction: 'send', dtlsParameters });
        callback();
      } catch (error) {
        errback(error as Error);
      }
    });

    this.sendTransport.on('produce', async ({ kind, rtpParameters }: { kind: string; rtpParameters: any }, callback: (params: { id: string }) => void, errback: (error: Error) => void) => {
      try {
        const { producerId } = await this.sendRequest('produce', { kind, rtpParameters });
        callback({ id: producerId });
      } catch (error) {
        errback(error as Error);
      }
    });
  }

  async createRecvTransport(): Promise<void> {
    if (!this.device) throw new Error('Device chưa được khởi tạo');

    const params = await this.sendRequest('createTransport', { direction: 'recv' });

    this.recvTransport = this.device.createRecvTransport({
      id: params.id,
      iceParameters: params.iceParameters,
      iceCandidates: params.iceCandidates,
      dtlsParameters: params.dtlsParameters,
    });

    this.recvTransport.on('connect', async ({ dtlsParameters }: { dtlsParameters: DtlsParameters }, callback: () => void, errback: (error: Error) => void) => {
      try {
        await this.sendRequest('connectTransport', { direction: 'recv', dtlsParameters });
        callback();
      } catch (error) {
        errback(error as Error);
      }
    });
  }

  async produceScreen(stream: MediaStream): Promise<{ videoProducerId: string | null; audioProducerId: string | null }> {
    if (!this.sendTransport) {
      await this.createSendTransport();
    }

    let videoProducerId: string | null = null;
    let audioProducerId: string | null = null;

    // Produce video track với chất lượng siêu nét
    const videoTrack = stream.getVideoTracks()[0];
    if (videoTrack) {
      console.log('[MediasoupClient] Producing video track:', {
        id: videoTrack.id,
        label: videoTrack.label,
        enabled: videoTrack.enabled,
        settings: videoTrack.getSettings(),
      });
      
      const producer = await this.sendTransport!.produce({
        track: videoTrack,
        encodings: [
          {
            maxBitrate: 5000000, // Max 5Mbps cho LAN
            maxFramerate: 30
          },
        ],
        codecOptions: {
          videoGoogleStartBitrate: 3000, // 3Mbps start
          videoGoogleMinBitrate: 1000, // 1Mbps min
          videoGoogleMaxBitrate: 5000, // 5Mbps max
        },
      });
      this.producers.set(producer.id, producer);
      videoProducerId = producer.id;
      console.log('[MediasoupClient] ✅ Video producer created:', producer.id);
    } else {
      console.warn('[MediasoupClient] ⚠️ No video track found in stream');
    }

    // Produce audio track (system audio)
    const audioTracks = stream.getAudioTracks();
    console.log('[MediasoupClient] Audio tracks in stream:', audioTracks.length);
    
    if (audioTracks.length > 0) {
      const audioTrack = audioTracks[0];
      console.log('[MediasoupClient] Producing audio track:', {
        id: audioTrack.id,
        label: audioTrack.label,
        enabled: audioTrack.enabled,
        muted: audioTrack.muted,
        readyState: audioTrack.readyState,
        settings: audioTrack.getSettings(),
      });

      // Kiểm tra xem track có đang active không
      if (audioTrack.readyState === 'ended') {
        console.error('[MediasoupClient] ❌ Audio track đã bị ended, không thể produce');
        return { videoProducerId, audioProducerId: null };
      }

      // Đảm bảo track được enable
      if (!audioTrack.enabled) {
        console.warn('[MediasoupClient] ⚠️ Audio track bị disabled, đang enable...');
        audioTrack.enabled = true;
      }

      try {
        const producer = await this.sendTransport!.produce({
          track: audioTrack,
          // Thêm codec options cho audio nếu cần
          codecOptions: {
            opusStereo: true,
            opusFec: true,
            opusDtx: true,
            opusMaxPlaybackRate: 48000,
          },
        });
        this.producers.set(producer.id, producer);
        audioProducerId = producer.id;
        console.log('[MediasoupClient] ✅ Audio producer created:', producer.id, 'kind:', producer.kind);
      } catch (error) {
        console.error('[MediasoupClient] ❌ Failed to produce audio track:', error);
        throw error;
      }
    } else {
      console.warn('[MediasoupClient] ⚠️ No audio track found in stream. System audio có thể chưa được capture.');
      console.warn('[MediasoupClient] Hướng dẫn: Đảm bảo đã chọn "Share system audio" trong hộp thoại chia sẻ màn hình.');
    }

    return { videoProducerId, audioProducerId };
  }

  async produceMicrophone(stream: MediaStream): Promise<string | null> {
    if (!this.sendTransport) {
      await this.createSendTransport();
    }

    const audioTrack = stream.getAudioTracks()[0];
    if (audioTrack) {
      const producer = await this.sendTransport!.produce({
        track: audioTrack,
      });
      this.producers.set(producer.id, producer);
      return producer.id;
    }
    return null;
  }

  async consumeAll(): Promise<MediaStream> {
    if (!this.recvTransport) {
      await this.createRecvTransport();
    }

    console.log('[MediasoupClient] Requesting producers list...');
    const producers = await this.sendRequest('getProducers', {});
    const stream = new MediaStream();

    console.log('[MediasoupClient] Received producers:', producers);

    if (Array.isArray(producers) && producers.length > 0) {
      console.log(`[MediasoupClient] Consuming ${producers.length} producers...`);
      for (const producerInfo of producers) {
        const producerId = producerInfo.producerId || producerInfo;
        const peerId = producerInfo.peerId;
        console.log(`[MediasoupClient] Consuming producer: ${producerId}${peerId ? ` from peer ${peerId}` : ''}`);
        try {
          const consumer = await this.consume(producerId);
          if (consumer) {
            stream.addTrack(consumer.track);
            console.log(`[MediasoupClient] ✅ Added track from producer ${producerId}, kind: ${consumer.track.kind}`);
          } else {
            console.warn(`[MediasoupClient] ⚠️ Failed to consume producer ${producerId}`);
          }
        } catch (error) {
          console.error(`[MediasoupClient] ❌ Error consuming producer ${producerId}:`, error);
        }
      }
    } else {
      console.log('[MediasoupClient] No producers available to consume');
    }

    console.log(`[MediasoupClient] Stream ready with ${stream.getTracks().length} tracks`);
    this.events.onStreamReady?.(stream);
    return stream;
  }

  async consume(producerId: string): Promise<Consumer | null> {
    // Ensure recvTransport exists before consuming
    if (!this.recvTransport) {
      console.log('[MediasoupClient] Creating recvTransport for consume...');
      await this.createRecvTransport();
    }

    if (!this.device) {
      console.error('[MediasoupClient] Device not initialized, cannot consume');
      return null;
    }

    if (!this.recvTransport) {
      console.error('[MediasoupClient] recvTransport still not available after creation');
      return null;
    }

    try {
      console.log(`[MediasoupClient] Consuming producer ${producerId}...`);
      const params = await this.sendRequest('consume', {
        producerId,
        rtpCapabilities: this.device.rtpCapabilities,
      });

      const consumer = await this.recvTransport.consume({
        id: params.consumerId,
        producerId: params.producerId,
        kind: params.kind,
        rtpParameters: params.rtpParameters,
      });

      this.consumers.set(consumer.id, consumer);

      // Resume consumer
      await this.sendRequest('resumeConsumer', { consumerId: consumer.id });

      return consumer;
    } catch (error) {
      console.error('Failed to consume:', error);
      return null;
    }
  }

  stopProducing(): void {
    for (const producer of this.producers.values()) {
      producer.close();
    }
    this.producers.clear();
  }

  stopAudioProducers(): void {
    const audioProducers: Producer[] = [];
    for (const producer of this.producers.values()) {
      if (producer.kind === 'audio') {
        audioProducers.push(producer);
      }
    }
    
    for (const producer of audioProducers) {
      producer.close();
      this.producers.delete(producer.id);
      console.log('[MediasoupClient] Stopped audio producer:', producer.id);
    }
  }

  stopProducer(producerId: string): void {
    const producer = this.producers.get(producerId);
    if (producer) {
      producer.close();
      this.producers.delete(producerId);
      console.log('[MediasoupClient] Stopped producer:', producerId);
    } else {
      console.warn('[MediasoupClient] Producer not found:', producerId);
    }
  }

  enableProducerTrack(producerId: string): void {
    const producer = this.producers.get(producerId);
    if (producer && producer.track) {
      producer.track.enabled = true;
      console.log('[MediasoupClient] Enabled producer track:', producerId);
    } else {
      console.warn('[MediasoupClient] Producer not found or has no track:', producerId);
    }
  }

  disableProducerTrack(producerId: string): void {
    const producer = this.producers.get(producerId);
    if (producer && producer.track) {
      producer.track.enabled = false;
      console.log('[MediasoupClient] Disabled producer track:', producerId);
    } else {
      console.warn('[MediasoupClient] Producer not found or has no track:', producerId);
    }
  }

  getScreenAudioProducerId(): string | null {
    // Find audio producer from screen share (kind === 'audio')
    for (const [producerId, producer] of this.producers.entries()) {
      if (producer.kind === 'audio') {
        return producerId;
      }
    }
    return null;
  }

  private closeAllConsumers(): void {
    for (const consumer of this.consumers.values()) {
      consumer.close();
    }
    this.consumers.clear();
  }

  private cleanup(): void {
    this.stopProducing();
    this.closeAllConsumers();
    this.sendTransport?.close();
    this.recvTransport?.close();
    this.sendTransport = null;
    this.recvTransport = null;
    this.device = null;
  }

  disconnect(): void {
    this.cleanup();
    this.ws?.close();
    this.ws = null;
  }

  sendChatMessage(content: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('[MediasoupClient] Cannot send chat message: WebSocket not connected');
      return;
    }

    this.ws.send(JSON.stringify({
      type: 'chatMessage',
      data: {
        content,
        timestamp: new Date().toISOString(),
      }
    }));
  }
}
