# RustSale Common Patterns (CP.md)

Tài liệu chuẩn hóa các mẫu thiết kế và quy tắc kỹ thuật (Common Patterns - CP) cốt lõi trong hệ thống RustSale CRM, đặc biệt là tầng xử lý đa kênh (Omnichannel Messaging), giao thức nhà cung cấp (Zalo, WhatsApp, FB, Telegram), quản lý trạng thái tin nhắn và khả năng chịu lỗi (fault tolerance).

---

## Danh mục Common Patterns

- [CP-1: Omnichannel Monotonic Message Status Progression & Read/Delivered Sweep](#cp-1-omnichannel-monotonic-message-status-progression--readdelivered-sweep)
- [CP-2: Resilient Heterogeneous Deserialization (Number/String & Missing Field Tolerance)](#cp-2-resilient-heterogeneous-deserialization-numberstring--missing-field-tolerance)
- [CP-3: Multi-Format Event Payload Container Decoding](#cp-3-multi-format-event-payload-container-decoding)
- [CP-4: Batch Fault Isolation in Realtime Protocol Stream Ingest](#cp-4-batch-fault-isolation-in-realtime-protocol-stream-ingest)
- [CP-5: Asynchronous Outbox Execution, Network Latency Separation & Realtime Invalidation](#cp-5-asynchronous-outbox-execution-network-latency-separation--realtime-invalidation)
- [CP-6: Platform-Specific Merged Receipt Protocol Handling (Zalo / OTT Networks)](#cp-6-platform-specific-merged-receipt-protocol-handling-zalo--ott-networks)

---

### CP-1: Omnichannel Monotonic Message Status Progression & Read/Delivered Sweep

#### 1. Vấn đề (Context & Problem)
Trong hệ thống chat đa nền tảng (Zalo, WhatsApp, Facebook, Telegram...), các sự kiện biên lai (delivery/read receipts) có thể đến trễ, đến không theo thứ tự thời gian (out-of-order), hoặc nền tảng chỉ gửi biên lai cho tin nhắn mới nhất mà không gửi lại cho các tin nhắn trước đó trong cùng cuộc trò chuyện.

#### 2. Giải pháp chuẩn (Pattern Solution)
1. **Thứ bậc trạng thái đơn điệu (`status_rank`)**:
   Trạng thái tin nhắn chỉ được phép tiến lên theo thứ tự cố định, tuyệt đối không được thụt lùi:
   ```
   pending (0) < sending (10) < queued (20) < sent (30) < delivered (40) < read (50)
   ```
   Trạng thái lỗi (`failed` / `undeliverable`) có thể chuyển từ các trạng thái chưa chốt (`pending`, `sending`, `queued`).
2. **Cơ chế Quét dọn (Sweep Mechanism)**:
   Khi nhận được biên lai `delivered` hoặc `read` cho một tin nhắn mốc (`anchor_message` hoặc `anchor_cutoff_timestamp`), hệ thống tự động cập nhật toàn bộ các tin nhắn gửi đi trước đó trong cùng cuộc hội thoại lên trạng thái tương ứng nếu trạng thái hiện tại thấp hơn.
3. **Hiển thị giao diện nhất quán**:
   - `pending` / `sending` / `queued`: Biểu tượng Đồng hồ quay (`Clock animate-spin`).
   - `sent`: 1 dấu tick xám (Đã gửi tới máy chủ nền tảng).
   - `delivered`: 2 dấu tick xám (Đã nhận trên thiết bị đối phương).
   - `read`: 2 dấu tick xanh / avatar người đọc (Đã xem).

---

### CP-2: Resilient Heterogeneous Deserialization (Number/String & Missing Field Tolerance)

#### 1. Vấn đề (Context & Problem)
Các API / WebSocket của bên thứ ba (như Zalo, WhatsApp Web/Mobile) thường xuyên thay đổi hoặc trả về dữ liệu hỗn hợp (lúc là chuỗi `string`, lúc là số nguyên `u64/i64/i32`, lúc là mảng `array`, lúc là giá trị đơn lẻ). Ngoài ra, khi người dùng ở các trạng thái khác nhau (ngoài inbox vs trong phòng chat), một số trường có thể bị bỏ qua hoàn toàn (omitted).

#### 2. Giải pháp chuẩn (Pattern Solution)
1. **Sử dụng Custom Deserializer linh hoạt**:
   - `deserialize_string_or_number`: Chấp nhận cả `"123456"` và `123456`, luôn chuẩn hóa về `String`.
   - `deserialize_vec_string_or_number`: Chấp nhận cả `["1", "2"]`, `[1, 2]`, `"1"`, `1`, luôn chuẩn hóa về `Vec<String>`.
   - `deserialize_string_or_number_u64` / `deserialize_string_or_number_i32`: Chấp nhận cả số và chuỗi số, chuyển về kiểu số an toàn với fallback về `0`.
2. **Luôn bổ sung `#[serde(default)]` cho Struct & Field**:
   Mọi struct nhận sự kiện từ protocol bắt buộc phải có `#[derive(Default)]` và `#[serde(default)]` để tránh lỗi khi server nền tảng thiếu trường (omitted fields).

---

### CP-3: Multi-Format Event Payload Container Decoding

#### 1. Vấn đề (Context & Problem)
Một frame WebSocket từ đối tác có thể chứa payload dưới nhiều định dạng tùy thuộc vào phiên bản hoặc loại sự kiện:
1. Đã giải mã sẵn thành JSON Object / JSON Array ở root hoặc dưới key `.data`.
2. Chuỗi JSON thô không mã hóa (`encrypt: 0`).
3. Chuỗi Base64 / URL-encoded được mã hóa AES-GCM + nén zlib/deflate (`encrypt: 2` hoặc `3`).
4. Có hoặc không có key bọc lồng nhau `{ "data": { ... } }`.

Nếu bộ giải mã giả định cấu trúc cứng nhắc, các gói tin dạng un-nested hoặc direct object sẽ bị coi là lỗi hoặc trả về `Null`.

#### 2. Giải pháp chuẩn (Pattern Solution)
Bộ giải mã (`decode_event_data`) phải thực hiện phân nhánh theo thứ tự:
1. Kiểm tra nếu `parsed` đã chứa các mảng sự kiện nghiệp vụ (`delivereds`, `seens`, `msgs`, `groupSeens`, `controls`) trực tiếp ở cấp cao nhất -> trả về ngay.
2. Kiểm tra nếu `parsed["data"]` đã là JSON Object / Array -> mở rộng lấy data bên trong hoặc trả về trực tiếp.
3. Nếu `parsed["data"]` là string -> kiểm tra cờ `encrypt`:
   - Nếu `encrypt == 0`: parse JSON string, tự động unwrap `.data` nếu có, ngược lại trả về JSON vừa parse.
   - Nếu `encrypt > 0`: thực hiện giải mã AES-GCM, giải nén zlib/deflate, parse JSON và unwrap `.data` nếu có.

---

### CP-4: Batch Fault Isolation in Realtime Protocol Stream Ingest

#### 1. Vấn đề (Context & Problem)
Khi đối tác gửi một danh sách gồm nhiều bản ghi trong cùng một gói tin (ví dụ mảng `delivereds: [item1, item2, item3]`), nếu áp dụng toán tử `?` hoặc `panic` trên từng phần tử, chỉ cần một phần tử bị lỗi định dạng (malformed item) sẽ làm hủy toàn bộ gói tin, khiến các phần tử hợp lệ còn lại bị mất oan.

#### 2. Giải pháp chuẩn (Pattern Solution)
1. **Khử lan truyền lỗi (Fault Isolation)**:
   Xử lý từng phần tử trong mảng bằng khối `match` độc lập:
   ```rust
   for del_val in delivereds {
       match serde_json::from_value::<TDeliveredMessage>(del_val.clone()) {
           Ok(del_data) => {
               delivered_events.push(DeliveredMessage::User(UserDeliveredMessage::new(del_data)));
           }
           Err(e) => {
               log::warn!("Failed to parse TDeliveredMessage from payload {:?}: {}", del_val, e);
           }
       }
   }
   ```
2. **Phát tín hiệu sự kiện nếu có ít nhất 1 phần tử hợp lệ**:
   Chỉ cần mảng kết quả `!events.is_empty()`, hệ thống phát ngay event vào luồng pipeline mà không bị gián đoạn.

---

### CP-5: Asynchronous Outbox Execution, Network Latency Separation & Realtime Invalidation

#### 1. Vấn đề (Context & Problem)
Khi gửi tin nhắn, thời gian phản hồi từ server đối tác (Zalo/FB/WhatsApp) phụ thuộc vào đường truyền mạng (500ms - 3s). Nếu UI đợi kết quả mạng đồng bộ, trải nghiệm người dùng sẽ bị đơ, giật lag.

#### 2. Giải pháp chuẩn (Pattern Solution)
1. **Optimistic Local Enqueue**:
   UI gọi lệnh gửi -> Backend ghi ngay vào SQLite với trạng thái `pending`/`queued` và trả về ngay cho UI trong < 10ms -> UI lập tức hiển thị tin nhắn với biểu tượng đồng hồ quay.
2. **Async Outbox Job Execution**:
   Worker supervisor chạy ngầm nhận job, thực hiện gọi HTTP/Socket đến nhà cung cấp.
3. **Chốt trạng thái & Phát Invalidation Event**:
   Khi nhà cung cấp phản hồi thành công và trả về `provider_message_id`, backend gọi `complete_outbox_job` cập nhật `status = 'sent'`, đồng thời phát sự kiện `conversation-invalidated`.
4. **Debounced UI Refresh**:
   Frontend lắng nghe `conversation-invalidated`, trigger `refreshOpenTimeline` với debounce ngắn (25ms) để tải trạng thái mới nhất (`sent` - 1 tick xám).

---

### CP-6: Platform-Specific Merged Receipt Protocol Handling (Zalo / OTT Networks)

#### 1. Vấn đề (Context & Problem)
Trên một số mạng xã hội (đặc biệt là Zalo), server đôi khi gộp (merge) sự kiện nhận và xem thành một gói tin duy nhất `delivered_messages` có cờ `seen: 1` hoặc mảng `seen_uids` thay vì phát `seen_messages` riêng biệt. Ngược lại, khi người nhận chỉ ở ngoài màn hình danh sách hội thoại, Zalo phát `seen: 0` và danh sách `delivered_uids` dạng số.

#### 2. Giải pháp chuẩn (Pattern Solution)
1. **Phân loại nhánh biên lai linh hoạt**:
   ```rust
   let is_seen = input.seen == 1 || !input.seen_uids.is_empty();
   let kind = if is_seen {
       ReceiptKind::Read
   } else {
       ReceiptKind::Delivered
   };
   ```
2. **Phân giải định danh đa tầng (Multi-Tier Identity Resolution)**:
   - Thử tìm `external_thread_id` từ `thread_id`.
   - Nếu trống, lấy từ `seen_uids.first()`.
   - Nếu trống, lấy từ `delivered_uids.first()`.
3. **Phân giải ID tin nhắn kết hợp**:
   - Khớp theo `msg_id` hoặc `real_msg_id`.
   - Nếu không tìm thấy tin nhắn chính xác bằng ID (ví dụ do sai lệch ID nhà cung cấp), tự động kích hoạt **Fallback Sweep** theo `external_thread_id` và mốc thời gian sự kiện (`occurred_at`).
