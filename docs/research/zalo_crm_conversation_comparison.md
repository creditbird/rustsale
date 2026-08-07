# Báo cáo Phân tích & So sánh Conversation Module: ZaloCRM vs RustSale_v2

## 1. Mục đích
Tài liệu này cung cấp cái nhìn chi tiết về cách quản lý hội thoại, UI/UX chat, tích hợp CRM và Zalo giữa hệ thống **ZaloCRM** hiện hành và kiến trúc mới của **RustSale_v2**, đối chiếu với tiêu chuẩn Zalo thực tế. Mục tiêu là tìm ra các điểm cần hoàn thiện của RustSale_v2.

## 2. Phân tích ZaloCRM
Dựa trên mã nguồn tại `/home/arch/Project/test/ZaloCRM` (Cụ thể: `backend/prisma/schema.prisma` và frontend `src/views/ChatView.vue`), ZaloCRM cung cấp các tính năng chuyên sâu:
* **Giao diện Chat (UI/UX):** Bố cục 4 cột linh hoạt (Sidebar Filter -> Chat List -> Thread -> Contact Info). Cột info hiển thị chân dung khách hàng, timeline hoạt động, nhãn Zalo, CRM Tags.
* **Tích hợp CRM:**
    * Chấm điểm tiềm năng (Lead Scoring / Priority Score / Engagement Heatmap).
    * Hỗ trợ Gắn nhãn (Tags Taxonomy V2) tự động và thủ công.
    * Gộp/phân bổ leads (Lead Pool FIFO, Merge contacts, Child-Parent contact link).
    * Bảng Info Panel bên phải chi tiết: ghi chú, hồ sơ, theo dõi trạng thái.
* **Xử lý hội thoại:** 
    * "Inbox Triage": phân tab "Ưu tiên", "Chưa đọc", "Kẹt", "Sẵn sàng chốt".
    * Đa tài khoản Zalo (Multi-account): gom chung nhiều tài khoản Zalo, hiển thị trạng thái "OutOfScope", chuyển nhanh giữa các tài khoản.
    * Automation & AI: Phân tích sentiment, AI Summary, AI Suggestion. Cảnh báo khách hàng bị bỏ quên (Stuck Leads).

## 3. Phân tích RustSale_v2
Mã nguồn tại `/home/arch/Project/test/RustSale_v2` (`src-tauri/src/conversation` và `src/lib/components/conversation`):
* **Kiến trúc dữ liệu:** Xây dựng đa kênh (Zalo, Whatsapp, Messenger, Email). Schema chuẩn hóa qua `channel_accounts`, `messages`, `channel_conversations`. Xử lý Local SQLite mạnh mẽ trên Tauri.
* **Khả năng Chat & Hội thoại:**
    * Virtual Scrolling (Timeline): Hiển thị tin nhắn mượt mà (có sử dụng virtual scroll window).
    * Message Pipeline: Xử lý Outbox (`message_outbox_jobs`), Sync Checkpoint (`channel_sync_checkpoints`), Batch forwarding, Attachment staging. Đảm bảo tính nhất quán (robust) của việc gửi/nhận kể cả khi rớt mạng.
    * UI Components: `MessageTimeline`, `MessageBubble`, `ReactionMutationState`, `QuickTemplateManagerDialog`.
* **Tích hợp CRM cơ bản:** `LinkedLeadActions.svelte` và `UnknownContactActions.svelte` quản lý thông tin Lead, nhưng mới dừng ở mức liên kết cơ bản.

## 4. Đối chiếu tiêu chuẩn Zalo Chat vs RustSale_v2
* **Các chuẩn Zalo hiện có trong RustSale_v2:**
  - Hỗ trợ hiển thị rich media (Image, Video, Audio, Link, File, Sticker).
  - Thu hồi (Recall) / Xoá (Delete) / Chuyển tiếp (Forward) tin nhắn.
  - Reactions: thả emoji (`ReactionPicker`, `ReactionSummary`).
  - Phản hồi trích dẫn (Reply), Mentions, Sticker Picker tích hợp.
