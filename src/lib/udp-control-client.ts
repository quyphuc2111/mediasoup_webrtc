/**
 * UDP Control Client for remote control
 * Sends control commands directly to student via UDP
 */

export interface UdpControlConfig {
  studentIp: string;
  studentPort: number;
}

export class UdpControlClient {
  private config: UdpControlConfig | null = null;

  constructor() {
    // UDP control client for sending commands via Tauri
  }

  async connect(config: UdpControlConfig): Promise<void> {
    this.config = config;
    
    // Create UDP socket using WebRTC DataChannel or WebSocket fallback
    // Since browser doesn't support raw UDP, we'll use WebSocket as proxy
    // For Tauri, we can use native UDP via Rust backend
    
    console.log('[UdpControlClient] Connected to', config);
  }

  async sendMouseControl(event: any): Promise<void> {
    if (!this.config) {
      throw new Error('Not connected');
    }

    const message = {
      type: 'mouse',
      ...event,
    };

    await this.sendMessage(message);
  }

  async sendKeyboardControl(event: any): Promise<void> {
    if (!this.config) {
      throw new Error('Not connected');
    }

    const message = {
      type: 'keyboard',
      ...event,
    };

    await this.sendMessage(message);
  }

  async sendControlCommand(action: string): Promise<void> {
    if (!this.config) {
      throw new Error('Not connected');
    }

    const message = {
      type: 'control',
      action,
    };

    await this.sendMessage(message);
  }

  private async sendMessage(message: any): Promise<void> {
    if (!this.config) {
      throw new Error('Not connected');
    }

    // Check if we're in Tauri environment
    if (typeof window !== 'undefined' && (window as any).__TAURI__) {
      // Use Tauri command to send UDP
      const { invoke } = await import('@tauri-apps/api/core');
      try {
        await invoke('send_udp_message', {
          ip: this.config.studentIp,
          port: this.config.studentPort,
          message: JSON.stringify(message),
        });
      } catch (error) {
        console.error('[UdpControlClient] Failed to send UDP message:', error);
        throw error;
      }
    } else {
      // Fallback: use WebSocket proxy (if available)
      console.warn('[UdpControlClient] UDP not available, using WebSocket fallback');
      throw new Error('UDP not available in browser. Please use Tauri app.');
    }
  }

  disconnect(): void {
    this.config = null;
  }
}
