import type {
  Router,
  Producer,
  Consumer,
  WebRtcTransport,
  RtpCapabilities,
} from 'mediasoup/node/lib/types.js';
import { v4 as uuidv4 } from 'uuid';

export interface Peer {
  id: string;
  name: string;
  isTeacher: boolean;
  transport?: WebRtcTransport;
  recvTransport?: WebRtcTransport;
  producers: Map<string, Producer>;
  consumers: Map<string, Consumer>;
}

export class Room {
  public id: string;
  public router: Router;
  private peers: Map<string, Peer> = new Map();
  private teacherId: string | null = null;

  constructor(router: Router, roomId?: string) {
    this.id = roomId || uuidv4();
    this.router = router;
  }

  get rtpCapabilities(): RtpCapabilities {
    return this.router.rtpCapabilities;
  }

  get teacherProducers(): Producer[] {
    if (!this.teacherId) return [];
    const teacher = this.peers.get(this.teacherId);
    return teacher ? Array.from(teacher.producers.values()) : [];
  }

  get peerCount(): number {
    return this.peers.size;
  }

  hasTeacher(): boolean {
    return this.teacherId !== null;
  }

  addPeer(id: string, name: string, isTeacher: boolean): Peer {
    const peer: Peer = {
      id,
      name,
      isTeacher,
      producers: new Map(),
      consumers: new Map(),
    };
    
    this.peers.set(id, peer);
    
    if (isTeacher) {
      this.teacherId = id;
    }
    
    console.log(`[Room ${this.id}] Peer joined: ${name} (${isTeacher ? 'Teacher' : 'Student'})`);
    return peer;
  }

  getPeer(id: string): Peer | undefined {
    return this.peers.get(id);
  }

  removePeer(id: string): void {
    const peer = this.peers.get(id);
    if (!peer) return;

    // Close all producers
    for (const producer of peer.producers.values()) {
      producer.close();
    }

    // Close all consumers
    for (const consumer of peer.consumers.values()) {
      consumer.close();
    }

    // Close transports
    peer.transport?.close();
    peer.recvTransport?.close();

    if (peer.isTeacher) {
      this.teacherId = null;
    }

    this.peers.delete(id);
    console.log(`[Room ${this.id}] Peer left: ${peer.name}`);
  }

  getAllPeers(): Peer[] {
    return Array.from(this.peers.values());
  }

  getStudents(): Peer[] {
    return this.getAllPeers().filter(p => !p.isTeacher);
  }

  isEmpty(): boolean {
    return this.peers.size === 0;
  }

  close(): void {
    for (const peer of this.peers.values()) {
      this.removePeer(peer.id);
    }
    this.router.close();
    console.log(`[Room ${this.id}] Closed`);
  }
}
