# Screen Sharing - WebRTC + Mediasoup SFU

Ứng dụng chia sẻ màn hình cho lớp học với hỗ trợ 30-50 học sinh đồng thời.

## Tính năng

- ✅ Chia sẻ màn hình với âm thanh hệ thống
- ✅ Chia sẻ giọng nói qua microphone
- ✅ Hỗ trợ 30-50 clients đồng thời
- ✅ Tối ưu cho máy cấu hình thấp
- ✅ WebRTC + SFU (Mediasoup)
- ✅ Tự động khởi động server khi mở app

## Kiến trúc

```
┌─────────────────────────────────────────────────────────┐
│                    Teacher App (Tauri)                   │
├─────────────────────────────────────────────────────────┤
│  React Frontend          │  Mediasoup Server (Sidecar)  │
│  - Screen capture        │  - WebSocket signaling       │
│  - Audio capture         │  - SFU routing               │
│  - mediasoup-client      │  - Room management           │
├─────────────────────────────────────────────────────────┤
│                    Tauri Backend (Rust)                  │
│                  - Sidecar management                    │
└─────────────────────────────────────────────────────────┘
                              │
                              │ WebRTC
                              ▼
┌─────────────────────────────────────────────────────────┐
│                Students (30-50 clients)                  │
│              Browser hoặc Student App                    │
└─────────────────────────────────────────────────────────┘
```

## Cài đặt

### 1. Cài đặt dependencies

```bash
# Frontend
npm install

# Mediasoup server
npm run server:install
```

### 2. Build Mediasoup server

```bash
npm run server:build
```

### 3. Chạy development

```bash
# Terminal 1: Mediasoup server
npm run server:dev

# Terminal 2: Tauri app
npm run tauri dev
```

## Sử dụng

### Giáo viên (Teacher)

1. Mở ứng dụng
2. Click "Khởi động Server"
3. Nhập tên và Room ID
4. Click "Giáo viên"
5. Click "Kết nối Server"
6. Click "Chia sẻ màn hình + Âm thanh"

### Học sinh (Student)

1. Mở ứng dụng hoặc truy cập web
2. Nhập Server URL (từ giáo viên)
3. Nhập tên và Room ID
4. Click "Học sinh"
5. Click "Kết nối vào lớp"

## Tối ưu cho máy cấu hình thấp

- Resolution: 720p max
- Frame rate: 24fps
- Codec: VP8 (nhẹ hơn VP9/H264)
- Simulcast: 3 layers cho adaptive quality
- Max 2 Mediasoup workers
- Giới hạn bitrate: 1.5Mbps

## Build Production

```bash
# Build mediasoup server binary
cd mediasoup-server
npm run pkg

# Build Tauri app
npm run tauri build
```

## Cấu trúc thư mục

```
├── src/                      # React frontend
│   ├── components/
│   │   ├── TeacherView.tsx
│   │   ├── StudentView.tsx
│   │   └── VideoPlayer.tsx
│   ├── hooks/
│   │   └── useMediasoup.ts
│   ├── lib/
│   │   └── mediasoup-client.ts
│   └── App.tsx
├── src-tauri/                # Tauri backend
│   ├── src/
│   │   └── lib.rs
│   └── binaries/             # Sidecar binaries
├── mediasoup-server/         # Node.js SFU server
│   └── src/
│       ├── index.ts
│       ├── config.ts
│       ├── Room.ts
│       ├── MediasoupManager.ts
│       └── SignalingServer.ts
└── package.json
```

## Yêu cầu hệ thống

### Server (Giáo viên)
- CPU: 2+ cores
- RAM: 4GB+
- Network: 10Mbps+ upload

### Client (Học sinh)
- Browser: Chrome/Edge/Firefox (mới nhất)
- Network: 2Mbps+ download
