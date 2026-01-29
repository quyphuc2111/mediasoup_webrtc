# Giải thích Luồng Hoạt động (Sequence Diagram)

Đây là giải thích chi tiết cho từng bước trong biểu đồ tuần tự mà bạn đã cung cấp. Quy trình này mô tả cách Giáo viên kết nối, xem màn hình và điều khiển máy Học sinh.

### Giai đoạn 1: Tìm kiếm & Kết nối (Discovery & Connection)
Giai đoạn này giống như việc điểm danh trong lớp học.
1. **Teacher -> Student**: Giáo viên "hét lớn" trong mạng LAN (UDP Broadcast) câu hỏi: *"Ai đang có mặt ở đây?"*.
2. **Student -> Teacher**: Máy học sinh nghe thấy và trả lời riêng cho giáo viên: *"Em là Học sinh A, địa chỉ nhà em (IP) là 192.168.1.5"*.
3. **Teacher -> Student**: Biết địa chỉ rồi, Giáo viên gõ cửa trực tiếp nhà học sinh (Mở kết nối WebSocket TCP).
4. **Student -> Teacher**: Học sinh mở cửa cho vào (Connection Accepted).

### Giai đoạn 2: Xác thực (Authentication)
Để đảm bảo an toàn, không phải ai gõ cửa cũng được vào, cần có mật khẩu/chữ ký.
1. **Student -> Teacher**: Học sinh đưa ra một câu đố ngẫu nhiên (Challenge string). *"Thầy hãy ký tên vào tờ giấy này để chứng minh là thầy"*.
2. **Teacher -> Teacher**: Giáo viên dùng "Con dấu riêng" (Private Key) để đóng dấu vào tờ giấy đó (Ký số).
3. **Teacher -> Student**: Giáo viên gửi tờ giấy đã đóng dấu lại cho học sinh.
4. **Student -> Student**: Học sinh dùng "Mẫu chữ ký công khai" (Public Key) của thầy để đối chiếu.
5. **Student -> Teacher**: Nếu khớp, học sinh xác nhận *"Đúng là thầy rồi, mời vào"* (AuthSuccess).

### Giai đoạn 3: Chia sẻ màn hình (Streaming Loop)
Đây là vòng lặp liên tục (Loop), xảy ra khoảng 30 lần mỗi giây (30 FPS) để tạo thành video mượt mà.
1. **Student -> OS**: Agent yêu cầu hệ điều hành: *"Cho xin hình ảnh màn hình hiện tại"*.
2. **OS -> Student**: Hệ điều hành trả về hình ảnh thô (rất nặng).
3. **Student -> Student**: Agent nén hình ảnh đó lại cho nhẹ bằng chuẩn H.264 (giống nén file zip nhưng chuyên cho video).
4. **Student -> Teacher**: Gửi gói dữ liệu video đã nén qua đường dây kết nối (WebSocket).
5. **Teacher -> Teacher**: Máy giáo viên nhận được, giải nén (Decode) và vẽ lên màn hình cho giáo viên xem.

### Giai đoạn 4: Điều khiển từ xa (Remote Control)
Giai đoạn này là tùy chọn (Opt - Optional), chỉ xảy ra khi giáo viên thao tác.
*   **Chuột**: Khi giáo viên di chuột trên màn hình của mình -> Gửi tọa độ (vd: 50% ngang, 50% dọc) -> Máy học sinh nhận lệnh và di chuyển con trỏ chuột thật đến vị trí đó.
*   **Bàn phím**: Khi giáo viên nhấn phím 'A' -> Gửi lệnh "Nhấn A" -> Máy học sinh nhận lệnh và giả lập việc nhấn phím 'A' y như thật.
