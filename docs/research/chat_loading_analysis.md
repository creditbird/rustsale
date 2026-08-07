# Phân tích Cơ chế Load Chat trong RustSale_v2 so với Chuẩn Production (WhatsApp / Telegram)

## 1. Khám phá Codebase Hiện tại (`RustSale_v2`)

### Backend (Rust)
- **API Fetching Messages**: Được xử lý tại [`src-tauri/src/conversation/repository.rs`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/repository.rs#L1575), trong hàm `get_conversation_timeline`.
- **Pagination & Limits**: Truy vấn SQL sử dụng Cursor-based pagination. Cursor bao gồm `occurred_at` (timestamp) và `m.id` (ID tin nhắn). Giới hạn số lượng tin nhắn load mỗi lần được kẹp an toàn: `let clamped_limit = limit.clamp(1, 100);`.
- **Query Engine**: Sử dụng `ORDER BY occurred_at DESC, m.id DESC` và `LIMIT ?` để lấy danh sách tin nhắn ngược về quá khứ an toàn không bị sót hay trùng lặp.

### Frontend (SvelteKit)
- **API Call & State**: Xử lý trong [`src/lib/components/conversation/ConversationWorkspace.svelte`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationWorkspace.svelte#L455). Khi click vào hội thoại, gọi hàm `fetchVariantFirstPage`, dùng hàm `getConversationTimeline` với `limit: 50`.
- **Render List**: [`src/lib/components/conversation/ConversationTimeline.svelte`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationTimeline.svelte) hiển thị danh sách tin nhắn, sử dụng kỹ thuật windowing để quản lý DOM.

### UI Scroll Behavior
- **Scroll to Bottom**: Hàm `scrollToBottom()` được gọi mặc định khi mở hội thoại ([`ConversationWorkspace.svelte:L496`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationWorkspace.svelte#L496)) và khi có tin nhắn mới mà người dùng đang cuộn gần cuối (`isNearBottom()`, [`ConversationTimeline.svelte:L166`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationTimeline.svelte#L166)).
- **Scroll UP (Load Older)**: Khởi chạy qua `IntersectionObserver` ở top sentinel element ([`ConversationTimeline.svelte:L103`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationTimeline.svelte#L103)). Hàm `handleLoadOlder` ([`ConversationWorkspace.svelte:L510`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationWorkspace.svelte#L510)) sẽ load thêm 50 tin nhắn với `nextCursor`.

---

## 2. Phân tích Hiện trạng RustSale_v2

- **Giới hạn số lượng load**: Hệ thống load **50 tin nhắn mới nhất mỗi lần** (không load tất cả), tối ưu tốt băng thông và tốc độ phản hồi.
- **Scroll tự động**: Hệ thống auto-scroll xuống cuối một cách mượt mà nhờ hàm `tick()` của Svelte sau khi dữ liệu render.
- **Hiệu năng DOM (Virtualization/Windowing)**: Có sử dụng kỹ thuật giới hạn số lượng phần tử DOM render cùng lúc (`MAX_DOM_ITEMS = 160`), kỹ thuật Windowing/Virtualization qua biến `virtualWindow` ([`ConversationTimeline.svelte:L69`](file:///home/arch/Project/test/RustSale_v2/src/lib/components/conversation/ConversationTimeline.svelte#L69)).
- **Đánh giá Production Ready**: Cơ chế hiện tại đã đạt chuẩn production (rất tốt). Quản lý memory và chống lag DOM nhờ `MAX_DOM_ITEMS` giúp duy trì mượt mà ngay cả với hội thoại có hàng nghìn tin nhắn. 

---

## 3. So sánh Chi tiết với Kiến trúc WhatsApp Web / Telegram / Slack

| Tiêu chí | WhatsApp Web / Telegram / Slack | RustSale_v2 | Trạng thái |
| :--- | :--- | :--- | :--- |
| **Phân trang (Pagination)** | Cursor-based pagination (Limit ~20-50 msgs/lần, theo `timestamp` + `id`). | Cursor-based pagination (`occurred_at` + `id`), limit = 50 msgs. | ✅ Đã chuẩn Production |
| **Tải ban đầu khi click** | Chỉ tải 20-50 tin nhắn mới nhất, cuộn ngay xuống bottom (hoặc tin chưa đọc đầu tiên). | Tải 50 tin nhắn mới nhất và cuộn xuống bottom bằng `scrollToBottom()`. | ✅ Đã chuẩn Production |
| **Tải tin nhắn cũ (Scroll Up)** | Dùng IntersectionObserver / Scroll listener ở đỉnh danh sách, fetch batch tiếp theo và **prepend**. | Dùng `IntersectionObserver` ở top sentinel, gọi `handleLoadOlder` với `nextCursor`. | ✅ Đã chuẩn Production |
| **Scroll Anchoring** | Giữ nguyên vị trí viewport của tin nhắn hiện tại khi prepend tin nhắn cũ (không bị nảy scroll). | Implement `captureAnchor()` và `restoreAnchor()` trong `ConversationTimeline.svelte`. | ✅ Đã chuẩn Production |
| **DOM Windowing / Virtual List** | Unmount các DOM Node đã cuộn xa khỏi viewport (chỉ giữ ~150-300 nodes trên DOM tree). | Dùng Windowing với `MAX_DOM_ITEMS = 160` và điều chỉnh `windowStart` khi cuộn. | ✅ Đã chuẩn Production |
| **Real-time Updates** | Append tin nhắn mới vào danh sách. Auto-scroll xuống dưới nếu người dùng ở near bottom. | Invalidation & append tin mới + auto-scroll khi `isNearBottom()`. | ✅ Đã chuẩn Production |

---

## 4. Kết luận

**Ứng dụng của bạn KHÔNG load toàn bộ nội dung chat một lúc**, mà làm theo đúng chuẩn Production khắt khe nhất hiện nay (giống như WhatsApp Web / Telegram Web):
1. Khi click vào một hội thoại, ứng dụng **chỉ load 50 tin nhắn mới nhất**.
2. Ô hội thoại bên phải lập tức render 50 tin nhắn này và **tự động scroll xuống tin nhắn sau cùng (tin mới nhất)**.
3. Khi người dùng vuốt/cuộn lên trên đỉnh (top), ứng dụng mới lazy-load tiếp 50 tin nhắn cũ hơn qua Cursor Pagination và sử dụng **Scroll Anchoring** để màn hình không bị nhảy.
4. Có sẵn cơ chế **DOM Windowing (`MAX_DOM_ITEMS = 160`)** giúp ứng dụng chạy cực kỳ mượt mà, không bao giờ lo giật lag hay tràn RAM cho dù hội thoại có hàng chục ngàn tin nhắn.
