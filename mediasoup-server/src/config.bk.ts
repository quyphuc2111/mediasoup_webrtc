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
  // Worker
  // =============================
  worker: {
    rtcMinPort: 40000,
    rtcMaxPort: 45000,
    logLevel: 'warn',
    logTags: ['ice', 'dtls', 'rtp', 'rtcp'],
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

      // -------- VIDEO (CHÍNH) --------
      {
        kind: 'video',
        mimeType: 'video/H264',
        clockRate: 90000,
        parameters: {
          'packetization-mode': 1,
          // H264 Baseline – tương thích tối đa, encode nhẹ
          'profile-level-id': '42e01f',
          'level-asymmetry-allowed': 1,

          // Bitrate THỰC TẾ cho LAN
          'x-google-start-bitrate': 3000, // kbps
          'x-google-max-bitrate': 5000,
        },
      },

      // -------- VIDEO (FALLBACK) --------
      {
        kind: 'video',
        mimeType: 'video/VP8',
        clockRate: 90000,
        parameters: {
          'x-google-start-bitrate': 2500,
          'x-google-max-bitrate': 4000,
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

    // Quan trọng: KHÔNG để quá cao
    initialAvailableOutgoingBitrate: 6000000, // 6 Mbps
  } as WebRtcTransportOptions,

  // =============================
  // Bitrate Control (CỰC KỲ QUAN TRỌNG)
  // =============================
  maxIncomingBitrate: 6000000, // 6 Mbps / producer

  // =============================
  // Room constraints
  // =============================
  maxClientsPerRoom: 50,

  // =============================
  // Capture hint cho Teacher
  // =============================
  videoConstraints: {
    width: { ideal: 1920, max: 1920 },
    height: { ideal: 1080, max: 1080 },
    frameRate: { ideal: 30, max: 30 },
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
