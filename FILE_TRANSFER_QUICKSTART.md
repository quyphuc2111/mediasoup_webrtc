# File Transfer - Hướng dẫn nhanh

## Gửi file cho học sinh

1. **Chọn học sinh** từ danh sách ở trên
2. Click button **"📤 Chọn file và gửi"**
3. Hộp thoại chọn file của hệ thống sẽ mở ra
4. Chọn file bạn muốn gửi
5. File sẽ được gửi tới máy học sinh

## Nhận file từ học sinh

1. **Chọn học sinh** từ danh sách ở trên
2. Duyệt file của học sinh ở bên phải (click "Tải thư mục home")
3. Click vào file muốn nhận để chọn
4. Click button **"📥 Nhận file từ học sinh"**
5. Hộp thoại lưu file sẽ mở ra
6. Chọn nơi lưu file
7. File sẽ được tải về máy bạn

## Duyệt file

### Bên giáo viên (trái):
- Click **🏠 Home**, **🖥️ Desktop**, **📄 Documents** để truy cập nhanh
- Click **📁** để mở hộp thoại chọn thư mục
- Click **⬆️** để quay lại thư mục cha
- Click vào thư mục để mở
- Click vào file để xem thông tin

### Bên học sinh (phải):
- Click "Tải thư mục home" để bắt đầu
- Navigate tương tự như bên giáo viên

## Lưu ý

⚠️ **Quan trọng:**
- Học sinh phải chạy Student Agent và đã kết nối
- Chỉ học sinh đang "Connected" mới hiển thị trong danh sách
- File lớn có thể mất thời gian để transfer

## Tính năng hiện tại

✅ **Đã hoàn thành:**
- Hộp thoại chọn file hệ thống (native file picker)
- Hộp thoại lưu file (native save dialog)
- Duyệt thư mục trên máy giáo viên
- Quick access buttons (Home, Desktop, Documents)
- Hiển thị thông tin file (tên, kích thước, ngày sửa)
- Chọn học sinh từ danh sách connected

⏳ **Đang phát triển:**
- Transfer file qua WebSocket (hiện tại chỉ là UI demo)
- Duyệt file trên máy học sinh từ xa
- Progress bar khi transfer
- Transfer nhiều file cùng lúc

## Troubleshooting

### Không thấy học sinh trong danh sách
→ Kiểm tra học sinh đã chạy Student Agent và kết nối chưa

### Hộp thoại không mở
→ Kiểm tra quyền truy cập file của ứng dụng

### Lỗi khi đọc file
→ Kiểm tra quyền đọc file và đường dẫn

## Demo

```
Giáo viên:
1. Mở File Transfer page
2. Chọn "Nguyễn Văn A" từ danh sách
3. Click "📤 Chọn file và gửi"
4. Chọn "bai_tap.pdf" từ Desktop
5. File được gửi!

Học sinh:
→ Nhận file "bai_tap.pdf" vào thư mục Downloads
```
