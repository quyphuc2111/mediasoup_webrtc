import os from 'os';
import type {
  WorkerSettings,
  RouterOptions,
  WebRtcTransportOptions
} from 'mediasoup/node/lib/types.js';

// =============================
// Workers
// =============================
const numWorkers = Math.min(os.cpus().length, 3);
// LAN không cần nhiều worker, ưu tiên ổn định

export const config = {
  listenPort: 3016,

  numWorkers,

  // =============================
  // Worker - Tối ưu cho Windows
  // =============================
  worker: {
    rtcMinPort: 40000,
    rtcMaxPort: 45000,
    logLevel: 'error', // 👈 Windows log nhiều gây jitter, chỉ log error
    logTags: [], // Rỗng để giảm overhead logging
  } as WorkerSettings,

  // =============================
  // Router – Codec LAN CHUẨN
  // =============================
  router: {
    mediaCodecs: [
      // -------- AUDIO --------
      {
        kind: 'audio',
        mimeType: 'audio/opus',
        clockRate: 48000,
        channels: 2,
        parameters: {
          'useinbandfec': 1,
          'minptime': 10,
        },
      },

      // -------- VIDEO (CHÍNH) - Tối ưu cho Windows --------
      {
        kind: 'video',
        mimeType: 'video/H264',
        clockRate: 90000,
        parameters: {
          // 👉 packetization-mode: 1 là lựa chọn chính xác nhất cho Windows
          // Mode 1 = Non-interleaved mode (tốt cho real-time, ít latency)
          'packetization-mode': 1,
          
          // 👉 Windows-friendly: Main Profile Level 3.2 (NVENC/QSV encode ổn hơn)
          // 4d0032 = Main Profile Level 3.2 (thay vì Baseline 42e01f)
          'profile-level-id': '4d0032',
          'level-asymmetry-allowed': 1,

          // Chrome / Edge tuning cho Windows - giảm peak bitrate để tránh encoder drop frame
          'x-google-start-bitrate': 2500, // kbps
          'x-google-max-bitrate': 4000,
          'x-google-min-bitrate': 1500,
        },
      },

      // -------- VIDEO (FALLBACK) - VP8 cho Windows --------
      {
        kind: 'video',
        mimeType: 'video/VP8',
        clockRate: 90000,
        parameters: {
          'x-google-start-bitrate': 2000, // Giảm cho Windows
          'x-google-max-bitrate': 3500,
          'x-google-min-bitrate': 1000,
        },
      },
    ],
  } as RouterOptions,

  // =============================
  // WebRTC Transport
  // =============================
  webRtcTransport: {
    listenInfos: [
      {
        protocol: 'udp',
        ip: '0.0.0.0',
        announcedAddress: undefined, // LAN auto detect
      },
      {
        protocol: 'tcp',
        ip: '0.0.0.0',
        announcedAddress: undefined,
      },
    ],
    enableUdp: true,
    enableTcp: true,
    preferUdp: true,

    // Tối ưu Windows: giảm từ 6 → 5 Mbps để tránh encoder burst
    initialAvailableOutgoingBitrate: 5000000, // 5 Mbps (giảm từ 6Mbps cho Windows)
  } as WebRtcTransportOptions,

  // =============================
  // Bitrate Control - Tối ưu Windows (CỰC KỲ QUAN TRỌNG)
  // =============================
  maxIncomingBitrate: 4500000, // 4.5 Mbps / producer (giảm từ 6Mbps - Windows encoder ghét burst)

  // =============================
  // Room constraints
  // =============================
  maxClientsPerRoom: 50,

  // =============================
  // Capture hint cho Teacher - Tối ưu Windows
  // =============================
  videoConstraints: {
    width: { ideal: 1920, max: 1920 },
    height: { ideal: 1080, max: 1080 },
    // 👇 QUAN TRỌNG: Chrome trên Windows 25fps mượt hơn 30fps rất nhiều
    // Mắt người không phân biệt rõ 25 vs 30, nhưng Windows encoder ổn định hơn ở 25fps
    frameRate: { ideal: 25, max: 30 },
  },
};

// Detect local IP
export function getLocalIp(): string {
  const interfaces = os.networkInterfaces();
  for (const name of Object.keys(interfaces)) {
    for (const iface of interfaces[name] || []) {
      if (iface.family === 'IPv4' && !iface.internal) {
        return iface.address;
      }
    }
  }
  return '127.0.0.1';
}
