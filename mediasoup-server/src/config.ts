import os from 'os';
import type { WorkerSettings, RouterOptions, WebRtcTransportOptions } from 'mediasoup/node/lib/types.js';

// Tối ưu cho chất lượng cao - sử dụng nhiều workers hơn
const numWorkers = Math.min(os.cpus().length, 4); // Tăng lên 4 workers cho chất lượng cao

export const config = {
  // Server settings
  listenPort: 3016,
  
  // Mediasoup Worker settings - tối ưu cho chất lượng cao
  worker: {
    rtcMinPort: 40000,
    rtcMaxPort: 49999, // Mở rộng port range cho nhiều kết nối
    logLevel: 'warn',
    logTags: ['info', 'ice', 'dtls', 'rtp', 'srtp', 'rtcp'],
  } as WorkerSettings,
  
  numWorkers,
  
  // Router settings với codecs chất lượng cao
  router: {
    mediaCodecs: [
      {
        kind: 'audio',
        mimeType: 'audio/opus',
        clockRate: 48000,
        channels: 2,
        parameters: {
          'sprop-stereo': 1,
          'useinbandfec': 1, // Forward Error Correction
          'minptime': 10, // Minimum packet time
          'maxplaybackrate': 48000, // Maximum playback rate
        },
      },
      {
        kind: 'video',
        mimeType: 'video/VP9', // VP9 cho chất lượng cao hơn VP8
        clockRate: 90000,
        parameters: {
          'x-google-start-bitrate': 2500000, // Start bitrate cao cho chất lượng tốt
          'x-google-min-bitrate': 1000000, // Min bitrate để đảm bảo chất lượng
          'x-google-max-bitrate': 10000000, // Max bitrate cho 4K
        },
      },
      {
        kind: 'video',
        mimeType: 'video/H264',
        clockRate: 90000,
        parameters: {
          'packetization-mode': 1,
          'profile-level-id': '640032', // High profile level 4.0 - chất lượng cao nhất
          'level-asymmetry-allowed': 1,
          'x-google-start-bitrate': 2500000, // Start bitrate cao
          'x-google-min-bitrate': 1000000, // Min bitrate
          'x-google-max-bitrate': 10000000, // Max bitrate cho 4K
        },
      },
      {
        kind: 'video',
        mimeType: 'video/VP8', // Giữ VP8 làm fallback
        clockRate: 90000,
        parameters: {
          'x-google-start-bitrate': 2000000, // Start bitrate cao
          'x-google-min-bitrate': 800000,
          'x-google-max-bitrate': 8000000,
        },
      },
    ],
  } as RouterOptions,
  
  // WebRTC Transport settings - tối ưu cho chất lượng cao
  webRtcTransport: {
    listenInfos: [
      { 
        protocol: 'udp' as const,
        ip: '0.0.0.0', 
        announcedAddress: undefined // Sẽ detect IP tự động
      },
      {
        protocol: 'tcp' as const,
        ip: '0.0.0.0',
        announcedAddress: undefined
      }
    ],
    enableUdp: true,
    enableTcp: true,
    preferUdp: true,
    initialAvailableOutgoingBitrate: 10000000, // 10Mbps start - cao hơn nhiều
  },
  
  maxIncomingBitrate: 10000000, // Max 10Mbps incoming - hỗ trợ 4K
  
  // Tối ưu cho 30-50 clients
  maxClientsPerRoom: 50,
  
  // Video constraints cho teacher - hỗ trợ 4K và 60fps
  videoConstraints: {
    width: { ideal: 3840, max: 3840 }, // 4K UHD
    height: { ideal: 2160, max: 2160 }, // 4K UHD
    frameRate: { ideal: 60, max: 60 }, // 60fps cho mượt mà
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
