# Caffein — thiết kế

## Mục đích
App giữ máy Windows không sleep / không khóa màn hình khi người dùng không đụng chuột:
đọc truyện dài, đang tải file, đợi agent coding chạy xong. Ưu tiên số 1: **cực nhẹ**.

## Mục tiêu
- Icon khay hệ thống, click trái = bật/tắt giữ thức.
- Hai chế độ: **giữ màn hình sáng** (đọc truyện) hoặc **chỉ giữ máy không sleep** (tải file, agent — màn hình được tắt).
- Hẹn giờ: Vô hạn / 15 phút / 30 phút / 1 giờ / 2 giờ; hết giờ tự tắt.
- Cửa sổ Settings: khởi động cùng Windows, tự bật khi mở app, chế độ mặc định, thời lượng mặc định.
- exe ~300 KB, RAM ~2 MB khi chỉ có icon.

## Không làm
- Không giả lập di chuyển chuột/bấm phím (dùng API chính thống).
- Không tự bật theo process/app, không phím tắt toàn cục, không auto-update (có thể thêm sau).
- Không framework UI.

## Anti-goal
- Thoát app / crash mà máy vẫn bị giữ thức → không được xảy ra (API tự nhả khi process chết).
- Ngốn CPU khi rảnh: không vòng lặp polling, chỉ chờ message Windows.

## Cách làm đã chọn
- **Approaches:** A) Win32 thuần (chọn — nhẹ nhất), B) Slint (đẹp hơn, ~3–5 MB), C) GPUI (đẹp kiểu Zed, 20–40 MB, nặng nhất). Người dùng chọn A.
- Rust + crate `windows-sys` duy nhất (binding Win32, không runtime).
- Giữ thức: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED [| ES_DISPLAY_REQUIRED])`; tắt = `ES_CONTINUOUS`.
- Hẹn giờ: `SetTimer` của Win32, không thread riêng.

## Các quyết định của mình (bạn sửa nếu muốn)
- **Icon:** 2 icon nhúng sẵn trong exe — ly cà phê đầy (đang bật) / ly rỗng (tắt). Tooltip ghi trạng thái + thời gian còn lại.
- **Menu chuột phải:** Bật/Tắt · Thời lượng ▸ · ☑ Giữ màn hình sáng · Settings… · Thoát.
- **Settings:** cửa sổ nhỏ, control gốc Windows, bật visual styles (manifest comctl v6) để trông giống Win10/11 chứ không phải Win95. Lưu là áp dụng.
- **Lưu config:** file `config.ini` trong `%APPDATA%\Caffein\` (tự viết parser vài dòng, không thêm crate).
- **Khởi động cùng Windows:** key `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (không cần quyền admin).
- **Một instance:** dùng named mutex; mở lần 2 thì không chạy thêm.
- **Build:** `release` với `opt-level="z"`, LTO, `panic="abort"`, strip; subsystem `windows` (không hiện console).
- Chỉ hỗ trợ Windows 10/11 x64.

## Để builder tự quyết
Bố cục pixel cửa sổ Settings, cách vẽ icon, cấu trúc module.
