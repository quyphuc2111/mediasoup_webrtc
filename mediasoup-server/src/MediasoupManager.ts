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

    // ⚠️ LƯU Ý QUAN TRỌNG:
    // Mediasoup Producer KHÔNG có setRtpEncodingParameters() hoặc setMaxSpatialLayer()
    // Bitrate và framerate được quyết định ở CLIENT khi gọi transport.produce()
    // Server chỉ có thể giới hạn tổng bitrate qua transport.setMaxIncomingBitrate()
    // 
    // Để "Lock" 25fps và 4.5Mbps cho Windows, phải cấu hình ở CLIENT:
    // encodings: [{ maxBitrate: 4500000, maxFramerate: 25 }]
    //
    // Xem: src/lib/mediasoup-client.ts - produceScreen()
    console.log(`Producer ${producer.id} created [${kind}] - encoding parameters set by client`);

    producer.on('transportclose', () => {
      producer.close();
    });

    return producer;
  }

  async createConsumer(
    room: Room,
    transport: WebRtcTransport,
    producer: Producer,
    rtpCapabilities: RtpCapabilities
  ): Promise<Consumer | null> {
    // Kiểm tra xem router có thể consume producer này không
    if (!room.router.canConsume({ producerId: producer.id, rtpCapabilities })) {
      console.warn(`Cannot consume producer ${producer.id} - codec mismatch or unsupported`);
      return null;
    }

    const consumer = await transport.consume({
      producerId: producer.id,
      rtpCapabilities,
      paused: true, // Start paused, resume after client ready
    });

    // 🔒 Tối ưu Windows/LAN - Điều khiển Consumer qua đúng API của mediasoup
    try {
      // Set preferred layers: Ép consumer nhận layer cao nhất (LAN băng thông rộng)
      // spatialLayer: 0 (vì không dùng simulcast, chỉ có 1 layer)
      // temporalLayer: 2 (nếu producer có temporal scalability, nhận layer cao nhất)
      if (consumer.type !== 'simple') {
        await consumer.setPreferredLayers({ spatialLayer: 0, temporalLayer: 2 });
        console.log(`Consumer ${consumer.id}: Set preferred layers (spatial: 0, temporal: 2)`);
      }

      // Set priority: Ưu tiên xử lý Consumer này (tốn thêm CPU nhưng giảm drop frame)
      // Priority range: 1-10 (10 = highest), 5 = medium-high
      await consumer.setPriority(5);
      console.log(`Consumer ${consumer.id}: Set priority to 5 (medium-high)`);
    } catch (error) {
      console.warn(`Failed to optimize consumer ${consumer.id}:`, error);
    }

    // ⚠️ LƯU Ý:
    // Consumer KHÔNG có setRtpEncodingParameters() - bitrate được điều khiển bởi:
    // 1. Producer bitrate (set ở client)
    // 2. Transport maxIncomingBitrate (đã set trong createWebRtcTransport)
    // 3. setPreferredLayers() và setPriority() (đã set ở trên)

    consumer.on('transportclose', () => {
      consumer.close();
    });

    consumer.on('producerclose', () => {
      consumer.close();
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
