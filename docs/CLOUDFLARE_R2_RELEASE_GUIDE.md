# Hướng dẫn Phát hành & Cập nhật Tự động qua Cloudflare R2 (Tauri v2)

Tài liệu này hướng dẫn toàn bộ quy trình phát hành bản cập nhật (Auto-Update) cho ứng dụng **RustSale CRM** sử dụng **Tauri v2 + Cloudflare R2** và chữ ký số **Ed25519**.

---

## 1. Tổng quan Kiến trúc

- **Backend / Tauri:** `tauri-plugin-updater` xác thực chữ ký mã hóa Ed25519 trước khi tải và cài đặt.
- **Frontend / Svelte 5:** `updaterService.ts` quản lý kiểm tra nền, thanh tiến trình tải (`%`), và hộp thoại cập nhật `UpdateModal.svelte`.
- **Hạ tầng phân phối:** **Cloudflare R2 Bucket** (`rustsale-releases` qua CDN `pub-6f14359d39ae4d50ac90b7e759b6de56.r2.dev`).

---

## 2. Quy trình Phát hành Toàn diện 1 Lệnh (All-in-One Command)

Chúng tôi đã tích hợp toàn bộ quy trình: **Nâng version ➔ Build Cloak Runtime ➔ Build Frontend ➔ Build Tauri Bundle (ký số) ➔ Tạo `latest.json` ➔ Tự động Upload lên Cloudflare R2** vào 1 lệnh duy nhất!

### Cách sử dụng:

```bash
# 1. Phát hành bản hiện tại kèm Changelog:
./scripts/release.sh --notes "Vá lỗi đồng bộ tin nhắn và tối ưu Google Maps scraper"

# 2. Hoặc vừa nâng version vừa phát hành:
./scripts/release.sh --version 0.2.0 --notes "Phát hành tính năng Zalo CRM và Campaign mới"

# 3. Hoặc dùng qua bun run:
bun run release --version 0.2.0 --notes "Changelog nội dung cập nhật"
```

### Các tùy chọn nâng cao (Flags):
- `--version <x.y.z>` hoặc `-v`: Tự động nâng version trong `package.json` và `src-tauri/tauri.conf.json`.
- `--notes "<Changelog>"` hoặc `-n`: Nội dung thông báo cập nhật gửi đến người dùng.
- `--skip-build`: Bỏ qua bước build nếu bạn đã chạy `tauri build` trước đó (chỉ tạo manifest và upload).
- `--dry-run`: Build và tạo manifest mà không upload lên Cloudflare R2.

---

## 3. Cấu hình Khóa Ký & Môi trường

Các thông tin đã được cấu hình tự động trong `.env` và `src-tauri/tauri.conf.json`:
- **Private Key:** `~/.tauri/rustsale.key` (Dùng để ký file binary khi build).
- **Public Key:** Đã nhúng sẵn trong `src-tauri/tauri.conf.json`.
- **Endpoint Manifest:** `https://pub-6f14359d39ae4d50ac90b7e759b6de56.r2.dev/latest.json`.

---

## 4. Trải nghiệm phía Khách hàng (Client App)

1. Khi khách hàng mở app RustSale CRM:
   - App tự động check ngầm `https://pub-6f14359d39ae4d50ac90b7e759b6de56.r2.dev/latest.json`.
   - Nếu có bản mới: Hiển thị thông báo Toast góc trên cùng và biểu tượng Sparkles ở Sidebar.
2. Khách hàng nhấn **"Cập nhật ngay"**:
   - Modal hiển thị chi tiết Release Notes, % tải về, dung lượng MB.
   - Khi tải xong: Nhấn **"Khởi động lại ngay"** để hoàn tất cài đặt phiên bản mới.
3. Dữ liệu SQLite `rustsale.db` được bảo toàn nguyên vẹn trong thư mục `%APPDATA%/com.rustsale.crm/`.
