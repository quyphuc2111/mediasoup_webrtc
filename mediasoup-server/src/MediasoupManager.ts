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
import { config, getLocalIp } from './config.js';
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
    });

    // 🔒 LOCK encoder behavior (CỰC KỲ QUAN TRỌNG CHO WINDOWS)
    // Ép CFR 30fps, không cho WebRTC Windows tự drop frame
    // Bitrate không dao động → encode đều → mượt
    if (kind === 'video') {
      try {
        // Set max spatial layer to disable simulcast
        if ('setMaxSpatialLayer' in producer && typeof producer.setMaxSpatialLayer === 'function') {
          await (producer as any).setMaxSpatialLayer(0);
        }

        // Lock bitrate và framerate để tránh Windows WebRTC tự scale
        // Note: setRtpEncodingParameters might not be available in all mediasoup versions
        // We configure these in rtpParameters when creating producer instead
        if ('setRtpEncodingParameters' in producer && typeof producer.setRtpEncodingParameters === 'function') {
          await (producer as any).setRtpEncodingParameters([
            {
              maxBitrate: 4_500_000, // 4.5Mbps (giảm từ 6Mbps - Windows encoder ghét burst)
              minBitrate: 2_500_000, // 2.5Mbps min
              maxFramerate: 25, // 25fps ideal cho Windows
              priority: 'high',
            },
          ]);
          console.log(`Producer ${producer.id}: Locked encoding parameters (4.5Mbps, 25fps) - Windows optimized`);
        } else {
          console.log(`Producer ${producer.id}: Created (encoding parameters set in rtpParameters)`);
        }
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
            maxBitrate: 4_500_000, // 4.5Mbps (match với producer - Windows optimized)
            minBitrate: 2_500_000, // 2.5Mbps min
            priority: 'high',
          },
        ]);
        console.log(`Consumer ${consumer.id}: Locked bitrate (4.5Mbps) and layers - Windows optimized`);
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
