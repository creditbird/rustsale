# Zalo Account Session Refactor Design Spec

**Document Version:** 1.0.0  
**Date:** 2026-08-09  
**Status:** Approved Architecture Spec  
**Target Module:** `RustSale_v2` / `conversation` / `zalo_accounts`

---

## Executive Summary

Khi người dùng thực hiện đăng nhập lại (QR Login) cùng một tài khoản Zalo trên hệ thống RustSale_v2:
1. `channel_accounts.provider_account_key` đang lưu chuỗi `IMEI` ngẫu nhiên làm khoá chính, dẫn tới việc sinh ra nhiều `channel_account_id` khác nhau cho cùng một tài khoản Zalo logic.
2. Việc tạo session mới trên Zalo khiến Zalo Server tự động vô hiệu hoá token đăng nhập cũ (`zpw_sek`), gây ra lỗi **Code 102** trên các cuộc trò chuyện cũ vốn đang gắn với `channel_account_id` (IMEI) cũ.
3. Nút `Reconnect` hiện tại chỉ kích hoạt lại supervisor dựa trên IMEI cũ nên không thể phục hồi token đã bị thu hồi.

Tài liệu này quy định kiến trúc refactor chuẩn: Chuyển `provider_account_key` sang dùng **Zalo User UID (`self_external_id`)**, gom toàn bộ các phiên đăng nhập lại của cùng một tài khoản Zalo về đúng một `ChannelAccount` duy nhất.

---

## Key Refactor Specifications

### 1. Account Key Identifier
- **Cũ:** `provider_account_key = imei` (mã thiết bị/session).
- **Mới:** `provider_account_key = self_external_id` (Zalo User UID duy nhất).

### 2. QR Login Reconciliation Flow
```text
QR Login Success 
  └── Fetch Zalo Profile (zalo.fetch_account_info())
        └── Extract self_external_id (Zalo UID)
              ├── Found existing channel_account(channel='zalo', key=zalo_uid):
              │     ├── Stop old IMEI supervisor task
              │     ├── Update active IMEI & cookies into existing channel_account
              │     └── Restart supervisor with new IMEI session
              └── Not found:
                    └── Insert new channel_account(channel='zalo', key=zalo_uid)
```

### 3. Frontend Error Code 102 UI Handling
- Khi `MessageComposer` phát hiện `lastError` có chứa `code: Some(102)`:
  - Thay thế nút `Reconnect` bằng nút `Đăng nhập lại QR` (`start_zalo_qr_login`).

### 4. Required File Changes
- `src-tauri/src/commands.rs` (dòng 962): Cập nhật luồng QR login để lấy UID & upsert account.
- `src-tauri/src/conversation/repository.rs` (dòng 2015): Đổi `provider_account_key` sang `zalo_uid`.
- `src-tauri/src/conversation/zalo/supervisor.rs` (dòng 342): Restart task theo Zalo UID.
- `src/lib/components/conversation/MessageComposer.svelte` (dòng 706 & 877): Thêm nút "Đăng nhập lại QR" khi gặp lỗi 102.
- `src-tauri/src/conversation/migration.rs`: Thêm migration script gộp các `channel_account_id` bị lặp lại.
