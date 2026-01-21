import * as mediasoup from 'mediasoup';
import type {
  Worker,
  Router,
  WebRtcTransport,
  Producer,
  Consumer,
  RtpCapabilities,
  DtlsParameters,
  MediaKind,
  RtpParameters,
} from 'mediasoup/node/lib/types.js';
import { config, getLocalIp } from './config.bk.js';
import { Room } from './Room.js';

export class MediasoupManager {
  private workers: Worker[] = [];
  private rooms: Map<string, Room> = new Map();
  private nextWorkerIndex = 0;

  async init(): Promise<void> {
    console.log(`Creating ${config.numWorkers} mediasoup workers...`);

    for (let i = 0; i < config.numWorkers; i++) {
      const worker = await mediasoup.createWorker(config.worker);

      worker.on('died', () => {
        console.error(`Worker ${i} died, exiting...`);
        process.exit(1);
      });

      this.workers.push(worker);
      console.log(`Worker ${i} created [pid: ${worker.pid}]`);
    }
  }

  private getNextWorker(): Worker {
    const worker = this.workers[this.nextWorkerIndex];
    this.nextWorkerIndex = (this.nextWorkerIndex + 1) % this.workers.length;
    return worker;
  }

  async createRoom(roomId?: string): Promise<Room> {
    const worker = this.getNextWorker();
    const router = await worker.createRouter(config.router);
    
    // Note: Router observer doesn't have 'newProducer' event
    // We handle producer configuration in createProducer() instead
    
    const room = new Room(router, roomId);
    this.rooms.set(room.id, room);
    console.log(`Room created: ${room.id}`);
    return room;
  }

  getRoom(roomId: string): Room | undefined {
    return this.rooms.get(roomId);
  }

  async getOrCreateRoom(roomId: string): Promise<Room> {
    let room = this.rooms.get(roomId);
    if (!room) {
      room = await this.createRoom(roomId);
    }
    return room;
  }

  removeRoom(roomId: string): void {
    const room = this.rooms.get(roomId);
    if (room) {
      room.close();
      this.rooms.delete(roomId);
    }
  }

  async createWebRtcTransport(room: Room): Promise<{
    transport: WebRtcTransport;
    params: {
      id: string;
      iceParameters: any;
      iceCandidates: any;
      dtlsParameters: any;
    };
  }> {
    const localIp = getLocalIp();

    const transportOptions = {
      listenInfos: [
        {
          protocol: 'udp' as const,
          ip: '0.0.0.0',
          announcedAddress: localIp,
        },
        // ❗ LAN: TẮT TCP để tránh fallback gây jitter trên Windows
        // Chỉ dùng UDP cho LAN để tránh TCP fallback oscillation
      ],
      enableUdp: true,
      preferUdp: true,
      enableTcp: false, // ❗ LAN: TẮT TCP để tránh fallback
      initialAvailableOutgoingBitrate: config.webRtcTransport.initialAvailableOutgoingBitrate,
    };

    const transport = await room.router.createWebRtcTransport(transportOptions);

    // Tối ưu: giới hạn bitrate cho mỗi transport
    await transport.setMaxIncomingBitrate(config.maxIncomingBitrate);

    return {
      transport,
      params: {
        id: transport.id,
        iceParameters: transport.iceParameters,
        iceCandidates: transport.iceCandidates,
        dtlsParameters: transport.dtlsParameters,
      },
    };
  }

  async connectTransport(
    transport: WebRtcTransport,
    dtlsParameters: DtlsParameters
  ): Promise<void> {
    await transport.connect({ dtlsParameters });
  }

