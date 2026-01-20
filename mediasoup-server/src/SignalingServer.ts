import { WebSocketServer, WebSocket } from 'ws';
import type { IncomingMessage } from 'http';
import type { RtpCapabilities, DtlsParameters, MediaKind, RtpParameters } from 'mediasoup/node/lib/types.js';
import { MediasoupManager } from './MediasoupManager.js';
import { config } from './config.js';
import type { Peer } from './Room.js';

interface SignalingMessage {
  type: string;
  data?: any;
}

interface ClientInfo {
  peerId: string;
  roomId: string;
  ws: WebSocket;
  rtpCapabilities?: RtpCapabilities;
}

export class SignalingServer {
  private wss: WebSocketServer;
  private manager: MediasoupManager;
  private clients: Map<WebSocket, ClientInfo> = new Map();

  constructor(manager: MediasoupManager) {
    this.manager = manager;
    this.wss = new WebSocketServer({ port: config.listenPort });
    this.setupServer();
  }

  private setupServer(): void {
    this.wss.on('connection', (ws: WebSocket, req: IncomingMessage) => {
      console.log('New WebSocket connection');
      
      ws.on('message', async (data) => {
        try {
          const message: SignalingMessage = JSON.parse(data.toString());
          await this.handleMessage(ws, message);
        } catch (error) {
          console.error('Error handling message:', error);
          this.send(ws, { type: 'error', data: { message: 'Invalid message' } });
        }
      });

      ws.on('close', () => {
        this.handleDisconnect(ws);
      });

      ws.on('error', (error) => {
        console.error('WebSocket error:', error);
      });
    });

    console.log(`Signaling server listening on port ${config.listenPort}`);
  }

