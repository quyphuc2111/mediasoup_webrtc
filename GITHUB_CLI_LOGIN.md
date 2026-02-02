# 🔐 GitHub CLI Login Guide

## ✅ Đã fix permissions

Thư mục `~/.config/gh` đã được tạo với quyền đúng.

## Bước 1: Login

Chạy lệnh:

```bash
gh auth login
```

## Bước 2: Chọn options

```
? What account do you want to log into?
> GitHub.com

? What is your preferred protocol for Git operations?
> HTTPS

? Authenticate Git with your GitHub credentials?
> Yes

? How would you like to authenticate GitHub CLI?
> Login with a web browser
```

## Bước 3: Copy one-time code

Sẽ hiển thị:
```
! First copy your one-time code: XXXX-XXXX
Press Enter to open github.com in your browser...
```

**Copy code này!**

## Bước 4: Mở browser và paste code

1. Nhấn Enter để mở browser
2. Paste one-time code
3. Click "Authorize"

## Bước 5: Verify

```bash
gh auth status
```

Should show:
```
✓ Logged in to github.com as YOUR_USERNAME
✓ Git operations for github.com configured to use https protocol.
✓ Token: gho_************************************
```

## Bước 6: Test access

```bash
gh repo view quyphuc2111/mediasoup_webrtc
```

Should show repo info.

## Troubleshooting

### "Permission denied"
Already fixed! ✅

### "Browser didn't open"
Manually go to: https://github.com/login/device
Paste the code shown in terminal.

### "Token expired"
```bash
gh auth refresh
```

## Next Step

Sau khi login thành công:

```bash
./scripts/release-github.sh 0.1.0 quyphuc2111/mediasoup_webrtc "Initial release"
```

## Alternative: Use Token

Nếu không muốn dùng browser:

```bash
gh auth login --with-token
```

Paste personal access token (tạo tại: https://github.com/settings/tokens)

Scopes cần:
- `repo` (Full control of private repositories)
- `workflow` (Update GitHub Action workflows)

## Ready! 🚀

Chỉ cần chạy `gh auth login` trong terminal của bạn!
