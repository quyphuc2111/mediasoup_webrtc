import { MediasoupManager } from './MediasoupManager.js';
import { SignalingServer } from './SignalingServer.js';
import { config, getLocalIp } from './config.js';

async function main() {
  console.log('='.repeat(50));
  console.log('Screen Sharing SFU Server (Mediasoup)');
  console.log('='.repeat(50));
  
  const localIp = getLocalIp();
  console.log(`Local IP: ${localIp}`);
  console.log(`WebSocket Port: ${config.listenPort}`);
  console.log(`Max Clients: ${config.maxClientsPerRoom}`);
  console.log('='.repeat(50));

  // Initialize Mediasoup
  const manager = new MediasoupManager();
  await manager.init();

  // Start signaling server
  const signaling = new SignalingServer(manager);

  console.log('\nServer ready!');
  console.log(`Students can connect to: ws://${localIp}:${config.listenPort}`);

  // Graceful shutdown
  const shutdown = () => {
    console.log('\nShutting down...');
    signaling.close();
    manager.close();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}

main().catch((error) => {
  console.error('Failed to start server:', error);
  process.exit(1);
});