  private send(ws: WebSocket, message: SignalingMessage): void {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(message));
    }
  }

  private broadcast(roomId: string, message: SignalingMessage, excludeWs?: WebSocket): void {
    for (const [ws, info] of this.clients) {
      if (info.roomId === roomId && ws !== excludeWs) {
        this.send(ws, message);
      }
    }
  }

  private async handleMessage(ws: WebSocket, message: SignalingMessage): Promise<void> {
    const { type, data } = message;

    switch (type) {
      case 'join':
        await this.handleJoin(ws, data);
        break;
      case 'getRouterRtpCapabilities':
        await this.handleGetRouterRtpCapabilities(ws);
        break;
      case 'createTransport':
        await this.handleCreateTransport(ws, data);
        break;
      case 'connectTransport':
        await this.handleConnectTransport(ws, data);
        break;
      case 'produce':
        await this.handleProduce(ws, data);
        break;
      case 'consume':
        await this.handleConsume(ws, data);
        break;
      case 'resumeConsumer':
        await this.handleResumeConsumer(ws, data);
        break;
      case 'getProducers':
        await this.handleGetProducers(ws);
        break;
      default:
        console.warn('Unknown message type:', type);
    }
  }

  private async handleJoin(ws: WebSocket, data: { roomId: string; peerId: string; name: string; isTeacher: boolean }): Promise<void> {
    const { roomId, peerId, name, isTeacher } = data;
    
    const room = await this.manager.getOrCreateRoom(roomId);
    
    // Check if room already has teacher
    if (isTeacher && room.hasTeacher()) {
      this.send(ws, { type: 'error', data: { message: 'Room already has a teacher' } });
      return;
    }

    // Check max clients
    if (room.peerCount >= config.maxClientsPerRoom) {
      this.send(ws, { type: 'error', data: { message: 'Room is full' } });
      return;
    }

    room.addPeer(peerId, name, isTeacher);
    
    this.clients.set(ws, { peerId, roomId, ws });

    this.send(ws, {
      type: 'joined',
      data: {
        roomId: room.id,
        peerId,
        isTeacher,
        rtpCapabilities: room.rtpCapabilities,
      },
    });

    // Notify others
    this.broadcast(roomId, {
      type: 'peerJoined',
      data: { peerId, name, isTeacher },
    }, ws);

    console.log(`Peer ${name} joined room ${roomId} as ${isTeacher ? 'Teacher' : 'Student'}`);
  }

  private async handleGetRouterRtpCapabilities(ws: WebSocket): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    this.send(ws, {
      type: 'routerRtpCapabilities',
      data: room.rtpCapabilities,
    });
  }

  private async handleCreateTransport(ws: WebSocket, data: { direction: 'send' | 'recv' }): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const peer = room.getPeer(info.peerId);
    if (!peer) return;

    const { transport, params } = await this.manager.createWebRtcTransport(room);

    if (data.direction === 'send') {
      peer.transport = transport;
    } else {
      peer.recvTransport = transport;
    }

    this.send(ws, {
      type: 'transportCreated',
      data: { direction: data.direction, ...params },
    });
  }

  private async handleConnectTransport(ws: WebSocket, data: { direction: 'send' | 'recv'; dtlsParameters: DtlsParameters }): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const peer = room.getPeer(info.peerId);
    if (!peer) return;

    const transport = data.direction === 'send' ? peer.transport : peer.recvTransport;
    if (!transport) return;

    await this.manager.connectTransport(transport, data.dtlsParameters);

    this.send(ws, { type: 'transportConnected', data: { direction: data.direction } });
  }

  private async handleProduce(ws: WebSocket, data: { kind: MediaKind; rtpParameters: RtpParameters }): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const peer = room.getPeer(info.peerId);
    if (!peer || !peer.transport) return;

    // Only teacher can produce
    if (!peer.isTeacher) {
      this.send(ws, { type: 'error', data: { message: 'Only teacher can share screen' } });
      return;
    }

    const producer = await this.manager.createProducer(peer.transport, data.kind, data.rtpParameters);
    peer.producers.set(producer.id, producer);

    this.send(ws, {
      type: 'produced',
      data: { producerId: producer.id, kind: data.kind },
    });

    // Notify students about new producer
    this.broadcast(info.roomId, {
      type: 'newProducer',
      data: { producerId: producer.id, kind: data.kind, peerId: info.peerId },
    }, ws);

    console.log(`Teacher produced ${data.kind}: ${producer.id}`);
  }

  private async handleConsume(ws: WebSocket, data: { producerId: string; rtpCapabilities: RtpCapabilities }): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const peer = room.getPeer(info.peerId);
    if (!peer || !peer.recvTransport) return;

    // Find producer (from teacher)
    let producer = null;
    for (const p of room.teacherProducers) {
      if (p.id === data.producerId) {
        producer = p;
        break;
      }
    }

    if (!producer) {
      this.send(ws, { type: 'error', data: { message: 'Producer not found' } });
      return;
    }

    const consumer = await this.manager.createConsumer(
      room,
      peer.recvTransport,
      producer,
      data.rtpCapabilities
    );

    if (!consumer) {
      this.send(ws, { type: 'error', data: { message: 'Cannot consume' } });
      return;
    }

    peer.consumers.set(consumer.id, consumer);

    this.send(ws, {
      type: 'consumed',
      data: {
        consumerId: consumer.id,
        producerId: producer.id,
        kind: consumer.kind,
        rtpParameters: consumer.rtpParameters,
      },
    });
  }

  private async handleResumeConsumer(ws: WebSocket, data: { consumerId: string }): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const peer = room.getPeer(info.peerId);
    if (!peer) return;

    const consumer = peer.consumers.get(data.consumerId);
    if (!consumer) return;

    await consumer.resume();
    this.send(ws, { type: 'consumerResumed', data: { consumerId: data.consumerId } });
  }

  private async handleGetProducers(ws: WebSocket): Promise<void> {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (!room) return;

    const producers = room.teacherProducers.map(p => ({
      producerId: p.id,
      kind: p.kind,
    }));

    this.send(ws, { type: 'producers', data: producers });
  }

  private handleDisconnect(ws: WebSocket): void {
    const info = this.clients.get(ws);
    if (!info) return;

    const room = this.manager.getRoom(info.roomId);
    if (room) {
      const peer = room.getPeer(info.peerId);
      const wasTeacher = peer?.isTeacher;
      
      room.removePeer(info.peerId);

      // Notify others
      this.broadcast(info.roomId, {
        type: 'peerLeft',
        data: { peerId: info.peerId, wasTeacher },
      });

      // Clean up empty room
      if (room.isEmpty()) {
        this.manager.removeRoom(info.roomId);
      }
    }

    this.clients.delete(ws);
    console.log(`Peer ${info.peerId} disconnected`);
  }

  close(): void {
    this.wss.close();
    console.log('Signaling server closed');
  }
}
