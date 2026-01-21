# Architecture & Flow Documentation

## 📋 Tổng quan

Dự án này là một hệ thống chia sẻ màn hình và audio real-time sử dụng WebRTC và Mediasoup SFU (Selective Forwarding Unit) cho môi trường LAN. Hệ thống hỗ trợ một Teacher chia sẻ màn hình/audio cho nhiều Students.

---

## 🏗️ Kiến trúc tổng thể

```
┌─────────────────────────────────────────────────────────────────┐
│                         Tauri Desktop App                        │
│  ┌──────────────────────┐         ┌──────────────────────┐     │
│  │   Teacher App        │         │   Student App        │     │
│  │  (React + TypeScript)│         │  (React + TypeScript)│     │
│  │                      │         │                      │     │
│  │  - Screen Share      │         │  - View Screen       │     │
│  │  - Microphone        │         │  - Listen Audio      │     │
│  │  - VideoPlayer       │         │  - VideoPlayer       │     │
│  └──────────────────────┘         └──────────────────────┘     │
│           │                                  │                   │
│           └──────────────┬───────────────────┘                   │
│                          │                                       │
│              ┌───────────▼───────────┐                          │
│              │  MediasoupClient      │                          │
│              │  (WebSocket + WebRTC) │                          │
│              └───────────┬───────────┘                          │
└──────────────────────────┼──────────────────────────────────────┘
                           │
                ┌──────────▼──────────┐
                │  Mediasoup Server   │
                │  (Node.js/TypeScript)│
                │                     │
                │  - SignalingServer  │
                │  - MediasoupManager │
                │  - Room             │
                │  - Router (SFU)     │
                └─────────────────────┘
```

---

## 👨‍🏫 Teacher Flow (Luồng hoạt động của Teacher)

### 1. Khởi động và kết nối

```
Teacher App Start
    │
    ├─► Tauri Rust Backend khởi động Mediasoup Server (background process)
    │   └─► Server lắng nghe trên port 3016 (WebSocket)
    │
    └─► React App render TeacherView
           │
           └─► User nhập Room ID và Name
                  │
                  └─► Click "Tham gia phòng"
                         │
                         ├─► useMediasoup.connect()
                         │   │
                         │   ├─► Create MediasoupClient instance
                         │   │
                         │   ├─► WebSocket connect to ws://localhost:3016
                         │   │   │
                         │   │   └─► Send: { type: "join", data: { roomId, peerId, name, isTeacher: true } }
                         │   │       │
                         │   │       └─► Server Response: { type: "joined", data: { roomId, rtpCapabilities } }
                         │   │
                         │   ├─► Load Device with routerRtpCapabilities
                         │   │
                         │   └─► Connection State = "connected"
                         │
                         └─► UI hiển thị: "Đã kết nối" + Button "Chia sẻ màn hình"
```

### 2. Chia sẻ màn hình

```
User click "Chia sẻ màn hình"
    │
    ├─► useMediasoup.startScreenShare()
    │   │
    │   ├─► Check MediaDevices support
    │   │
    │   ├─► navigator.mediaDevices.getDisplayMedia()
    │   │   └─► Browser/OS shows screen selection dialog
    │   │       └─► User selects screen/window
    │   │           └─► Returns MediaStream (video + optional system audio)
    │   │
    │   ├─► client.produceScreen(stream)
    │   │   │
    │   │   ├─► Create Send Transport (if not exists)
    │   │   │   │
    │   │   │   ├─► Send: { type: "createTransport", data: { direction: "send" } }
    │   │   │   │   └─► Server Response: { type: "transportCreated", data: { id, iceParameters, iceCandidates, dtlsParameters } }
    │   │   │   │
    │   │   │   └─► Create SendTransport with mediasoup-client
    │   │   │       └─► Transport emits "connect" event
    │   │   │           └─► Send: { type: "connectTransport", data: { direction: "send", dtlsParameters } }
    │   │   │
    │   │   ├─► Produce Video Track
    │   │   │   │
    │   │   │   └─► sendTransport.produce({ track: videoTrack, encodings: [{ maxBitrate: 5000000, maxFramerate: 30 }] })
    │   │   │       │
    │   │   │       ├─► Transport emits "produce" event
    │   │   │       │   └─► Send: { type: "produce", data: { kind: "video", rtpParameters } }
    │   │   │       │       │
    │   │   │       │       └─► Server: createProducer() → Lock encoding (6Mbps, 30fps) → Response: { type: "produced", data: { producerId } }
    │   │   │       │
    │   │   │       └─► Producer created, stream flowing
    │   │   │
    │   │   └─► Produce Audio Track (if system audio enabled)
    │   │       └─► Similar flow for audio track
    │   │
    │   └─► Server broadcasts to all Students: { type: "newProducer", data: { producerId, kind } }
    │
    └─► UI: Button changes to "Dừng chia sẻ" + Local preview shows screen
```

