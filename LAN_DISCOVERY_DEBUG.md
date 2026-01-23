# HƯỚNG DẪN DEBUG LAN DISCOVERY

## Vấn đề: "Chưa có thiết bị nào. Nhấn 'Tìm kiếm thiết bị' để bắt đầu."

### Các bước kiểm tra:

## 1. Kiểm tra Học sinh (Client) đã start listener chưa

**Trên máy học sinh:**
1. Mở ứng dụng học sinh
2. Chọn chế độ **"UDP Streaming"** (không phải WebRTC)
3. Kiểm tra console logs - phải thấy:
   ```
   [Discovery] Listener started on port 5000 for device: [Tên học sinh]
   ```
4. Nếu không thấy log này → Listener chưa được start

**Cách sửa:**
- Đảm bảo đã chọn "UDP Streaming" mode
- Kiểm tra console để xem có lỗi gì không
- Thử restart ứng dụng học sinh

## 2. Kiểm tra Port

**Kiểm tra port 5000 có đang được sử dụng:**
```bash
# macOS/Linux
lsof -i :5000
# hoặc
netstat -an | grep 5000

# Windows
netstat -an | findstr 5000
```

**Nếu port đã được sử dụng:**
- Thay đổi port trong UI (ví dụ: 5001, 5002)
- Đảm bảo cả giáo viên và học sinh dùng cùng port

## 3. Kiểm tra Firewall

**macOS:**
1. System Settings > Network > Firewall
2. Đảm bảo ứng dụng có quyền nhận kết nối đến
3. Hoặc tạm thời tắt firewall để test

**Windows:**
1. Windows Defender Firewall
2. Cho phép ứng dụng qua firewall
3. Hoặc tạm thời tắt firewall để test

## 4. Kiểm tra mạng LAN

**Đảm bảo tất cả thiết bị trong cùng subnet:**
```bash
# Kiểm tra IP của giáo viên
ipconfig  # Windows
ifconfig  # macOS/Linux

# Kiểm tra IP của học sinh
# Phải cùng subnet (ví dụ: 192.168.1.x)
```

**Ví dụ:**
- ✅ Giáo viên: 192.168.1.100
- ✅ Học sinh: 192.168.1.101
- ❌ Học sinh: 192.168.2.101 (khác subnet)

## 5. Kiểm tra Console Logs

**Trên máy Giáo viên (khi nhấn "Tìm kiếm thiết bị"):**
```
[Discovery] Sending discovery broadcast to 255.255.255.255:5000 on port 5000
[Discovery] Discovery request sent, waiting for responses (timeout: 3000ms)...
[Discovery] Received response from 192.168.1.101:xxxxx: 'DISCOVERY_RESPONSE:Học sinh 1'
[Discovery] Found device: Học sinh 1 at 192.168.1.101
```

**Trên máy Học sinh (khi nhận được discovery request):**
```
[Discovery] Listener started on port 5000 for device: Học sinh 1
[Discovery] Received request from 192.168.1.100:xxxxx: 'DISCOVERY_REQUEST'
[Discovery] ✅ Responded to discovery from 192.168.1.100:xxxxx (sent XX bytes, name: Học sinh 1)
```

## 6. Các lỗi thường gặp

### Lỗi: "Port is already in use"
**Nguyên nhân:** Port 5000 đã được sử dụng bởi ứng dụng khác
**Giải pháp:** 
- Thay đổi port trong UI
- Hoặc đóng ứng dụng đang dùng port 5000

### Lỗi: "Failed to bind response socket"
**Nguyên nhân:** Không có quyền bind port
**Giải pháp:**
- Chạy ứng dụng với quyền admin (nếu cần)
- Hoặc thay đổi port

### Không nhận được response
**Nguyên nhân có thể:**
1. Học sinh chưa start listener
2. Firewall chặn UDP
3. Khác subnet
4. Router chặn broadcast

**Giải pháp:**
1. Đảm bảo học sinh đã chọn UDP mode
2. Kiểm tra firewall
3. Kiểm tra IP addresses
4. Thử tăng timeout (5000ms thay vì 3000ms)

## 7. Test thủ công

**Test 1: Kiểm tra listener đang chạy**
- Trên học sinh: Chọn UDP mode → Phải thấy log "Listener started"
- Nếu không thấy → Có lỗi khi start listener

**Test 2: Test broadcast**
- Trên giáo viên: Nhấn "Tìm kiếm thiết bị"
- Phải thấy log "Sending discovery broadcast"
- Nếu không thấy → Có lỗi khi gửi broadcast

**Test 3: Test response**
- Sau khi giáo viên gửi broadcast
- Học sinh phải thấy log "Received request"
- Học sinh phải thấy log "Responded to discovery"
- Giáo viên phải thấy log "Found device"

## 8. Workaround nếu discovery không hoạt động

**Cách 1: Nhập IP thủ công**
- Giáo viên có thể nhập IP của học sinh thủ công
- Thiết bị sẽ được lưu vào database

**Cách 2: Sử dụng WebRTC**
- Nếu UDP discovery không hoạt động, có thể dùng WebRTC mode
- WebRTC không cần discovery, chỉ cần server URL

## 9. Debug Commands

**Kiểm tra UDP port:**
```bash
# macOS
sudo lsof -i UDP:5000

# Linux
sudo netstat -ulnp | grep 5000

# Windows
netstat -an | findstr "5000"
```

**Test UDP broadcast:**
```bash
# Gửi test message (cần netcat)
echo "DISCOVERY_REQUEST" | nc -u -b 255.255.255.255 5000
```

## 10. Checklist trước khi báo lỗi

- [ ] Học sinh đã chọn "UDP Streaming" mode
- [ ] Console logs cho thấy listener đã start
- [ ] Cả hai máy trong cùng subnet (192.168.x.x)
- [ ] Firewall không chặn UDP port
- [ ] Port 5000 không bị sử dụng bởi app khác
- [ ] Đã thử tăng timeout lên 5000ms
- [ ] Đã kiểm tra console logs trên cả hai máy

---

**Nếu vẫn không hoạt động sau khi kiểm tra tất cả các bước trên, vui lòng cung cấp:**
1. Console logs từ cả giáo viên và học sinh
2. IP addresses của cả hai máy
3. Output của lệnh kiểm tra port
4. Thông tin về firewall settings
