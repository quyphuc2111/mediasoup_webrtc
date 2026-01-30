# Workflow Optimization - Giải thích về node_modules

## Câu hỏi: Có cần copy node_modules không?

### ✅ CẦN THIẾT - Phải copy cả 3:
1. `dist/` - JavaScript đã build
2. `node_modules/` - Dependencies
3. `package.json` - Metadata

## Tại sao?

### 1. Không có binary .exe
Code hiện tại chạy:
```rust
// src-tauri/src/lib.rs
let server_path = get_resource_path(app, "binaries/server/dist/index.js");
command.arg(&server_path); // node dist/index.js
```

→ Chạy Node.js script, KHÔNG phải binary!

### 2. dist/index.js cần node_modules
File `dist/index.js` vẫn có:
```javascript
import mediasoup from 'mediasoup';
import WebSocket from 'ws';
import { v4 as uuidv4 } from 'uuid';
```

→ Cần `node_modules/` để resolve các import này!

### 3. mediasoup có native bindings
```
node_modules/
  └── mediasoup/
      └── worker/
          └── Release/
              └── mediasoup-worker.exe  ← Native binary!
```

→ Không thể bundle với `pkg` hoặc `nexe`!

## Kích thước

### Hiện tại:
```
binaries/
  ├── node/           (~50MB)
  │   └── node.exe
  └── server/         (~150MB)
      ├── dist/       (1MB)
      ├── node_modules/ (148MB)
      │   └── mediasoup/ (140MB - chủ yếu là native binaries)
      └── package.json (1KB)
```

**Tổng: ~200MB**

### Nếu bỏ node_modules:
❌ **App sẽ crash** khi start server:
```
Error: Cannot find module 'mediasoup'
```

## Có thể tối ưu không?

### Option 1: Production dependencies only ✅
Hiện tại copy cả `devDependencies`. Có thể tối ưu:

```yaml
- name: Install mediasoup-server dependencies (production only)
  run: npm ci --production
  working-directory: mediasoup-server
```

**Tiết kiệm:** ~10MB (bỏ tsx, typescript, @types/*)

### Option 2: Bundle với webpack/esbuild ❌
```bash
# Thử bundle
npx esbuild dist/index.js --bundle --platform=node --outfile=bundle.js
```

**Vấn đề:**
- mediasoup có native bindings
- Phải copy `mediasoup-worker.exe` riêng
- Phức tạp hơn, không đáng

### Option 3: Dùng pkg ❌
```bash
pkg dist/index.js --targets node20-win-x64
```

**Vấn đề:**
- mediasoup native bindings không hoạt động
- Cần patch và copy worker binary riêng
- Không stable

### Option 4: Giữ nguyên ✅ (Khuyến nghị)
**Lý do:**
- Đơn giản, stable
- mediasoup cần native binaries
- 200MB là chấp nhận được cho desktop app
- Không cần optimize quá mức

## So sánh với các app khác

| App | Size | Lý do |
|-----|------|-------|
| VS Code | ~300MB | Electron + extensions |
| Discord | ~150MB | Electron |
| Zoom | ~200MB | Native + codecs |
| **SmartlabPromax** | **~200MB** | **Node.js + mediasoup** |

→ Kích thước hợp lý!

## Kết luận

### ✅ Workflow hiện tại là ĐÚNG:
```yaml
- name: Prepare sidecar
  run: |
    mkdir -p src-tauri/binaries/server
    cp -r mediasoup-server/dist src-tauri/binaries/server/
    cp -r mediasoup-server/node_modules src-tauri/binaries/server/
    cp mediasoup-server/package.json src-tauri/binaries/server/
```

### Tối ưu nhỏ (optional):
```yaml
- name: Install mediasoup-server dependencies
  run: npm ci --production  # ← Thêm --production
  working-directory: mediasoup-server
```

**Tiết kiệm:** ~10MB
**Trade-off:** Không đáng kể

### ❌ KHÔNG nên:
- Bỏ node_modules → App crash
- Bundle với pkg/nexe → Không hoạt động với mediasoup
- Tối ưu quá mức → Phức tạp, dễ lỗi

## Giải thích cho team

**Q: Tại sao phải copy node_modules?**
A: Vì app chạy Node.js script (`node dist/index.js`), không phải binary. Script cần `require('mediasoup')` từ node_modules.

**Q: Có thể bundle thành 1 file .exe không?**
A: Không, vì mediasoup có native bindings (mediasoup-worker.exe). Phải giữ nguyên cấu trúc.

**Q: 200MB có quá lớn không?**
A: Không, so với các app tương tự (VS Code 300MB, Discord 150MB). Đây là kích thước bình thường cho desktop app có WebRTC.

**Q: Có cách nào giảm size không?**
A: Có thể dùng `npm ci --production` để bỏ devDependencies, tiết kiệm ~10MB. Nhưng không đáng kể.

## Recommendation

**Giữ nguyên workflow hiện tại!** ✅

Nếu muốn tối ưu nhỏ, thêm `--production`:
```yaml
- name: Install mediasoup-server dependencies
  run: npm ci --production
  working-directory: mediasoup-server
```

Nhưng không bắt buộc. Workflow hiện tại hoạt động tốt và đơn giản.
