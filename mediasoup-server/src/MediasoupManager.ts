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
        {
          protocol: 'tcp' as const,
          ip: '0.0.0.0',
          announcedAddress: localIp,
        },
      ],
      enableUdp: true,
      enableTcp: true,
      preferUdp: true,
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
    const producer = await transport.produce({ kind, rtpParameters });
    
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