### 3. Bật/Tắt Microphone

```
User click "Bật Microphone"
    │
    ├─► useMediasoup.startMicrophone()
    │   │
    │   ├─► navigator.mediaDevices.getUserMedia({ audio: true })
    │   │   └─► Browser/OS requests microphone permission
    │   │       └─► Returns MediaStream (audio track)
    │   │
    │   ├─► client.produceMicrophone(stream)
    │   │   │
    │   │   ├─► sendTransport.produce({ track: audioTrack })
    │   │   │   └─► Similar produce flow as video
    │   │   │
    │   │   └─► Store producerId for later stop
    │   │
    │   └─► UI: Button changes to "Tắt Microphone"
    │
    └─► User click "Tắt Microphone"
          │
          └─► client.stopProducer(producerId)
                └─► Track stopped, producer closed
```

---

## 👨‍🎓 Student Flow (Luồng hoạt động của Student)

### 1. Kết nối và join room

```
Student App Start
    │
    └─► User nhập Room ID và Name
          │
          └─► Click "Tham gia phòng"
                │
                ├─► useMediasoup.connect()
                │   │
                │   ├─► Similar WebSocket connection flow as Teacher
                │   │
                │   ├─► Connection State = "connected"
                │   │
                │   └─► IF not teacher:
                │       │
                │       ├─► Create Recv Transport
                │       │   └─► Send: { type: "createTransport", data: { direction: "recv" } }
                │       │
                │       └─► client.consumeAll()
                │           │
                │           ├─► Send: { type: "getProducers" }
                │           │   └─► Server Response: { type: "producers", data: [{ producerId, kind }] }
                │           │
                │           └─► For each producer:
                │               └─► consume(producerId)
                │                   │
                │                   ├─► Send: { type: "consume", data: { producerId, rtpCapabilities } }
                │                   │   └─► Server Response: { type: "consumed", data: { consumerId, rtpParameters } }
                │                   │
                │                   ├─► recvTransport.consume({ id, producerId, kind, rtpParameters })
                │                   │   └─► Consumer created, track available
                │                   │
                │                   ├─► Send: { type: "resumeConsumer", data: { consumerId } }
                │                   │   └─► Consumer starts receiving media
                │                   │
                │                   └─► Add track to MediaStream
                │
                └─► UI: VideoPlayer displays remote stream (if teacher is sharing)
```

### 2. Nhận stream mới (New Producer)

```
Teacher starts sharing (new producer created)
    │
    ├─► Server broadcasts: { type: "newProducer", data: { producerId, kind } }
    │   │
    │   └─► All Students receive message
    │       │
    │       └─► MediasoupClient.onNewProducer() triggered
    │           │
    │           └─► useMediasoup.onNewProducer()
    │               │
    │               ├─► client.consume(producerId)
    │               │   │
    │               │   └─► Same consume flow as above
    │               │
    │               └─► Update remoteStream with new track
    │                   └─► VideoPlayer automatically updates
```

---

## 🔄 Signaling Flow (Luồng Signaling qua WebSocket)

### Message Types

#### 1. Join Room
```
Client → Server: { type: "join", data: { roomId, peerId, name, isTeacher } }
Server → Client: { type: "joined", data: { roomId, peerId, rtpCapabilities } }
Server → All: { type: "peerJoined", data: { peerId, name, isTeacher } }
```

#### 2. Create Transport
```
Client → Server: { type: "createTransport", data: { direction: "send" | "recv" } }
Server → Client: { type: "transportCreated", data: { direction, id, iceParameters, iceCandidates, dtlsParameters } }
```

#### 3. Connect Transport (DTLS Handshake)
```
Client → Server: { type: "connectTransport", data: { direction, dtlsParameters } }
Server → Client: { type: "transportConnected", data: { direction } }
```

