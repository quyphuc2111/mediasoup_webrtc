# File Transfer - Troubleshooting

## ❌ Không thấy học sinh trong danh sách

### Nguyên nhân và giải pháp:

#### 1. Học sinh chưa kết nối
**Triệu chứng:** Danh sách học sinh trống, hiển thị "Không có học sinh nào đang kết nối"

**Giải pháp:**
1. Mở trang **"View Client"**
2. Kết nối với học sinh (click vào thumbnail học sinh)
3. Đợi status chuyển sang "Connected" hoặc "Viewing"
4. Quay lại trang **"File Transfer"**
5. Học sinh sẽ xuất hiện trong danh sách

#### 2. Học sinh đang ở trạng thái khác
**Triệu chứng:** Bạn thấy học sinh ở View Client nhưng không thấy ở File Transfer

**Giải pháp:**
- File Transfer chỉ hiển thị học sinh có status: `Connected` hoặc `Viewing`
- Nếu status là `Disconnected`, `Connecting`, hoặc `Error`, học sinh sẽ không hiển thị
- Kiểm tra status trong View Client và đảm bảo kết nối thành công

#### 3. Kiểm tra Console Log
**Cách kiểm tra:**
1. Mở DevTools (F12 hoặc Cmd+Option+I)
2. Vào tab Console
3. Tìm log: `All connections:` và `Connected students:`
4. Xem danh sách connections và status của từng học sinh

**Ví dụ log:**
```javascript
All connections: [
  { id: "192.168.1.100:8080", ip: "192.168.1.100", port: 8080, name: "Student 1", status: "Connected" },
  { id: "192.168.1.101:8080", ip: "192.168.1.101", port: 8080, name: "Student 2", status: "Viewing" },
  { id: "192.168.1.102:8080", ip: "192.168.1.102", port: 8080, name: "Student 3", status: "Disconnected" }
]
Connected students: [
  { id: "192.168.1.100:8080", ... },
  { id: "192.168.1.101:8080", ... }
]
```

## 🔄 Quy trình kết nối đúng

### Bước 1: Khởi động Student Agent (Máy học sinh)
```
1. Mở ứng dụng học sinh
2. Click "Student Agent"
3. Nhập tên học sinh
4. Click "Start Agent"
5. Đợi status: "Listening on port 8080"
```

### Bước 2: Kết nối từ giáo viên
```
1. Mở ứng dụng giáo viên
2. Click "View Client"
3. Discover hoặc thêm IP học sinh
4. Click vào thumbnail học sinh
5. Đợi authentication và kết nối
6. Status chuyển sang "Connected"
```

### Bước 3: Sử dụng File Transfer
```
1. Click "File Transfer" từ home
2. Chọn học sinh từ danh sách
3. Gửi/nhận file
```

## 🐛 Debug Steps

### 1. Kiểm tra kết nối cơ bản
```bash
# Từ máy giáo viên, ping máy học sinh
ping <student_ip>

# Kiểm tra port có mở không
telnet <student_ip> 8080
# hoặc
nc -zv <student_ip> 8080
```

### 2. Kiểm tra Student Agent
- Đảm bảo Student Agent đang chạy
- Kiểm tra port không bị conflict
- Xem log trong DebugPanel

### 3. Kiểm tra View Client
- Vào View Client
- Xem danh sách connections
- Kiểm tra status của từng học sinh
- Thử disconnect và reconnect

### 4. Kiểm tra File Transfer
- Mở Console (F12)
- Xem log `All connections:` và `Connected students:`
- Kiểm tra filter logic

## 📊 Status Codes

| Status | Ý nghĩa | Hiển thị trong File Transfer? |
|--------|---------|-------------------------------|
| `Disconnected` | Chưa kết nối | ❌ Không |
| `Connecting` | Đang kết nối | ❌ Không |
| `Connected` | Đã kết nối | ✅ Có |
| `Viewing` | Đang xem màn hình | ✅ Có |
| `Error` | Lỗi kết nối | ❌ Không |

## 🔧 Code Changes (v2)

### FileTransferPage.tsx - Updated Filter
```typescript
// OLD (chỉ filter Connected)
const connected = conns.filter(c => c.status === 'Connected');

// NEW (filter cả Connected và Viewing)
const connected = conns.filter(c => c.status === 'Connected' || c.status === 'Viewing');
```

### Added Debug Logs
```typescript
console.log('All connections:', conns);
console.log('Connected students:', connected);
```

### Added Status Display
```tsx
<span className="student-status">{student.status}</span>
```

## 💡 Tips

### Tip 1: Refresh danh sách
- File Transfer tự động refresh mỗi 2 giây
- Nếu không thấy học sinh, đợi vài giây

### Tip 2: Kiểm tra View Client trước
- Luôn kiểm tra View Client trước khi dùng File Transfer
- Đảm bảo học sinh có status "Connected" hoặc "Viewing"

### Tip 3: Sử dụng hint
- Nếu không thấy học sinh, page sẽ hiển thị hint:
  > 💡 Vào trang "View Client" để kết nối với học sinh trước

### Tip 4: Xem status badge
- Mỗi học sinh trong danh s