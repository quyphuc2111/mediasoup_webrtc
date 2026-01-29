# Luồng hoạt động: Xem và Điều khiển Màn hình (Smartlab)

Tài liệu này mô tả chi tiết luồng dữ liệu và tương tác giữa Giáo viên (Teacher) và Học sinh (Student Agent).

## 1. Tổng quan Kiến trúc

```mermaid
graph TD
    subgraph Teacher [Giáo viên / View Client]
        T_UI["Giao diện React"]
        T_WebCodecs["WebCodecs API<br/>(Giải mã H.264)"]
        T_Input["Bắt sự kiện Chuột/Phím"]
    end

    subgraph Network [Mạng LAN]
        WS["WebSocket Connection<br/>(TCP Port 3017)"]
        UDP["UDP Broadcast<br/>(Discovery)"]
    end

    subgraph Student [Học sinh / Agent]
        S_Agent["Student Agent<br/>(Rust WebSocket Server)"]
        S_Xcap["Xcap Crate<br/>(Chụp màn hình)"]
        S_H264["OpenH264<br/>(Nén Video)"]
        S_Enigo["Enigo Crate<br/>(Điều khiển IO)"]
    end

    %% Discovery
    T_UI -.->|1. Quét LAN UDP| S_Agent
    S_Agent -.->|2. Phản hồi IP| T_UI

    %% Connection
    T_UI ==>|3. Kết nối & Xác thực| WS
    WS ==> S_Agent

    %% Streaming Flow
    S_Xcap -->|Raw RGBA| S_H264
    S_H264 -->|H.264| S_Agent
    S_Agent -->|Video Frame Binary| WS
    WS -->|Video Frame| T_UI
    T_UI -->|Encoded Chunk| T_WebCodecs
    T_WebCodecs -->|Video Frame| T_UI

    %% Control Flow
    T_Input -->|Input Event JSON| WS
    WS -->|Input Event| S_Agent
    S_Agent -->|Thực thi| S_Enigo
```

## 2. Biểu đồ Tuần tự (Sequence Diagram)

```mermaid
sequenceDiagram
    participant T as Teacher (View Client)
    participant S as Student Agent (Rust)
    participant OS as Student OS

    Note over T, S: Giai đoạn 1: Tìm kiếm & Kết nối

    T->>S: UDP Broadcast (Who is there?)
    S-->>T: UDP Response (I am Student A, IP: 192.168.1.5)
    
    T->>S: WebSocket Connect (ws://192.168.1.5:3017)
    S-->>T: Connection Accepted
    
    Note over T, S: Giai đoạn 2: Xác thực (Ed25519)
    
    S->>T: Welcome { challenge: "random_string" }
    T->>T: Ký challenge bằng Private Key
    T->>S: AuthResponse { signature: "..." }
    S->>S: Verify Signature với Public Key
    S-->>T: AuthSuccess

    Note over T, S: Giai đoạn 3: Chia sẻ màn hình (Streaming)

    S->>OS: Capture Screen (xcap)
    OS-->>S: Raw Frame (RGBA)
    S->>S: Encode to H.264 (OpenH264)
    S->>T: WS Message: Binary Frame [H.264 Data]
    T->>T: Decode (WebCodecs) & Render

    Note over T, S: Giai đoạn 4: Điều khiển từ xa (Remote Control)

    T->>T: User di chuột / gõ phím
    T->>S: WS Message: MouseInput { x: 0.5, y: 0.5 }
    S->>OS: Di chuyển chuột thật (Enigo)
    
    T->>T: User click chuột trái
    T->>S: WS Message: MouseInput { action: "click" }
    S->>OS: Click chuột thật (Enigo)
```