  async createProducer(
    transport: WebRtcTransport,
    kind: MediaKind,
    rtpParameters: RtpParameters
  ): Promise<Producer> {
    const producer = await transport.produce({
      kind,
      rtpParameters,
      appData: {
        // dùng để debug nếu cần
        source: kind === 'video' ? 'screen' : 'microphone',
      },
      
      // encodings: [
      //   {
      //     maxBitrate: 12_000_000,
      //     scalabilityMode: 'L1T3', // temporal only
      //   },
      // ],
      // codecOptions: {
      //   videoGoogleStartBitrate: 6000,
      // },
    });

    // 🔒 LOCK encoder behavior (CỰC KỲ QUAN TRỌNG CHO WINDOWS)
    // Screen share: L1T3 (1 spatial, 3 temporal layers) - KHÔNG simulcast
    // Đảm bảo chỉ dùng temporal scalability, không cho WebRTC tự adapt spatial layers
    if (kind === 'video') {
      try {
        // Set max spatial layer to 0 để disable simulcast
        // Screen share dùng L1T3 (temporal only), không cần multiple spatial layers
        if ('setMaxSpatialLayer' in producer && typeof producer.setMaxSpatialLayer === 'function') {
          await (producer as any).setMaxSpatialLayer(0);
          console.log(`Producer ${producer.id}: Locked to spatial layer 0 (no simulcast)`);
        }

        // Note: Encodings (L1T3, bitrate) được set ở client trong transport.produce()
        // Server chỉ cần đảm bảo không có spatial layer switching
        // Temporal layers (L1T3) cho phép framerate adaptation tự nhiên
        console.log(`Producer ${producer.id}: Created with encodings from client (L1T3 for screen share)`);
      } catch (error) {
        console.warn(`Failed to lock producer encoding parameters:`, error);
      }
    }

    producer.on('transportclose', () => {
      console.log(`Producer ${producer.id} transport closed`);
    });

    return producer;
  }

  async createConsumer(
    room: Room,
    transport: WebRtcTransport,
    producer: Producer,
    rtpCapabilities: RtpCapabilities
  ): Promise<Consumer | null> {
    if (!room.router.canConsume({ producerId: producer.id, rtpCapabilities })) {
      console.warn('Cannot consume producer', producer.id);
      return null;
    }

    const consumer = await transport.consume({
      producerId: producer.id,
      rtpCapabilities,
      paused: true, // Start paused, resume after client ready
    });

    // 🔒 Lock consumer bitrate và layer (LAN only)
    // Ngăn WebRTC "thông minh quá mức", tránh oscillation bitrate (căn nguyên jitter)
    try {
      // Set preferred layers first
      if (consumer.type !== 'simple') {
        await consumer.setPreferredLayers({ spatialLayer: 0, temporalLayer: 0 });
      }
      
      // Lock max spatial layer
      if ('setMaxSpatialLayer' in consumer && typeof consumer.setMaxSpatialLayer === 'function') {
        await (consumer as any).setMaxSpatialLayer(0);
      }

      // Giới hạn bitrate downstream để tránh oscillation
      // Windows receiver không thể request bitrate thấp hơn → frame spacing đều
      if ('setRtpEncodingParameters' in consumer && typeof consumer.setRtpEncodingParameters === 'function') {
        await (consumer as any).setRtpEncodingParameters([
          {
            maxBitrate: 6_000_000,
            minBitrate: 3_000_000,
            priority: 'high',
          },
        ]);
        console.log(`Consumer ${consumer.id}: Locked bitrate (6Mbps) and layers`);
      } else {
        console.log(`Consumer ${consumer.id}: Created (bitrate limits set via transport)`);
      }
    } catch (error) {
      console.warn('Set consumer encoding parameters failed:', error);
    }

    consumer.on('transportclose', () => {
      console.log(`Consumer ${consumer.id} transport closed`);
    });

    consumer.on('producerclose', () => {
      console.log(`Consumer ${consumer.id} producer closed`);
    });

    return consumer;
  }

  close(): void {
    for (const room of this.rooms.values()) {
      room.close();
    }
    for (const worker of this.workers) {
      worker.close();
    }
    console.log('MediasoupManager closed');
  }
}