#### 4. Produce Media
```
Client → Server: { type: "produce", data: { kind: "video" | "audio", rtpParameters } }
Server → Client: { type: "produced", data: { producerId, kind } }
Server → All Students: { type: "newProducer", data: { producerId, kind, peerId } }
```

#### 5. Consume Media
```
Client → Server: { type: "consume", data: { producerId, rtpCapabilities } }
Server → Client: { type: "consumed", data: { consumerId, producerId, kind, rtpParameters } }
```

#### 6. Resume Consumer
```
Client → Server: { type: "resumeConsumer", data: { consumerId } }
Server → Client: { type: "consumerResumed", data: { consumerId } }
```

#### 7. Get Producers List
```
Client → Server: { type: "getProducers" }
Server → Client: { type: "producers", data: [{ producerId, kind }] }
```

---

## 🌊 Media Flow (Luồng Media qua WebRTC)

### WebRTC Connection Process

```
1. ICE Candidate Exchange
   ├─► Server generates ICE candidates (UDP, local IP)
   ├─► Server sends candidates to client via WebSocket
   └─► Client uses candidates for peer connection

2. DTLS Handshake
   ├─► Client generates DTLS parameters (fingerprint, role)
   ├─► Client sends dtlsParameters via WebSocket
   └─► Server completes DTLS handshake → Encrypted connection

3. RTP Media Stream
   ├─► Producer (Teacher) → Router (SFU) → Consumers (Students)
   ├─► Video: H.264 or VP8, 1920x1080 @ 30fps, ~6Mbps
   ├─► Audio: Opus, 48kHz, stereo
   └─► All via UDP (TCP disabled for LAN performance)
```

### SFU Architecture (Selective Forwarding Unit)

```
Teacher (Producer)
    │
    ├─► [Video Track] ──┐
    │                    │
    └─► [Audio Track] ──┼──► Router (SFU)
                         │   │
                         │   ├─► Consumer 1 (Student 1)
                         │   │   ├─► Video Track
                         │   │   └─► Audio Track
                         │   │
                         │   ├─► Consumer 2 (Student 2)
                         │   │   ├─► Video Track
                         │   │   └─► Audio Track
                         │   │
                         │   └─► Consumer N (Student N)
                         │       ├─► Video Track
                         │       └─► Audio Track

Key Points:
- Router receives ONE stream from Teacher
- Router forwards to ALL Students (duplicates stream)
- No P2P connection between Students
- All traffic goes through SFU
```

---

## 🎛️ Mediasoup Server Architecture

### Components

#### 1. SignalingServer
- WebSocket server (port 3016)
- Handles all signaling messages
- Manages client connections
- Routes messages to MediasoupManager

#### 2. MediasoupManager
- Manages Workers (mediasoup workers)
- Creates and manages Rooms
- Creates Transports (WebRTC transports)
- Creates Producers and Consumers
- Applies encoding locks (Windows optimization)

#### 3. Room
- Contains Router (SFU router)
- Manages Peers (Teacher + Students)
- Tracks Producers and Consumers
- Handles peer join/leave

#### 4. Router (SFU)
- Mediasoup Router instance
- Codecs: H.264, VP8 (video), Opus (audio)
- RTP processing and forwarding
- Bitrate control (6Mbps max)

### Flow Example: Teacher Shares Screen

```
1. Teacher → SignalingServer: produce { video, rtpParameters }
2. SignalingServer → MediasoupManager: createProducer()
3. MediasoupManager:
   ├─► Get Teacher's transport
   ├─► transport.produce({ kind: "video", rtpParameters })
   ├─► Lock encoding: maxBitrate=6Mbps, maxFramerate=30fps
   └─► Return Producer
4. SignalingServer → Teacher: produced { producerId }
5. SignalingServer → All Students: newProducer { producerId, kind }
6. Student → SignalingServer: consume { producerId }
7. SignalingServer → MediasoupManager: createConsumer()
8. MediasoupManager:
   ├─► Find Producer from Teacher
   ├─► Get Student's recvTransport
   ├─► transport.consume({ producerId })
   ├─► Lock bitrate: maxBitrate=6Mbps
   └─► Return Consumer
9. SignalingServer → Student: consumed { consumerId, rtpParameters }
10. Student → SignalingServer: resumeConsumer { consumerId }
11. Media flows: Teacher → Router → Student
```

