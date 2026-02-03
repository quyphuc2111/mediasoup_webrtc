# Mediasoup Rust Server

Screen Sharing SFU Server sử dụng mediasoup (Rust bindings).

## Yêu cầu

- Rust 1.70+
- Các dependencies của mediasoup (tự động cài qua cargo)

## Build

```bash
cd mediasoup-rust-server
cargo build --release
```

## Chạy

```bash
cargo run --release
```

Hoặc chạy binary trực tiếp:

```bash
./target/release/mediasoup-rust-server
```

## Cấu hình

Mặc định:
- Port: 3016
- Workers: min(CPU cores, 3)
- Max clients per room: 50
- Max bitrate: 6 Mbps

## API WebSocket

Server sử dụng cùng protocol với phiên bản TypeScript:

### Client → Server

- `join` - Tham gia room
- `getRouterRtpCapabilities` - Lấy RTP capabilities
- `createTransport` - Tạo WebRTC transport
- `connectTransport` - Kết nối transport
- `produce` - Tạo producer (stream)
- `consume` - Tạo consumer (nhận stream)
- `resumeConsumer` - Resume consumer
- `getProducers` - Lấy danh sách producers
- `chatMessage` - Gửi tin nhắn chat

### Server → Client

- `joined` - Đã tham gia room
- `routerRtpCapabilities` - RTP capabilities
- `transportCreated` - Transport đã tạo
- `transportConnected` - Transport đã kết nối
- `produced` - Producer đã tạo
- `consumed` - Consumer đã tạo
- `consumerResumed` - Consumer đã resume
- `producers` - Danh sách producers
- `peerJoined` - Peer mới tham gia
- `peerLeft` - Peer rời đi
- `newProducer` - Producer mới
- `chatMessage` - Tin nhắn chat
- `error` - Lỗi

## So sánh với TypeScript

| Feature | TypeScript | Rust |
|---------|------------|------|
| Performance | Tốt | Tốt hơn |
| Memory | ~50MB | ~10MB |
| Binary size | ~100MB (pkg) | ~5MB |
| Startup time | ~2s | ~100ms |
| Dependencies | Node.js | Không |

## Tích hợp với Tauri

Có thể embed server này vào Tauri app bằng cách:

1. Build release binary
2. Copy vào `src-tauri/binaries/`
3. Sử dụng `tauri::api::process::Command` để spawn
