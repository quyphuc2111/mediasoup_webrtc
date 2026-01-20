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
    const handlerName = detectDevice();
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
          this.device = new Device();
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

    // Produce video track với chất lượng siêu nét
    const videoTrack = stream.getVideoTracks()[0];
    if (videoTrack) {
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

  async produceMicrophone(stream: MediaStream): Promise<void> {
    if (!this.sendTransport) {
      await this.createSendTransport();
    }

    const audioTrack = stream.getAudioTracks()[0];
    if (audioTrack) {
      const producer = await this.sendTransport!.produce({
        track: audioTrack,
      });
      this.producers.set(producer.id, producer);
    }
  }

  async consumeAll(): Promise<MediaStream> {
    if (!this.recvTransport) {
      await this.createRecvTransport();
    }

    const producers = await this.sendRequest('getProducers', {});
    const stream = new MediaStream();

    if (Array.isArray(producers)) {
      for (const { producerId } of producers) {
        const consumer = await this.consume(producerId);
        if (consumer) {
          stream.addTrack(consumer.track);
        }
      }
    }

    this.events.onStreamReady?.(stream);
    return stream;
  }

  async consume(producerId: string): Promise<Consumer | null> {
    if (!this.recvTransport || !this.device) return null;

    try {
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
