# Hướng dẫn sử dụng chức năng File Transfer

## Tổng quan

Chức năng File Transfer cho phép giáo viên:
- 📂 Xem các thư mục và file trên máy học sinh
- ➡️ Gửi file từ máy giáo viên tới máy học sinh
- ⬅️ Nhận file từ máy học sinh về máy giáo viên

## Cách sử dụng

### 1. Mở trang File Transfer

Từ trang chủ của ứng dụng giáo viên, click vào button **"File Transfer"** trong phần "Xem và điều khiển màn hình".

### 2. Chọn học sinh

Ở phần trên cùng của trang, bạn sẽ thấy danh sách các học sinh đang kết nối. Click vào học sinh mà bạn muốn gửi/nhận file.

**Lưu ý:** Học sinh phải đang chạy Student Agent và đã kết nối với giáo viên thì mới hiển thị trong danh sách.

### 3. Duyệt file của giáo viên (bên trái)

- Bên trái màn hình hiển thị các file và thư mục trên máy giáo viên
- Mặc định sẽ mở thư mục Home của giáo viên
- Click vào thư mục để mở
- Click vào file để chọn (file được chọn sẽ có màu xanh)
- Click nút ⬆️ để quay lại thư mục cha

### 4. Gửi file cho học sinh

1. Chọn học sinh từ danh sách
2. Duyệt và chọn file muốn gửi ở bên trái
3. Click button **"➡️ Gửi file cho học sinh"**
4. File sẽ được gửi tới máy học sinh

### 5. Xem file của học sinh (bên phải)

- Bên phải màn hình hiển thị các file và thư mục trên máy học sinh
- Click button **"Tải thư mục home"** để bắt đầu duyệt file học sinh
- Tương tự như bên giáo viên, click vào thư mục để mở, click vào file để chọn

### 6. Nhận file từ học sinh

1. Chọn học sinh từ danh sách
2. Duyệt và chọn file muốn nhận ở bên phải
3. Click button **"⬅️ Nhận file từ học sinh"**
4. File sẽ được tải về máy giáo viên

## Thông tin file

Mỗi file/thư mục hiển thị:
- 📁 Icon thư mục hoặc 📄 icon file
- Tên file/thư mục
- Kích thước (đối với file)
- Ngày giờ chỉnh sửa lần cuối

## Trạng thái transfer

Khi đang gửi/nhận file, button sẽ hiển thị:
- ⏳ Đang gửi... (khi gửi file)
- ⏳ Đang nhận... (khi nhận file)

Thông báo sẽ hiển thị ở trên cùng:
- 🔵 Màu xanh: Thông tin
- 🟢 Màu xanh lá: Thành công
- 🔴 Màu đỏ: Lỗi

## Kiến trúc kỹ thuật

### Backend (Rust)

File `src-tauri/src/file_transfer.rs` cung cấp các chức năng:

```rust
// List files trong thư mục
pub fn list_directory(path: &str) -> Result<Vec<FileInfo>, String>

// Đọc file dưới dạng base64 để transfer
pub fn read_file_as_base64(path: &str) -> Result<String, String>

// Ghi file từ base64
pub fn write_file_from_base64(path: &str, data: &str) -> Result<(), String>

// Lấy thông tin file
pub fn get_file_info(path: &str) -> Result<FileInfo, String>

// Các helper functions
pub fn get_home_directory() -> Result<String, String>
pub fn get_desktop_directory() -> Result<String, String>
pub fn get_documents_directory() -> Result<String, String>
```

### Frontend (React + TypeScript)

File `src/pages/FileTransferPage.tsx` cung cấp UI:

- Danh sách học sinh đang kết nối
- 2 file browser (giáo viên và học sinh)
- Buttons để gửi/nhận file
- Hiển thị trạng thái và thông báo

### Tauri Commands

Các command được expose từ Rust sang JavaScript:

```typescript
// List directory
invoke<FileInfo[]>('list_directory', { path: '/path/to/dir' })

// Get special directories
invoke<string>('get_home_directory')
invoke<string>('get_desktop_directory')
invoke<string>('get_documents_directory')

// File operations
invoke<string>('read_file_as_base64', { path: '/path/to/file' })
invoke('write_file_from_base64', { path: '/path/to/file', data: 'base64...' })
invoke<FileInfo>('get_file_info', { path: '/path/to/file' })
```

## Phát triển tiếp

### Các tính năng cần implement:

1. **WebSocket Protocol cho File Transfer**
   - Thêm message types: `list_directory`, `send_file`, `receive_file`
   - Implement trong `teacher_connector.rs` và `student_agent.rs`

2. **Progress Bar**
   - Hiển thị tiến trình upload/download
   - Tính toán % hoàn thành

3. **Batch Transfer**
   - Gửi nhiều file cùng lúc
   - Gửi cả thư mục (zip trước khi gửi)

4. **File Permissions**
   - Kiểm tra quyền truy cập file
   - Xử lý lỗi permission denied

5. **Security**
   - Giới hạn kích thước file
   - Kiểm tra loại file (whitelist/blacklist)
   - Mã hóa file khi transfer

6. **UI Improvements**
   - Drag & drop để gửi file
   - Context menu (right-click)
   - Search/filter files
   - Sort by name/size/date

## Lưu ý bảo mật

⚠️ **Quan trọng:**
- Chức năng này cho phép giáo viên truy cập file trên máy học sinh
- Cần có sự đồng ý và giám sát phù hợp
- Nên giới hạn quyền truy cập chỉ trong thư mục cụ thể
- Cân nhắc thêm authentication và logging

## Troubleshooting

### Không thấy học sinh trong danh sách
- Kiểm tra học sinh đã chạy Student Agent chưa
- Kiểm tra kết nối mạng giữa giáo viên và học sinh
- Xem log trong DebugPanel

### Không tải được thư mục
- Kiểm tra quyền truy cập thư mục
- Thử với thư mục khác (Desktop, Documents)
- Xem thông báo lỗi chi tiết

### Lỗi khi gửi/nhận file
- Kiểm tra dung lượng đĩa còn trống
- Kiểm tra quyền ghi file
- File có thể đang được sử dụng bởi chương trình khác

## Demo Flow

```
1. Giáo viên mở File Transfer page
2. Chọn học sinh "Nguyễn Văn A" từ danh sách
3. Duyệt file bên trái, chọn "bai_tap.pdf"
4. Click "Gửi file cho học sinh"
5. File được gửi tới máy học sinh
6. Học sinh nhận file vào thư mục Downloads
7. Giáo viên click "Tải thư mục home" bên phải
8. Xem file của học sinh, chọn "bai_lam.docx"
9. Click "Nhận file từ học sinh"
10. File được tải về máy giáo viên
```

## Kết luận

Chức năng File Transfer giúp giáo viên dễ dàng chia sẻ tài liệu và thu bài tập từ học sinh. Giao diện trực quan với 2 file browser song song giúp việc quản lý file trở nên đơn giản và hiệu quả.