---

## 🔐 Security & Permissions

### macOS Permissions

#### Teacher App cần:
1. **Screen Recording** (`NSScreenCaptureUsageDescription`)
   - Để chia sẻ màn hình
   - System Settings > Privacy & Security > Screen Recording

2. **Microphone** (`NSMicrophoneUsageDescription`)
   - Để bật microphone
   - System Settings > Privacy & Security > Microphone

#### Student App cần:
- Không cần permission đặc biệt (chỉ xem)

### WebRTC Security

- **DTLS Encryption**: Tất cả media streams được mã hóa
- **ICE**: Chỉ kết nối trong LAN (local IP)
- **No External Access**: Server chỉ lắng nghe trên localhost

---

## 📊 Performance Optimizations

### Windows Jitter Fixes

1. **Producer Encoding Lock**
   - Max Bitrate: 6Mbps (fixed)
   - Min Bitrate: 3Mbps (fixed)
   - Max Framerate: 30fps (fixed)
   - Prevents WebRTC auto-adaptation

2. **Consumer Bitrate Lock**
   - Max Bitrate: 6Mbps
   - Min Bitrate: 3Mbps
   - Preferred Layers: Spatial=0, Temporal=0

3. **TCP Disabled**
   - Only UDP for LAN
   - No TCP fallback (prevents oscillation)

4. **Simulcast Disabled**
   - Single layer only
   - No layer switching

---

## 🚀 Deployment Flow

### Build Process

```
1. Build Teacher App
   ├─► npm run build:teacher
   ├─► Vite builds React app → dist/
   ├─► Tauri builds → src-tauri/target/release/bundle/macos/
   └─► Result: Screen Sharing Teacher.app

2. Build Student App
   ├─► npm run build:student
   ├─► Vite builds React app → dist-student/
   ├─► Tauri builds → src-tauri/target/release/bundle/macos/
   └─► Result: Screen Sharing Student.app

3. Prepare Binaries
   ├─► npm run prepare:binaries
   ├─► Build mediasoup-server TypeScript → dist/
   ├─► Copy binaries to src-tauri/binaries/
   └─► Server binaries bundled with app
```

### Runtime Flow

```
Teacher App Start
    │
    ├─► User clicks "Khởi động server"
    │   └─► Tauri Rust backend spawns mediasoup-server process
    │       └─► Server runs in background (headless)
    │
    ├─► Server listens on ws://localhost:3016
    │
    └─► Teacher can now connect to server

Student App Start
    │
    ├─► Connects to ws://localhost:3016 (assumes server running)
    │
    └─► Can join room and receive streams
```

---

## 📝 Key Files

### Frontend (React)
- `src/hooks/useMediasoup.ts` - Main React hook for mediasoup
- `src/lib/mediasoup-client.ts` - Low-level mediasoup client
- `src/components/TeacherView.tsx` - Teacher UI
- `src/components/StudentView.tsx` - Student UI

### Backend (Mediasoup Server)
- `mediasoup-server/src/index.ts` - Entry point
- `mediasoup-server/src/SignalingServer.ts` - WebSocket signaling
- `mediasoup-server/src/MediasoupManager.ts` - Mediasoup operations
- `mediasoup-server/src/Room.ts` - Room management
- `mediasoup-server/src/config.ts` - Configuration

### Backend (Tauri Rust)
- `src-tauri/src/lib.rs` - Server process management
- `src-tauri/src/main.rs` - Teacher app entry
- `src-tauri/src/main_student.rs` - Student app entry

---

## 🎯 Summary

**Teacher:**
1. Khởi động server
2. Join room với `isTeacher: true`
3. Chia sẻ màn hình → Creates Producers
4. Bật microphone → Creates Audio Producer
5. Students tự động nhận streams

**Student:**
1. Join room với `isTeacher: false`
2. Tự động consume existing producers
3. Nhận new producers qua `newProducer` event
4. Hiển thị video/audio trong VideoPlayer

**Mediasoup Server:**
- SFU architecture (1-to-many)
- Router forwards single stream to all consumers
- All media encrypted via DTLS
- Optimized for LAN (UDP only, fixed bitrate)
