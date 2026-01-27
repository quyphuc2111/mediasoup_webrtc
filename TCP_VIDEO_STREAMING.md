# TCP Video Streaming Implementation

## Overview

Đã thành công chuyển đổi kiến trúc video streaming từ **WebSocket** sang **TCP thuần túy** để tránh lỗi "Failed to parse avcC" khi truyền H.264 binary data.

## Kiến trúc mới

```
┌─────────────────────────────────────────────────────────┐
│                    STUDENT AGENT                        │
├─────────────────────────────────────────────────────────┤
│  • Port 3017: WebSocket Server (Control)               │
│    - Authentication (Ed25519)                           │
│    - Commands (RequestScreen, StopScreen)               │
│    - Status messages                                     │
│                                                          │
│  • Port 3018: TCP Server (Video Stream)                │
│    - Raw H.264 binary stream                            │
│    - Frame format: [4 bytes size][frame data]          │
└─────────────────────────────────────────────────────────┘
                          ▲  ▲
                          │  │
                    WS    │  │  TCP
                  Control │  │  Video
                          │  │
┌─────────────────────────┼──┼───────────────────────────┐
│                    TEACHER CONNECTOR                    │
├─────────────────────────┼──┼───────────────────────────┤
│  • WebSocket Client ────┘  │                           │
│  • TCP Client ─────────────┘                           │
└─────────────────────────────────────────────────────────┘
```

## Các thay đổi chính

### 1. Module mới: `video_stream.rs`

**File**: `src-tauri/src/video_stream.rs`

Cung cấp:
- `VideoFrame` struct: Cấu trúc dữ liệu cho H.264 frames
- `start_video_server()`: TCP server cho student side
- `connect_video_client()`: TCP client cho teacher side
- Protocol đơn giản: `[4 bytes: frame_size][frame_data]`

**Frame data format**:
```
[1 byte: is_keyframe (0=delta, 1=keyframe)]
[8 bytes: timestamp (little-endian)]
[4 bytes: width (little-endian)]
[4 bytes: height (little-endian)]
[2 bytes: sps_pps_length (little-endian)]
[sps_pps_length bytes: AVCC description] (chỉ cho keyframes)
[remaining: H.264 Annex-B data]
```

### 2. Student Agent Updates

**File**: `src-tauri/src/student_agent.rs`

- Added constant: `VIDEO_STREAM_PORT = 3018`
- New function: `start_screen_capture_tcp()` thay thế `start_screen_capture_direct()`
- Updated `StudentMessage::ScreenReady` để bao gồm `video_port`
- WebSocket chỉ dùng cho control messages, không còn gửi binary video data

**Workflow**:
1. Teacher kết nối qua WebSocket (port 3017)
2. Xác thực Ed25519
3. Student gửi `ScreenReady` với `video_port: 3018`
4. Student khởi động TCP video server trên port 3018
5. Student bắt đầu capture và encode H.264
6. Frames được gửi qua TCP stream (không qua WebSocket)

### 3. Teacher Connector Updates

**File**: `src-tauri/src/teacher_connector.rs`

- Updated `StudentMessage::ScreenReady` để nhận `video_port`
- Added `video_client_stop_flags` to `ConnectorState`
- Khi nhận `ScreenReady`:
  - Extract student IP từ connection ID
  - Spawn TCP client task để kết nối đến `student_ip:video_port`
  - Receive frames qua TCP và update `screen_frames`

**Workflow**:
1. Teacher nhận `ScreenReady` message qua WebSocket
2. Extract `video_port` từ message
3. Tạo TCP connection riêng đến `student_ip:video_port`
4. Nhận H.264 frames qua TCP stream
5. Convert sang `ScreenFrame` và store cho frontend

### 4. Protocol Benefits

**Ưu điểm của TCP thuần túy**:
- ✅ **Không có WebSocket framing overhead**: Raw binary data
- ✅ **Không bị lỗi encoding**: Binary data truyền trực tiếp
- ✅ **Tốc độ cao hơn**: Ít layer xử lý
- ✅ **Tách biệt control và data**: WebSocket cho control, TCP cho video
- ✅ **Fix lỗi "Failed to parse avcC"**: Do không còn encode/decode qua WebSocket

## Testing

Để test implementation mới:

1. **Start Student Agent**:
   - WebSocket server sẽ listen trên port 3017
   - Khi có teacher connect và authenticated, TCP video server sẽ tự động start trên port 3018

2. **Connect Teacher**:
   - Teacher connect qua WebSocket
   - Sau khi nhận `ScreenReady`, teacher sẽ tự động kết nối TCP video stream
   - Xem logs để confirm cả 2 connections hoạt động

3. **Kiểm tra logs**:
```
[VideoStream] TCP video server listening on port 3018
[ScreenCapture] Starting H.264 capture loop (TCP streaming)
[TeacherConnector] Screen ready from <ip>, video port: 3018
[VideoStream] Connected to video server: <ip>:3018
[VideoStream] Received 30 frames...
```

## Backward Compatibility

- Code cũ `start_screen_capture_direct()` vẫn còn nhưng không được sử dụng
- Binary WebSocket messages không còn được gửi
- Frontend cần update để handle `video_port` trong `ScreenReady` message

## Next Steps

1. **Frontend Updates**: Update ViewClient page để sử dụng frames từ TCP stream
2. **Testing**: Test với multiple students đồng thời
3. **Cleanup**: Có thể xóa code WebSocket video streaming cũ nếu TCP hoạt động tốt
4. **Performance**: Monitor CPU/bandwidth usage so với WebSocket

## Troubleshooting

**Lỗi "Failed to bind video server to 0.0.0.0:3018"**:
- Port 3018 đã được sử dụng
- Solution: Stop student agent và restart

**Lỗi "Failed to connect to video server"**:
- Kiểm tra firewall
- Đảm bảo student agent đã start TCP server
- Check student IP address đúng

**Không nhận được frames**:
- Check logs để confirm TCP connection established
- Verify H.264 encoder đang hoạt động
- Check channel không bị closed
