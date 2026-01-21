# Hướng dẫn Build App cho macOS

## Prerequisites (Yêu cầu trước)

1. **Node.js** (v18 trở lên) - [Download](https://nodejs.org/)
2. **Rust** và **Cargo** - Cài đặt: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
3. **Tauri CLI** - Cài đặt: `npm install -g @tauri-apps/cli`
4. **Xcode Command Line Tools** - Cài đặt: `xcode-select --install`

## Cài đặt Dependencies

```bash
# Cài đặt dependencies cho frontend
npm install

# Cài đặt dependencies cho mediasoup server
npm run server:install
```

---

## Build Teacher App (Ứng dụng Giáo viên)

Teacher app có chức năng quản lý server và chia sẻ màn hình.

### Cách 1: Build tự động (Khuyên dùng)

```bash
npm run build:teacher
```

Lệnh này sẽ tự động:
1. Build frontend teacher (`dist/`)
2. Build Tauri teacher app

### Cách 2: Build thủ công

#### Bước 1: Build Frontend Teacher

```bash
npm run build
```

Lệnh này sẽ tạo thư mục `dist/` chứa frontend build.

#### Bước 2: Build Tauri Teacher App

```bash
cd src-tauri
tauri build --config tauri.conf.json
```

Hoặc sử dụng npm script:

```bash
npm run tauri:build
```

### Kết quả

File `.app` sẽ được tạo tại:
```
src-tauri/target/release/bundle/macos/Screen Sharing Teacher.app
```

File `.dmg` (nếu có) sẽ được tạo tại:
```
src-tauri/target/release/bundle/dmg/
```

---

## Build Student App (Ứng dụng Học sinh)

Student app chỉ có chức năng xem màn hình, không có server management.

### Cách 1: Build tự động (Khuyên dùng)

```bash
npm run build:student-app
```

Lệnh này sẽ tự động:
1. Build frontend student (`dist-student/`)
2. Build Tauri student app

### Cách 2: Build thủ công

#### Bước 1: Build Frontend Student

```bash
npm run build:student
```

Lệnh này sẽ tạo thư mục `dist-student/` chứa frontend build cho student.

#### Bước 2: Build Tauri Student App

```bash
cd src-tauri
tauri build --config tauri.student.conf.json
```

### Kết quả

File `.app` sẽ được tạo tại:
```
src-tauri/target/release/bundle/macos/Screen Sharing Student.app
```

**Lưu ý**: Student app sử dụng `main_student.rs` và `lib_student.rs` (không có server management) đã được cấu hình sẵn trong `Cargo.toml`.

---

## Build Scripts Tự động (Đã được cấu hình sẵn)

Đã có sẵn các scripts trong `package.json` để build dễ dàng:

```bash
# Build Teacher App (frontend + Tauri)
npm run build:teacher

# Build Student App (frontend + Tauri)
npm run build:student-app

# Build cả hai app
npm run build:all
```

Các scripts này sẽ tự động:
1. Build frontend (teacher hoặc student)
2. Build Tauri app với config tương ứng

---

## Build Development Version

Để build development version (chưa được tối ưu hóa):

```bash
# Teacher
cd src-tauri
tauri build --debug --config tauri.conf.json

# Student
cd src-tauri
tauri build --debug --config tauri.student.conf.json
```

---

## Lưu ý Quan trọng

1. **Code Signing**: Trên macOS, cần code signing để app có thể chạy trên máy khác. Xem [Tauri Code Signing Guide](https://tauri.app/v1/guides/building/code-signing).

2. **Notarization**: Để distribute app qua App Store hoặc Gatekeeper, cần notarize. Xem [Tauri Notarization Guide](https://tauri.app/v1/guides/building/notarization).

3. **Bundle ID**: Đảm bảo `identifier` trong config files là unique:
   - Teacher: `com.zenadev.screensharing.teacher`
   - Student: `com.zenadev.screensharing.student`

4. **Icons**: Icons đã được config trong cả hai file config, đảm bảo các file icon tồn tại trong `src-tauri/icons/`.

---

## Troubleshooting

### Lỗi "Command not found: tauri"
```bash
npm install -g @tauri-apps/cli
```

### Lỗi "Xcode Command Line Tools not found"
```bash
xcode-select --install
```

### Lỗi "Rust not found"
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Lỗi khi build student app
Đảm bảo đã build frontend student trước:
```bash
npm run build:student
```