* **Các chuẩn Zalo CẦN BỔ SUNG / CẢI THIỆN trong RustSale_v2:**
  - **Voice Message Recording:** Cần thêm tính năng bấm giữ ghi âm tại `ComposerToolbar.svelte`. (Hiện đã có render AudioMessageContent nhưng chưa rõ UI tạo voice).
  - **Ghim tin nhắn (Pin Message):** Thiếu schema lưu trữ tin nhắn ghim (Pinned Messages) của Zalo.
  - **Tìm kiếm tin nhắn trong hội thoại:** RustSale_v2 hiện mới có ô search ở danh sách hội thoại (`ConversationSidebar.svelte`), chưa có công cụ "Ctrl+F" tìm nội dung tin nhắn trong luồng chat hiện tại.
  - **Đồng bộ trạng thái đọc/nhận (Read Receipts):** Schema có `message_receipts`, nhưng UI cần render 2 dấu tick xanh (Đã xem), tick xám (Đã nhận) giống chuẩn Zalo để sale nhận biết.

## 5. So sánh 3 góc độ: ZaloCRM vs RustSale_v2

### A. ZaloCRM vs RustSale_v2
* **Thế mạnh ZaloCRM:** Trải nghiệm Sales-Centric với 4 cột phân luồng thông minh (Filter -> Danh sách -> Chat -> Info Khách Hàng). Bộ lọc mạnh (Inbox Triage, Priority Score).
* **Thế mạnh RustSale_v2:** Kiến trúc Desktop (Tauri + Rust + Svelte) tốc độ cực cao, xử lý đa kênh (omni-channel) thay vì chỉ Zalo, quản lý Queue/Outbox/Offline-first hoàn hảo bằng Local DB.
* **Khoảng trống RustSale_v2:** Chưa có Dashboard 4 cột tối ưu cho Sales như `ChatView.vue`. Giao diện Lead Info Panel bên phải chưa đủ sâu (thiếu Heatmap, Lead Scoring, Deals, Auto Tags).

### B. UI/UX: Điểm CẦN CẢI THIỆN trên RustSale_v2
1. **Kiến trúc UI / Layout:** Cần học hỏi `ChatView.vue` của ZaloCRM để xây dựng bố cục: Sidebar (Filter/Tags) -> Conversation List -> Chat Timeline -> Contact/Lead Panel.
2. **Quản lý Lead (CRM Panel):** Cần chuyển đổi `LinkedLeadActions` thành một Right-Sidebar thực thụ hiển thị: Lịch sử gọi, Lịch hẹn (Appointments), Mức độ nhiệt tình (Scoring).
3. **Inbox Triage (Lọc thông minh):** RustSale_v2 cần bộ lọc nhanh "Chưa đọc", "Chưa trả lời", "Ưu tiên" thay vì chỉ search text.
4. **Hiệu năng Render:** File `schema.sql` cho thấy RustSale xử lý dữ liệu chuẩn, nhưng khi nối thêm bộ lọc CRM phức tạp (Tag, Trạng thái Lead), cần đánh index cẩn thận trên SQLite để không bị nghẽn (hiện `idx_conversations_last_message` là cơ bản, chưa cover case filter complex).
5. **Trải nghiệm gõ Chat:** Bổ sung thu âm trực tiếp (Voice record) và paste image từ clipboard vào composer một cách mượt mà nhất.

## 6. Kết luận
Dự án **RustSale_v2** có backend Tauri core rất vững chắc, khả năng mở rộng đa kênh và offline-first là lợi thế tuyệt đối so với nền web của ZaloCRM. Tuy nhiên, phần giao diện frontend cần tham khảo kỹ kiến trúc 4 cột, các tính năng CRM nâng cao (Lead Score, Automation) và UX chi tiết (Pin, Ghi âm, Trạng thái tin) từ **ZaloCRM** để trở thành một hệ thống bán hàng hoàn hảo.
