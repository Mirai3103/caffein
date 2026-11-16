# Caffein

Tiện ích khay hệ thống cực nhẹ cho Windows 10/11 (x64) giúp giữ máy không sleep / không tắt màn hình. Viết bằng Rust thuần Win32 (`windows-sys`), không framework UI, kích thước file `.exe` bản release ~142 KB.

## Tính năng

- **Bật/tắt nhanh:** Click trái vào icon ly cà phê ở khay hệ thống (ly đầy = đang giữ thức, ly rỗng = tắt).
- **Hai chế độ:** Bật **Giữ màn hình sáng** (đọc tài liệu/truyện) hoặc tắt để **chỉ giữ máy không sleep** (màn hình vẫn tự tắt).
- **Hẹn giờ tự tắt:** Vô hạn / 15 phút / 30 phút / 1 giờ / 2 giờ, hiển thị thời gian còn lại trên tooltip.
- **Cài đặt:** Khởi động cùng Windows (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), tự bật khi mở app, chế độ & thời lượng mặc định.
- **Nhẹ & an toàn:** Dùng `SetThreadExecutionState` + `SetTimer` của Windows (tự nhả khi thoát), single-instance, lưu cấu hình tại `%APPDATA%\Caffein\config.ini`.

## Sử dụng

1. Tải file `.zip` ở trang **Releases** và giải nén.
2. Chạy `caffein.exe` (không cần cài đặt).

## Build từ mã nguồn

Yêu cầu Rust (`x86_64-pc-windows-msvc`):

```powershell
cargo test
cargo build --release
```

File thực thi nằm tại `target\release\caffein.exe`.
