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
  onNewProducer: (producerId: string, kind: MediaKind) => void;
  onProducerClosed: (producerId: string) => void;
  onPeerJoined: (peerId: string, name: string, isTeacher: boolean) => void;
  onPeerLeft: (peerId: string, wasTeacher: boolean) => void;
  onError: (error: string) => void;
  onStreamReady: (stream: MediaStream) => void;
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
        this.events.onNewProducer?.(data.producerId, data.kind);
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

  async produceScreen(stream: MediaStream): Promise<void> {
    if (!this.sendTransport) {
      await this.createSendTransport();
    }

    // 🔒 LOCK bitrate và framerate ở CLIENT (QUAN TRỌNG CHO WINDOWS)
    // Mediasoup Producer KHÔNG có API để set bitrate/framerate ở server
    // Tất cả encoding parameters phải được set ở đây khi gọi transport.produce()
    //
    // Windows encoder optimization:
    // - 25fps mượt hơn 30fps (Chrome trên Windows)
    // - 4.5Mbps max tránh encoder burst (NVENC/QSV ổn định hơn)
    // - Bitrate range: 1.5-4Mbps để tránh oscillation
    const videoTrack = stream.getVideoTracks()[0];
    if (videoTrack) {
      const producer = await this.sendTransport!.produce({
        track: videoTrack,
        encodings: [
          {
            maxBitrate: 4500000, // 4.5Mbps max (tối ưu Windows - tránh encoder burst)
            maxFramerate: 25, // 25fps ideal cho Windows (mượt hơn 30fps)
          },
        ],
        codecOptions: {
          videoGoogleStartBitrate: 2500, // 2.5Mbps start (match với config server)
          videoGoogleMinBitrate: 1500, // 1.5Mbps min
          videoGoogleMaxBitrate: 4000, // 4Mbps max (giảm từ 5Mbps)
        },
      });
      this.producers.set(producer.id, producer);
      console.log(`[MediasoupClient] Video producer created with encoding: 4.5Mbps @ 25fps (Windows optimized)`);
    }

    // Produce audio track (system audio)
    const audioTrack = stream.getAudioTracks()[0];
    if (audioTrack) {
      const producer = await this.sendTransport!.produce({
        track: audioTrack,
      });
      this.producers.set(producer.id, producer);
    }
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
      for (const { producerId } of producers) {
        console.log(`[MediasoupClient] Consuming producer: ${producerId}`);
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

    // Ensure device has rtpCapabilities loaded
    if (!this.device.rtpCapabilities) {
      console.error('[MediasoupClient] Device rtpCapabilities not loaded, cannot consume');
      return null;
    }

    try {
      console.log(`[MediasoupClient] Consuming producer ${producerId}...`);
      console.log(`[MediasoupClient] Sending rtpCapabilities:`, {
        codecs: this.device.rtpCapabilities.codecs?.length || 0,
        headerExtensions: this.device.rtpCapabilities.headerExtensions?.length || 0,
      });

      // 1. Gửi request lên server để lấy consumer params
      const params = await this.sendRequest('consume', {
        producerId,
        rtpCapabilities: this.device.rtpCapabilities, // Cần thiết để server biết client hỗ trợ gì
      });

      console.log(`[MediasoupClient] Received consumer params:`, {
        consumerId: params.consumerId,
        producerId: params.producerId,
        kind: params.kind,
      });

      // 2. Tạo consumer phía client
      const consumer = await this.recvTransport.consume({
        id: params.consumerId,
        producerId: params.producerId,
        kind: params.kind,
        rtpParameters: params.rtpParameters,
      });

      this.consumers.set(consumer.id, consumer);
      console.log(`[MediasoupClient] Consumer created: ${consumer.id}, kind: ${consumer.kind}, paused: ${consumer.paused}`);

      // 3. THÔNG BÁO CHO SERVER RESUME
      // Rất quan trọng vì server đang ở trạng thái paused để đợi client sẵn sàng
      await this.sendRequest('resumeConsumer', { consumerId: consumer.id });
      console.log(`[MediasoupClient] ✅ Consumer ${consumer.id} resumed`);

      // 4. Track sẵn sàng để play
      console.log(`[MediasoupClient] Track ready:`, {
        id: consumer.track.id,
        kind: consumer.track.kind,
        enabled: consumer.track.enabled,
        readyState: consumer.track.readyState,
      });

      return consumer;
    } catch (error) {
      console.error(`[MediasoupClient] ❌ Failed to consume producer ${producerId}:`, error);
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
}
