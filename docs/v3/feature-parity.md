# RS: RustSale v2 → v3 — Danh sách tính năng đối chiếu (feature parity)

- **Repo:** `/home/arch/Project/test/RustSale_v3` (bản clone của v2 tại commit production `2535767`)
- **Nhánh:** `docs/v3-feature-parity`, tách từ `feat/nsh-v3` (`e9b1d32`)
- **Người lập:** Lữ Bố, 2026-11-22
- **Phạm vi:** chỉ đọc code trong repo. Không đọc DB, `~/.local/share/com.rustsale.crm*`, `.env`, token hay dữ liệu thật.
- **Mục đích:** P0a trong `docs/v3/ARCHITECTURE.md`. Cột **Trạng thái v3** của 2.1–2.11 đã điền ở P1 (2026-11-22, nhánh `feat/nsh-v3-native`); các bảng khác vẫn để trống.
- **Quy ước ID:** `LEAD`, `CONV`, `CAMP`, `SCRAPE`, `ZALO`, `FB`, `AI`, `IMPORT`, `SET`, `DESK`, `UTIL` cho command; `EVT` cho event; `UI` cho màn hình; `BG` cho tác vụ nền; `DSK` cho tính năng desktop không phải command.

## 0. Tóm tắt số lượng

| Hạng mục | Số lượng | Nguồn đếm |
|---|---:|---|
| Tauri command | 177 | `rg -n '#\[tauri::command' src-tauri/src --glob '*.rs' \| wc -l` |
| Tên event backend emit | 26 | parse 107 emit site (non-test) và gộp tên |
| Điểm frontend `listen` | 17 | 16 tên event, `tray-dnd-changed` nghe 2 nơi |
| Route Svelte | 1 trang | `src/routes/+page.svelte` (SPA, 15 tab) |
| Màn hình và dialog chính | 48 | bảng `UI` |
| Tác vụ nền và tích hợp | 16 | bảng `BG` |
| API endpoint và webhook | 8 | bảng `API` |
| SCRAPE chi tiết (không phải command) | 58 | bảng 2A |
| Tính năng desktop không phải command | 10 | bảng `DSK` |
| Cài đặt người dùng | 8 nhóm | bảng `SET-20..27` |
| Ngôn ngữ | 2 | `en`, `vi`, 724 key mỗi ngôn ngữ |
| Kênh hội thoại | 4 | Zalo, WhatsApp, Facebook Messenger, Email |

**Đối chiếu bắt buộc:** 177 dòng command trong các bảng mục 2 = 177 kết quả grep. Đã kiểm tra tự động.

---

## 1. Kiến trúc v2 đang chạy

- **Desktop:** Tauri 2, app id `com.rustsale.crm`, SQLite `rustsale.db` trong app data dir, WAL.
- **Frontend:** SvelteKit SPA, chỉ một route `+page.svelte`, chuyển 15 tab bằng state; song ngữ Việt/Anh qua Paraglide.
- **Kênh:** Zalo (zca-rs), WhatsApp (client + supervisor riêng), Facebook Messenger (Graph API), Email (SMTP/lettre).
- **Mở rộng:** Local HTTP API chỉ bind `127.0.0.1:8787` (đổi bằng env), MCP mini, Zalo webhook ra ngoài, sidecar `cloak-runtime` cho Google Maps.
- **Nền tảng:** Linux, Windows, macOS; autostart, tray, notification, updater, dialog, opener, process, shell.

### 1.1 Bản đồ module backend

| Module | File chính | Vai trò |
|---|---|---|
| CRM lõi | `src-tauri/src/commands.rs` | Lead, deal, activity, category, outreach, SMTP, import/export, template, account Facebook |
| Hội thoại | `src-tauri/src/conversation/**` | Inbox 4 kênh, outbox, timeline, reaction, forward, pin, quick template, group member |
| Chiến dịch | `src-tauri/src/campaign/**` | Campaign đa kênh, bước, điều kiện, throttling, dispatcher |
| Cào Google Maps | `src-tauri/src/google_maps_jobs/**`, `src-tauri/src/scrapers/**` | Job, grid, browser sidecar, email enrichment |
| Hành chính | `src-tauri/src/administrative_catalog/**` | Tải dữ liệu GeoNames, tìm khu vực, bbox |
| Facebook | `src-tauri/src/facebook/**` | Bài đăng hẹn giờ, comment automation, scheduler, uploader |
| AI | `src-tauri/src/ai/**` | Intent, gợi ý trả lời, policy gate, evidence, proposal |
| Local API | `src-tauri/src/local_api/**` | REST + MCP, token theo client, rate limit |
| Desktop | `src-tauri/src/tray.rs`, `notification.rs`, `lib.rs` | Tray, thông báo, autostart, updater |

---

## 2. Tauri command (177 dòng)

### 2.1 Nhóm LEAD — 22 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| LEAD-01 | LEAD | Phân trang lead theo bộ lọc outreach: category, template, kênh, search, Zalo đã xác minh, chưa từng nhắn, chỉ mobile | ZaloOutreach.svelte, EmailOutreach.svelte | src-tauri/src/commands.rs:get_outreach_leads_page | Có cursor và limit; dùng chung cho Zalo và Email | Đã xuất `LeadsMore.outreachPage` |
| LEAD-02 | LEAD | Trả về toàn bộ ID lead khớp bộ lọc outreach, trừ danh sách ID đã loại | ZaloOutreach.svelte, EmailOutreach.svelte | src-tauri/src/commands.rs:resolve_outreach_lead_ids | Dùng khi chọn "tất cả" thay vì chọn tay | Đã xuất `LeadsMore.outreachIds` |
| LEAD-03 | LEAD | Lấy toàn bộ lead cho bảng, dashboard, kanban và bộ chọn chiến dịch | LeadsList.svelte, Dashboard.svelte, KanbanBoard.svelte, CategoryManager.svelte, CampaignCenter.svelte | src-tauri/src/commands.rs:get_leads | Trả cả lead đã Converted; UI tự tách | Đã xuất `LeadsMore.list`; bản phân trang: `Leads.page` |
| LEAD-04 | LEAD | Lấy chi tiết một lead theo ID | LeadDetails.svelte | src-tauri/src/commands.rs:get_lead_details | Kèm các trường Zalo, currency | Đã xuất `Leads.details` |
| LEAD-05 | LEAD | Tạo lead mới với tiền tệ tùy chọn | LeadsList.svelte | src-tauri/src/commands.rs:add_lead | Validate currency; mặc định VND | Đã xuất `Leads.add` |
| LEAD-06 | LEAD | Đổi trạng thái lead | LeadsList.svelte | src-tauri/src/commands.rs:update_lead_status | Không ghi activity | Đã xuất `Leads.updateStatus` |
| LEAD-07 | LEAD | Lấy toàn bộ deal cho pipeline và dashboard | KanbanBoard.svelte, Dashboard.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:get_deals | Kèm lead, stage, value, currency | Đã xuất `Deals.list` |
| LEAD-08 | LEAD | Chuyển deal sang stage khác khi kéo thả kanban | KanbanBoard.svelte | src-tauri/src/commands.rs:update_deal_stage | Không chặn stage không hợp lệ ở backend | Đã xuất `Deals.updateStage` |
| LEAD-09 | LEAD | Tạo deal gắn lead, giá trị và tiền tệ | KanbanBoard.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:create_deal | Có thể tạo từ lead chưa có deal | Đã xuất `Deals.create` |
| LEAD-10 | LEAD | Xóa deal | KanbanBoard.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:delete_deal | Xóa cứng | Đã xuất `Deals.delete` |
| LEAD-11 | LEAD | Lấy lịch sử hoạt động của một lead | LeadDetails.svelte | src-tauri/src/commands.rs:get_activities | Sắp xếp mới nhất trước | Đã xuất `LeadsMore.activities` |
| LEAD-12 | LEAD | Lấy N hoạt động gần nhất cho dashboard | Dashboard.svelte | src-tauri/src/commands.rs:get_recent_activities | Mặc định 5 trong UI | Đã xuất `LeadsMore.recentActivities` |
| LEAD-13 | LEAD | Ghi một hoạt động mới cho lead | LeadDetails.svelte | src-tauri/src/commands.rs:add_activity | Có loại hoạt động và nội dung | Đã xuất `LeadsMore.addActivity` |
| LEAD-14 | LEAD | Cập nhật toàn bộ thông tin lead | LeadDetails.svelte | src-tauri/src/commands.rs:update_lead | Gồm currency, address, about | Đã xuất `LeadsMore.update` |
| LEAD-15 | LEAD | Xóa lead | LeadDetails.svelte | src-tauri/src/commands.rs:delete_lead | Xóa cứng, UI có xác nhận | Đã xuất `LeadsMore.delete` |
| LEAD-16 | LEAD | Liệt kê category | CategoryManager.svelte, LeadsList.svelte, GroupMembersModal.svelte | src-tauri/src/commands.rs:get_categories | Dùng chung cho gán category | Đã xuất `Categories.list`; cũng có `Leads.categories` |
| LEAD-17 | LEAD | Liệt kê category của một lead | LeadDetails.svelte, GroupMembersModal.svelte | src-tauri/src/commands.rs:get_categories_for_lead | Trả id và tên | Đã xuất `Categories.forLead` |
| LEAD-18 | LEAD | Tạo category mới, từ chối tên rỗng | CategoryManager.svelte, LeadsList.svelte, GroupMembersModal.svelte | src-tauri/src/commands.rs:create_category | Trim và validate | Đã xuất `Categories.create` |
| LEAD-19 | LEAD | Xóa category | CategoryManager.svelte | src-tauri/src/commands.rs:delete_category | Không xóa lead | Đã xuất `Categories.delete` |
| LEAD-20 | LEAD | Gán nhiều lead vào category | CategoryManager.svelte, LeadsList.svelte | src-tauri/src/commands.rs:add_leads_to_category | Trả sớm nếu danh sách rỗng | Đã xuất `Categories.addLeads` |
| LEAD-21 | LEAD | Bỏ nhiều lead khỏi category | CategoryManager.svelte | src-tauri/src/commands.rs:remove_leads_from_category | Trả sớm nếu danh sách rỗng | Đã xuất `Categories.removeLeads` |
| LEAD-22 | LEAD | Liệt kê lead thuộc category | CategoryManager.svelte | src-tauri/src/commands.rs:get_leads_by_category | Dùng cho panel phải của CategoryManager | Đã xuất `Categories.leads`; bản phân trang: `Leads.byCategoryPage` |

### 2.2 Nhóm CONV — 47 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| CONV-01 | CONV | Lấy danh sách hội thoại khách/nhóm của 4 kênh kèm unread và preview | ConversationWorkspace.svelte, ConversationSidebar.svelte | src-tauri/src/conversation/commands.rs:list_conversation_inbox | Gộp identity theo lead và kênh | Đã xuất `Conversations.inboxPage` (phân trang theo cursor) |
| CONV-02 | CONV | Đánh dấu hội thoại đã đọc | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:mark_conversation_read | Cập nhật unread trong inbox | Đã xuất `Conversations.markRead` |
| CONV-03 | CONV | Lấy timeline theo contact, gộp các kênh | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:get_contact_timeline | Dùng khi một người có nhiều kênh | Đã xuất `ConversationActions.contactTimeline` |
| CONV-04 | CONV | Lấy timeline phân trang của một hội thoại | ConversationTimeline.svelte | src-tauri/src/conversation/commands.rs:get_conversation_timeline | Cursor theo occurredAt và messageId | Đã xuất `Conversations.timeline` |
| CONV-05 | CONV | Lấy timeline theo identity đã liên kết | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:get_identity_timeline | Identity là cầu nối Zalo/WhatsApp/Facebook | Đã xuất `ConversationActions.identityTimeline` |
| CONV-06 | CONV | Liên kết identity hội thoại với lead | LinkedLeadActions.svelte | src-tauri/src/conversation/commands.rs:link_conversation_identity | Có thể gộp nhiều identity | Đã xuất `ConversationActions.linkIdentity` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-07 | CONV | Bỏ liên kết identity | LinkedLeadActions.svelte | src-tauri/src/conversation/commands.rs:unlink_conversation_identity | Không xóa tin nhắn | Đã xuất `ConversationActions.unlinkIdentity` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-08 | CONV | Bỏ liên kết lead khỏi hội thoại | LinkedLeadActions.svelte, UnknownContactActions.svelte | src-tauri/src/conversation/commands.rs:unlink_conversation_lead | Hội thoại vẫn còn | Đã xuất `ConversationActions.unlinkLead` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-09 | CONV | Chuyển liên kết lead sang lead khác | LinkedLeadActions.svelte | src-tauri/src/conversation/commands.rs:relink_conversation_lead | Gộp hoặc tách identity | Đã xuất `ConversationActions.relinkLead` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-10 | CONV | Xếp hàng gửi tin nhắn văn bản vào outbox | MessageComposer.svelte | src-tauri/src/conversation/commands.rs:send_conversation_message | Gọi notify_outbox cho Zalo và WhatsApp | Đã xuất `ConversationActions.sendMessage` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-11 | CONV | Lấy trạng thái runtime của các tài khoản kênh | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:get_conversation_runtime_status | Hiển thị banner mất kết nối | Đã xuất `Conversations.runtimeStatus` |
| CONV-12 | CONV | Khởi động lại listener của một tài khoản Zalo | ZaloAccountsSettings.svelte | src-tauri/src/conversation/commands.rs:restart_conversation_account | Chỉ Zalo supervisor | Đã xuất `ConversationNetwork.restartAccount` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CONV-13 | CONV | Xóa một hội thoại theo conversation, identity hoặc lead | DeleteConversationDialog.svelte | src-tauri/src/conversation/commands.rs:delete_conversation | Có validate đúng một định danh | Đã xuất `ConversationActions.deleteConversation` |
| CONV-14 | CONV | Xóa nhiều hội thoại | ConversationSidebar.svelte | src-tauri/src/conversation/commands.rs:delete_conversations_batch | Chế độ chọn nhiều trong sidebar | Đã xuất `ConversationActions.deleteConversations` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-15 | CONV | Mở hội thoại trực tiếp với số hoặc UID mới | NewChatDialog.svelte | src-tauri/src/conversation/commands.rs:start_direct_conversation | Enqueue tin nhắn đầu tiên | Đã xuất `ConversationNetwork.startDirect`; TSX: `NewChatDialog` (RS-62), chọn tài khoản qua `Conversations.directChatAccounts` |
| CONV-16 | CONV | Gửi một batch nhiều phần text/đính kèm, hỗ trợ tag HUMAN_AGENT | MessageComposer.svelte, MessageBatchGroup.svelte | src-tauri/src/conversation/commands.rs:send_conversation_batch | Chỉ chấp nhận tag HUMAN_AGENT | Đã xuất `ConversationActions.sendBatch` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-17 | CONV | Đưa file từ đường dẫn vào hàng chờ đính kèm | MessageComposer.svelte, AttachmentPreviewQueue.svelte | src-tauri/src/conversation/commands.rs:stage_conversation_attachment_paths | Copy vào staging, trả metadata | Đã xuất `ConversationNetwork.stageAttachmentPaths` |
| CONV-18 | CONV | Đưa ảnh clipboard vào hàng chờ đính kèm | MessageComposer.svelte | src-tauri/src/conversation/commands.rs:stage_conversation_clipboard_image | Giới hạn 10 MB ở frontend | Đã xuất `ConversationNetwork.stageClipboardImage` |
| CONV-19 | CONV | Bỏ một tệp đã stage | AttachmentPreviewQueue.svelte | src-tauri/src/conversation/commands.rs:discard_conversation_staged_attachment | Xóa file staging | Đã xuất `ConversationNetwork.discardStagedAttachment` |
| CONV-20 | CONV | Tìm sticker Zalo theo từ khóa | ZaloStickerPicker.svelte | src-tauri/src/conversation/commands.rs:search_zalo_stickers | Gọi qua tài khoản Zalo của hội thoại | Đã xuất `ConversationNetwork.searchStickers` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CONV-21 | CONV | Lấy chi tiết sticker Zalo | StickerMessageContent.svelte | src-tauri/src/conversation/commands.rs:get_zalo_sticker_detail | Tải ảnh sticker | Đã xuất `ConversationNetwork.stickerDetail` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CONV-22 | CONV | Thu hồi tin nhắn đã gửi | RecallMessageDialog.svelte, MessageActionsMenu.svelte | src-tauri/src/conversation/commands.rs:recall_conversation_message | Chỉ tin của mình và trong hạn | Đã xuất `ConversationNetwork.recallMessage` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CONV-23 | CONV | Liệt kê mẫu trả lời nhanh | QuickTemplatePopover.svelte, QuickTemplateManagerDialog.svelte | src-tauri/src/conversation/commands.rs:list_conversation_quick_templates | Mẫu dùng chung toàn app | Đã xuất `ConversationActions.quickTemplates` |
| CONV-24 | CONV | Tạo mẫu trả lời nhanh | QuickTemplateForm.svelte | src-tauri/src/conversation/commands.rs:create_conversation_quick_template | Hỗ trợ ảnh và biến | Đã xuất `ConversationActions.createQuickTemplate` |
| CONV-25 | CONV | Sửa mẫu trả lời nhanh | QuickTemplateForm.svelte | src-tauri/src/conversation/commands.rs:update_conversation_quick_template | Cập nhật nội dung và ảnh | Đã xuất `ConversationActions.updateQuickTemplate` |
| CONV-26 | CONV | Xóa mẫu trả lời nhanh | QuickTemplateManagerDialog.svelte | src-tauri/src/conversation/commands.rs:delete_conversation_quick_template | Xóa cứng | Đã xuất `ConversationActions.deleteQuickTemplate` |
| CONV-27 | CONV | Tăng bộ đếm sử dụng mẫu | QuickTemplatePopover.svelte | src-tauri/src/conversation/commands.rs:mark_conversation_quick_template_used | Dùng cho sắp xếp gần đây | Đã xuất `ConversationActions.markQuickTemplateUsed` |
| CONV-28 | CONV | Thả hoặc bỏ cảm xúc trên tin nhắn | MessageReactionPicker.svelte, MessageReactionSummary.svelte | src-tauri/src/conversation/commands.rs:set_conversation_reaction | Hỗ trợ Zalo và WhatsApp | Đã xuất `ConversationNetwork.setReaction` — bọc, chưa gọi thử (cần tài khoản kênh thật) |
| CONV-29 | CONV | Chuyển tiếp tin nhắn sang nhiều hội thoại | ForwardConversationDialog.svelte | src-tauri/src/conversation/commands.rs:forward_conversation_message | Trả operation id để theo dõi | Đã xuất `ConversationNetwork.forwardMessage` |
| CONV-30 | CONV | Đọc trạng thái một lần chuyển tiếp | ForwardTargetResultList.svelte | src-tauri/src/conversation/commands.rs:get_conversation_forward_operation | Theo từng đích | Đã xuất `ConversationActions.forwardOperation` |
| CONV-31 | CONV | Thử lại một đích chuyển tiếp bị lỗi | ForwardTargetResultList.svelte | src-tauri/src/conversation/commands.rs:retry_conversation_forward_target | Theo client operation id | Đã xuất `ConversationNetwork.retryForwardTarget` |
| CONV-32 | CONV | Liệt kê thành viên có thể @mention | MentionCandidatePopover.svelte | src-tauri/src/conversation/commands.rs:list_conversation_mention_candidates | Chỉ nhóm | Đã xuất `ConversationActions.mentionCandidates` |
| CONV-33 | CONV | Ghim tin nhắn | PinnedMessageBanner.svelte, MessageActionsMenu.svelte | src-tauri/src/conversation/commands.rs:pin_conversation_message | Lưu người ghim | Đã xuất `ConversationActions.pinMessage` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-34 | CONV | Bỏ ghim tin nhắn | PinnedMessageBanner.svelte | src-tauri/src/conversation/commands.rs:unpin_conversation_message | Theo conversation và message | Đã xuất `ConversationActions.unpinMessage` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| CONV-35 | CONV | Liệt kê tin nhắn đã ghim | PinnedMessageBanner.svelte | src-tauri/src/conversation/commands.rs:list_conversation_pinned_messages | Kèm preview | Đã xuất `ConversationActions.pinnedMessages` |
| CONV-36 | CONV | Đọc thành viên nhóm Zalo kèm lead và category | GroupMembersModal.svelte | src-tauri/src/commands.rs:get_group_members_unrestricted | Tên command cũ nhưng vẫn dùng | Đã xuất `ConversationNetwork.groupMembers` |
| CONV-37 | CONV | Import thành viên nhóm thành lead, gán category | GroupMembersModal.svelte | src-tauri/src/commands.rs:batch_import_group_members_to_leads | Có skip_existing và tạo category mới | Đã xuất `ConversationNetwork.importGroupMembers` |
| CONV-38 | CONV | Đồng bộ sâu thành viên nhóm qua runtime Zalo/WhatsApp | GroupMembersModal.svelte | src-tauri/src/commands.rs:sync_and_deep_scan_group_members | Timeout 15 giây, batch profile 50 | Đã xuất `ConversationNetwork.deepScanGroupMembers` — bọc, chưa gọi thử (cần runtime kênh) |
| CONV-39 | CONV | Đồng bộ lịch sử chat nhóm Zalo | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:sync_zalo_group_chat_history | Backfill tối đa 5 trang | Đã xuất `ConversationNetwork.syncZaloGroupHistory` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CONV-40 | CONV | Đồng bộ hội thoại WhatsApp | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:sync_whatsapp_conversations | Qua WhatsApp supervisor | Đã xuất `ConversationNetwork.syncWhatsappConversations` |
| CONV-41 | CONV | Tải tệp đính kèm về máy | ImageMessageContent.svelte, FileMessageContent.svelte, ImageGalleryModal.svelte | src-tauri/src/conversation/commands.rs:download_conversation_attachment | Có kiểm tra quyền và dung lượng | Đã xuất `ConversationNetwork.downloadAttachment` — bọc, chưa gọi thử (cần tệp trên kênh) |
| CONV-42 | CONV | Phát lại delta tin nhắn bị lỡ | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:resume_conversation_deltas | Cứu gap khi event bị lag | Đã xuất `ConversationActions.resumeDeltas` |
| CONV-43 | CONV | Đọc mốc sequence cao nhất của hội thoại | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:get_conversation_high_water_mark | Dùng khi khởi động lại app | Đã xuất `ConversationActions.highWaterMark` |
| CONV-44 | CONV | Bắt đầu phiên QR đăng nhập WhatsApp | WhatsAppAccountsSettings.svelte | src-tauri/src/conversation/commands.rs:start_whatsapp_qr_login | Trả QR qua event whatsapp-qr-code | Đã xuất `ConversationNetwork.startWhatsappQr` — bọc, chưa gọi thử (cần đăng nhập WhatsApp thật) |
| CONV-45 | CONV | Hủy phiên QR WhatsApp | WhatsAppAccountsSettings.svelte | src-tauri/src/conversation/commands.rs:cancel_whatsapp_qr_login | Dọn file session tạm | Đã xuất `ConversationNetwork.cancelWhatsappQr` |
| CONV-46 | CONV | Liệt kê tài khoản WhatsApp đã kết nối | WhatsAppAccountsSettings.svelte | src-tauri/src/conversation/commands.rs:get_managed_whatsapp_accounts | Kèm tên và số điện thoại | Đã xuất `ConversationNetwork.whatsappAccounts` |
| CONV-47 | CONV | Ngắt kết nối tài khoản WhatsApp | WhatsAppAccountsSettings.svelte | src-tauri/src/conversation/commands.rs:disconnect_whatsapp_account | Xóa session và dừng bot | Đã xuất `ConversationNetwork.disconnectWhatsappAccount` |

### 2.3 Nhóm CAMP — 25 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| CAMP-01 | CAMP | Liệt kê mẫu outreach Zalo | ZaloOutreach.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:get_outreach_templates | Kèm ảnh đại diện mẫu | Đã xuất `Campaigns.outreachTemplates` |
| CAMP-02 | CAMP | Tạo mẫu outreach Zalo kèm ảnh | ZaloOutreach.svelte | src-tauri/src/commands.rs:create_outreach_template | Lưu image_url | Đã xuất `Campaigns.createOutreachTemplate` |
| CAMP-03 | CAMP | Sửa mẫu outreach Zalo | ZaloOutreach.svelte | src-tauri/src/commands.rs:update_outreach_template | Ghi đè nội dung và ảnh | Đã xuất `Campaigns.updateOutreachTemplate` |
| CAMP-04 | CAMP | Xóa mẫu outreach Zalo | ZaloOutreach.svelte | src-tauri/src/commands.rs:delete_outreach_template | Xóa cứng | Đã xuất `Campaigns.deleteOutreachTemplate` |
| CAMP-05 | CAMP | Gửi outreach Zalo cho danh sách lead, phát event tiến độ | LeadDetails.svelte | src-tauri/src/commands.rs:send_zalo_outreach | Lọc lead thiếu số; phát outreach-progress, outreach-log, outreach-complete | Đã xuất `Campaigns.sendZaloOutreach` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CAMP-06 | CAMP | Chạy chiến dịch outreach Zalo hàng loạt với delay và ảnh | ZaloOutreach.svelte | src-tauri/src/commands.rs:start_zalo_outreach | Có burst delay và inter-message delay | Đã xuất `Campaigns.startZaloOutreach` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| CAMP-07 | CAMP | Lệnh gửi chiến dịch Zalo kiểu cũ | Không có | src-tauri/src/commands.rs:send_zalo_campaign | Không thấy frontend gọi; giữ để tương thích | Không xuất (lệnh cũ, TSX không được thấy; thay bằng luồng mới) |
| CAMP-08 | CAMP | Đọc lead của chiến dịch cũ | Không có | src-tauri/src/commands.rs:get_campaign_leads | Chỉ service còn dùng; UI dùng LEAD-01 | Không xuất (lệnh cũ, TSX không được thấy; thay bằng luồng mới) |
| CAMP-09 | CAMP | Gửi email hàng loạt qua SMTP, phát event tiến độ | EmailOutreach.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:send_email_outreach | Chạy tokio nền; thay biến tên, công ty, email, số điện thoại | Đã xuất `Campaigns.sendEmailOutreach` — bọc, chưa gọi thử (cần SMTP thật) |
| CAMP-10 | CAMP | Tạo chiến dịch WhatsApp với rate limit và quiet hours | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:create_whatsapp_campaign | Lưu tài khoản, delay, burst, giờ yên tĩnh | Đã xuất `Campaigns.createWhatsappCampaign` |
| CAMP-11 | CAMP | Nạp danh sách người nhận cho chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:populate_whatsapp_campaign_recipients | Chuẩn hóa số E.164, render template | Đã xuất `Campaigns.populateWhatsappCampaignRecipients` |
| CAMP-12 | CAMP | Chạy worker gửi chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:start_whatsapp_campaign | Spawn run_campaign_worker | Đã xuất `Campaigns.startWhatsappCampaign` — bọc, chưa gọi thử (cần tài khoản WhatsApp thật) |
| CAMP-13 | CAMP | Tạm dừng chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:pause_whatsapp_campaign | Đổi trạng thái trong DB | Đã xuất `Campaigns.pauseWhatsappCampaign` |
| CAMP-14 | CAMP | Tiếp tục chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:resume_whatsapp_campaign | Worker đọc lại trạng thái | Đã xuất `Campaigns.resumeWhatsappCampaign` |
| CAMP-15 | CAMP | Hủy chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:cancel_whatsapp_campaign | Dừng worker | Đã xuất `Campaigns.cancelWhatsappCampaign` |
| CAMP-16 | CAMP | Đọc một chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:get_whatsapp_campaign | Kèm counters | Đã xuất `Campaigns.whatsappCampaign` |
| CAMP-17 | CAMP | Liệt kê chiến dịch WhatsApp | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:list_whatsapp_campaigns | Lọc theo trạng thái ở UI | Đã xuất `Campaigns.whatsappCampaigns` |
| CAMP-18 | CAMP | Liệt kê người nhận và trạng thái gửi | WhatsAppOutreach.svelte | src-tauri/src/conversation/commands.rs:get_whatsapp_campaign_recipients | Có skip reason và lỗi cuối | Đã xuất `Campaigns.whatsappCampaignRecipients` |
| CAMP-19 | CAMP | Đặt chiến dịch đa kênh sang ACTIVE | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:start_unified_campaign | Chỉ đổi status, không tự gửi | Đã xuất `Campaigns.startUnifiedCampaign` |
| CAMP-20 | CAMP | Đặt chiến dịch đa kênh sang PAUSED | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:pause_unified_campaign | Chỉ đổi status | Đã xuất `Campaigns.pauseUnifiedCampaign` |
| CAMP-21 | CAMP | Ghi danh lead vào bước đầu của chiến dịch | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:enroll_leads_into_campaign | Tạo campaign_execution PENDING, cần có bước | Đã xuất `Campaigns.enrollLeadsIntoCampaign` |
| CAMP-22 | CAMP | Tính analytics theo bước và tỷ lệ phản hồi/chuyển đổi | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:get_unified_campaign_analytics | Đọc campaign_executions và deals | Đã xuất `Campaigns.unifiedCampaignAnalytics` |
| CAMP-23 | CAMP | Liệt kê chiến dịch đa kênh | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:get_all_unified_campaigns | Trả campaign và metadata | Đã xuất `Campaigns.unifiedCampaigns` |
| CAMP-24 | CAMP | Lưu chiến dịch và toàn bộ bước trong một transaction | CampaignCenter.svelte, SequenceBuilder.svelte | src-tauri/src/campaign/commands.rs:save_unified_campaign | Xóa bước cũ rồi ghi lại | Đã xuất `Campaigns.saveUnifiedCampaign` |
| CAMP-25 | CAMP | Xóa chiến dịch đa kênh | CampaignCenter.svelte | src-tauri/src/campaign/commands.rs:delete_unified_campaign | Xóa cứng | Đã xuất `Campaigns.deleteUnifiedCampaign` |

### 2.4 Nhóm SCRAPE — 22 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| SCRAPE-01 | SCRAPE | Lệnh cào Google Maps kiểu cũ, giữ để tương thích IPC | Không có | src-tauri/src/commands.rs:start_google_maps_scrape | Chuyển tiếp sang job service; UI mới dùng SCRAPE-14 | Không xuất (lệnh cũ, TSX không được thấy; thay bằng luồng mới) |
| SCRAPE-02 | SCRAPE | Dừng job cào kiểu cũ | Không có | src-tauri/src/commands.rs:stop_google_maps_scrape | Không thấy frontend gọi | Không xuất (lệnh cũ, TSX không được thấy; thay bằng luồng mới) |
| SCRAPE-03 | SCRAPE | Tính lưới ô cào từ bbox và bước km | GoogleMapsScraper.svelte | src-tauri/src/commands.rs:plan_grid_crawl | Dùng GridPlanner, giới hạn 10.000 ô | Đã xuất `GoogleMapsMore.planGrid` |
| SCRAPE-04 | SCRAPE | Trả về bbox 63 tỉnh/thành Việt Nam | GoogleMapsScraper.svelte | src-tauri/src/commands.rs:get_preset_provinces | Hard-code trong backend | Đã xuất `GoogleMapsMore.presetProvinces` |
| SCRAPE-05 | SCRAPE | Liệt kê quốc gia từ catalog GeoNames bundled | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/commands.rs:list_administrative_countries | Đọc file resources, không mạng | Đã xuất `GoogleMapsMore.countries` |
| SCRAPE-06 | SCRAPE | Đọc trạng thái dữ liệu hành chính của một nước | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/commands.rs:get_administrative_country_status | Cho biết đã nạp hay chưa | Đã xuất `GoogleMapsMore.countryStatus` |
| SCRAPE-07 | SCRAPE | Tải và nạp dữ liệu hành chính GeoNames cho một nước | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/commands.rs:load_administrative_country | Tải từ download.geonames.org, có force refresh | Đã xuất `GoogleMapsMore.loadCountry` — bọc, chưa gọi thử (tải dữ liệu GeoNames) |
| SCRAPE-08 | SCRAPE | Liệt kê khu vực con theo parent | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/commands.rs:list_administrative_areas | Có limit | Đã xuất `GoogleMapsMore.areas` |
| SCRAPE-09 | SCRAPE | Tìm khu vực hành chính theo từ khóa | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/commands.rs:search_administrative_areas | Tìm theo tên | Đã xuất `GoogleMapsMore.searchAreas` |
| SCRAPE-10 | SCRAPE | Đọc bbox của một geoname ID | AdministrativeAreaSelector.svelte, GoogleMapsScraper.svelte | src-tauri/src/administrative_catalog/commands.rs:resolve_administrative_bounds | Có cache bounds | Đã xuất `GoogleMapsMore.resolveBounds` |
| SCRAPE-11 | SCRAPE | Geocode địa danh thành bbox | GoogleMapsScraper.svelte | src-tauri/src/administrative_catalog/commands.rs:geocode_bounding_box | Dùng catalog, không gọi Google | Đã xuất `GoogleMapsMore.geocodeBbox` |
| SCRAPE-12 | SCRAPE | Đọc trạng thái runtime trình duyệt cào | GoogleMapsRuntimePanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:get_google_maps_runtime_status | Sidecar cloak-runtime | Đã xuất `GoogleMaps.runtimeStatus` |
| SCRAPE-13 | SCRAPE | Cài hoặc chuẩn bị runtime trình duyệt cào | GoogleMapsRuntimePanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:prepare_google_maps_runtime | Có thể tải browser, timeout 15 phút | Đã xuất `GoogleMapsMore.prepareRuntime` — bọc, chưa gọi thử (tải browser runtime) |
| SCRAPE-14 | SCRAPE | Tạo job cào mới và spawn runner | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/commands.rs:create_google_maps_scrape_job | Chia ô ban đầu, chạy nền | Đã xuất `GoogleMaps.createJob` |
| SCRAPE-15 | SCRAPE | Tạm dừng job cào | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:pause_google_maps_scrape_job | Giữ tiến độ | Đã xuất `GoogleMaps.pauseJob` |
| SCRAPE-16 | SCRAPE | Tiếp tục job cào | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:resume_google_maps_scrape_job | Chạy tiếp từ ô dở | Đã xuất `GoogleMaps.resumeJob` |
| SCRAPE-17 | SCRAPE | Hủy job cào | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:cancel_google_maps_scrape_job | Dừng runner | Đã xuất `GoogleMaps.cancelJob` |
| SCRAPE-18 | SCRAPE | Thử lại các ô cào lỗi | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:retry_google_maps_failed_units | Reset ô FAILED | Đã xuất `GoogleMapsMore.retryFailedUnits` |
| SCRAPE-19 | SCRAPE | Thử lại bước tìm email doanh nghiệp | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:retry_google_maps_email_enrichment | Enrichment từ website | Đã xuất `GoogleMapsMore.retryEmailEnrichment` |
| SCRAPE-20 | SCRAPE | Đọc trạng thái một job cào | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:get_google_maps_scrape_job | Kèm action_required | Đã xuất `GoogleMaps.getJob` |
| SCRAPE-21 | SCRAPE | Liệt kê job cào | GoogleMapsJobHistory.svelte | src-tauri/src/google_maps_jobs/commands.rs:list_google_maps_scrape_jobs | Mới nhất trước | Đã xuất `GoogleMaps.listJobs` |
| SCRAPE-22 | SCRAPE | Xóa job cào và dữ liệu tạm | GoogleMapsJobHistory.svelte | src-tauri/src/google_maps_jobs/commands.rs:delete_google_maps_scrape_job | Xóa profile nếu có | Đã xuất `GoogleMaps.deleteJob` |

### 2.5 Nhóm ZALO — 5 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| ZALO-01 | ZALO | Bắt đầu đăng nhập Zalo bằng QR | ZaloAccountsSettings.svelte | src-tauri/src/commands.rs:start_zalo_qr_login | Phát zalo-qr-code, scanned, expired, declined, success, error | Đã xuất `Zalo.startQrLogin` — bọc, chưa gọi thử (cần đăng nhập Zalo thật) |
| ZALO-02 | ZALO | Liệt kê tài khoản Zalo kèm trạng thái auth và runtime | ZaloAccountsSettings.svelte, ZaloOutreach.svelte | src-tauri/src/commands.rs:get_managed_zalo_accounts | Tách auth status và runtime status | Đã xuất `Zalo.accounts` |
| ZALO-03 | ZALO | Xóa phiên Zalo | ZaloAccountsSettings.svelte | src-tauri/src/commands.rs:delete_zalo_session | Xóa session và tài khoản | Đã xuất `Zalo.deleteSession` |
| ZALO-04 | ZALO | Kiểm tra lại phiên Zalo | ZaloAccountsSettings.svelte | src-tauri/src/commands.rs:verify_zalo_session | Gọi zca-rs | Đã xuất `Zalo.verifySession` — bọc, chưa gọi thử (cần tài khoản Zalo thật) |
| ZALO-05 | ZALO | Kiểm tra toàn bộ phiên Zalo | Không có | src-tauri/src/commands.rs:verify_all_zalo_sessions | Không thấy frontend gọi | Không xuất (lệnh cũ, TSX không được thấy; thay bằng luồng mới) |

### 2.6 Nhóm FB — 16 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| FB-01 | FB | Liệt kê page Facebook đã kết nối | FacebookAccountsSettings.svelte, FacebookHub.svelte | src-tauri/src/commands.rs:get_managed_facebook_accounts | Kèm token status, runtime | Đã xuất `Facebook.accounts` |
| FB-02 | FB | Xóa tài khoản hoặc page Facebook | FacebookAccountsSettings.svelte | src-tauri/src/commands.rs:delete_facebook_account | Xóa token trong vault | Đã xuất `Facebook.deleteAccount` |
| FB-03 | FB | Lưu token hoặc page Facebook mới, hoặc cập nhật | FacebookAccountsSettings.svelte | src-tauri/src/commands.rs:save_managed_facebook_account | Hỗ trợ OAuth và nhập tay | Đã xuất `Facebook.saveAccount` — bọc, chưa gọi thử (cần token Meta thật) |
| FB-04 | FB | Lấy danh sách page từ token Facebook | FacebookAccountsSettings.svelte | src-tauri/src/commands.rs:fetch_facebook_pages_from_token | Gọi Graph API | Đã xuất `Facebook.discoverPages` — bọc, chưa gọi thử (cần token Meta thật) |
| FB-05 | FB | Tạo bài đăng Facebook hẹn giờ hoặc nháp | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:schedule_facebook_post | Có media, link, scheduling mode | Đã xuất `Facebook.schedulePost` |
| FB-06 | FB | Đẩy bài đăng Facebook vào hàng đợi xuất bản ngay | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:publish_facebook_post_now | Chỉ nhận draft, failed, queued, overdue_prompt | Đã xuất `Facebook.publishPostNow` |
| FB-07 | FB | Liệt kê bài đăng Facebook | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:list_facebook_scheduled_posts | Lọc theo page, tối đa 100 | Đã xuất `Facebook.scheduledPosts` |
| FB-08 | FB | Hủy bài đăng Facebook chưa xuất bản | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:cancel_facebook_scheduled_post | Xóa bản ghi draft, queued, overdue_prompt | Đã xuất `Facebook.cancelScheduledPost` |
| FB-09 | FB | Tạo rule tự động trả lời bình luận | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:create_comment_automation | Có keyword, match mode, auto like, reply công khai và riêng | Đã xuất `Facebook.createCommentAutomation` |
| FB-10 | FB | Liệt kê rule tự động bình luận | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:list_comment_automations | Lọc theo page | Đã xuất `Facebook.commentAutomations` |
| FB-11 | FB | Sửa rule tự động bình luận | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:update_comment_automation | Cập nhật toàn bộ trường | Đã xuất `Facebook.updateCommentAutomation` |
| FB-12 | FB | Xóa rule tự động bình luận | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:delete_comment_automation | Xóa cứng | Đã xuất `Facebook.deleteCommentAutomation` |
| FB-13 | FB | Đọc log đã trả lời bình luận | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:list_comment_logs | Lọc theo page và rule | Đã xuất `Facebook.commentLogs` |
| FB-14 | FB | Đọc sức khỏe token và page Facebook | FacebookHub.svelte | src-tauri/src/facebook/commands.rs:get_facebook_page_health | Kiểm tra token, quyền, page | Đã xuất `Facebook.pageHealth` |
| FB-15 | FB | Đồng bộ hội thoại Messenger từ Graph API | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:sync_facebook_conversations | Theo page hoặc toàn bộ | Đã xuất `Facebook.syncConversations` — bọc, chưa gọi thử (cần page đã kết nối) |
| FB-16 | FB | Nhận event webhook Messenger vào SQLite, kiểm tra page hợp lệ | ConversationWorkspace.svelte | src-tauri/src/conversation/commands.rs:ingest_facebook_webhook_events | Chỉ nhận event đúng page đã cấu hình | Đã xuất `Facebook.ingestWebhookEvents` |

### 2.7 Nhóm AI — 5 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| AI-01 | AI | Phân loại ý định khách từ text, fallback heuristic | AICopilotBar.svelte | src-tauri/src/ai/commands.rs:classify_customer_intent | Provider từ env: OpenAI, Gemini, Claude, Ollama, Mock | Đã xuất `Ai.classifyIntent` — bọc, chưa gọi thử (cần provider AI) |
| AI-02 | AI | Sinh gợi ý trả lời từ evidence CRM và policy gate | AICopilotBar.svelte | src-tauri/src/ai/commands.rs:generate_smart_reply_suggestions | Ghi evidence log và cập nhật journey state | Đã xuất `Ai.smartReplySuggestions` — bọc, chưa gọi thử (cần provider AI) |
| AI-03 | AI | Liệt kê proposal AI đang chờ duyệt | AIApprovalQueue.svelte | src-tauri/src/ai/commands.rs:get_pending_ai_proposals | Chỉ trạng thái PENDING | Đã xuất `Ai.pendingProposals` |
| AI-04 | AI | Duyệt, từ chối hoặc đánh dấu proposal | AIApprovalQueue.svelte | src-tauri/src/ai/commands.rs:review_ai_proposal | Chấp nhận APPROVED, REJECTED, APPLIED, PENDING | Đã xuất `Ai.reviewProposal` |
| AI-05 | AI | Đọc evidence CRM của lead cho AI | AICopilotBar.svelte, AIApprovalQueue.svelte | src-tauri/src/ai/commands.rs:get_lead_evidence_context | Lead, deal, message gần đây, journey | Đã xuất `Ai.leadEvidenceContext` |

### 2.8 Nhóm IMPORT — 5 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| IMPORT-01 | IMPORT | Mở hộp thoại chọn file Excel hoặc CSV | LeadImportModal.svelte | src-tauri/src/commands.rs:pick_lead_import_file | Lọc xlsx, xls, csv | Đã xuất `ImportLeads.pickFile` — bọc, chưa gọi thử (mở hộp thoại hệ điều hành) |
| IMPORT-02 | IMPORT | Đọc sheet và cột, trả về inspection để map | LeadImportModal.svelte | src-tauri/src/commands.rs:inspect_lead_import_file | Chạy trong spawn_blocking | Đã xuất `ImportLeads.inspectFile` |
| IMPORT-03 | IMPORT | Import lead theo mapping, phát progress qua Channel và event | LeadImportModal.svelte | src-tauri/src/commands.rs:execute_lead_import | Vừa gửi Channel vừa emit lead-import-progress | Đã xuất `ImportLeads.execute` |
| IMPORT-04 | IMPORT | Xuất file mẫu Excel hoặc CSV | LeadImportModal.svelte | src-tauri/src/commands.rs:download_lead_import_template | Có sẵn hai mẫu | Đã xuất `ImportLeads.downloadTemplate` — bọc, chưa gọi thử (mở hộp thoại hệ điều hành) |
| IMPORT-05 | IMPORT | Xuất CSV lead đã chọn qua hộp thoại lưu file | LeadsList.svelte | src-tauri/src/commands.rs:export_leads_csv_file | Gọi qua utils/leadsExportCsv.ts | Đã xuất `ImportLeads.exportCsv` — bọc, chưa gọi thử (mở hộp thoại hệ điều hành) |

### 2.9 Nhóm SET — 19 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| SET-01 | SET | Đọc tiền tệ gốc và tỷ giá | LanguageSettings.svelte, LeadsList.svelte, KanbanBoard.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:get_currency_settings | Lưu trong app_metadata | Đã xuất `Settings.currency` |
| SET-02 | SET | Lưu tiền tệ gốc và tỷ giá | LanguageSettings.svelte | src-tauri/src/commands.rs:update_currency_settings | Chỉ VND và USD | Đã xuất `Settings.updateCurrency` |
| SET-03 | SET | Liệt kê tài khoản SMTP | EmailAccountsSettings.svelte, EmailOutreach.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:get_managed_smtp_accounts | Không trả mật khẩu | Đã xuất `Templates.smtpAccounts` |
| SET-04 | SET | Thêm hoặc sửa SMTP | EmailAccountsSettings.svelte | src-tauri/src/commands.rs:save_smtp_server | Lưu cấu hình | Đã xuất `Templates.saveSmtpServer` |
| SET-05 | SET | Xóa SMTP | EmailAccountsSettings.svelte | src-tauri/src/commands.rs:delete_smtp_server | Xóa cứng | Đã xuất `Templates.deleteSmtpServer` |
| SET-06 | SET | Kiểm tra kết nối SMTP | EmailAccountsSettings.svelte | src-tauri/src/commands.rs:test_smtp_connection | Chỉ kiểm tra host và port | Đã xuất `Templates.testSmtpConnection` — bọc, chưa gọi thử (cần mạng hoặc dữ liệu thật) |
| SET-07 | SET | Liệt kê mẫu email | EmailOutreach.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:get_email_templates | Mẫu email riêng, không phải template đa kênh | Đã xuất `Templates.emailTemplates` |
| SET-08 | SET | Tạo mẫu email | EmailOutreach.svelte, LeadDetails.svelte | src-tauri/src/commands.rs:create_email_template | Có subject và body | Đã xuất `Templates.createEmailTemplate` |
| SET-09 | SET | Sửa mẫu email | EmailOutreach.svelte | src-tauri/src/commands.rs:update_email_template | Cập nhật subject và body | Đã xuất `Templates.updateEmailTemplate` |
| SET-10 | SET | Xóa mẫu email | EmailOutreach.svelte | src-tauri/src/commands.rs:delete_email_template | Xóa cứng | Đã xuất `Templates.deleteEmailTemplate` |
| SET-11 | SET | Liệt kê template đa kênh theo filter | TemplateCenter.svelte, SequenceBuilder.svelte | src-tauri/src/commands.rs:get_templates | Filter theo folder, stage, channel, tag | Đã xuất `Templates.list` |
| SET-12 | SET | Đọc một template đa kênh | TemplateCenter.svelte, TemplatePickerModal.svelte | src-tauri/src/commands.rs:get_template_by_id | Trả đầy đủ nội dung | Đã xuất `Templates.template` |
| SET-13 | SET | Tạo template đa kênh | TemplateCenter.svelte, SaveTemplateModal.svelte | src-tauri/src/commands.rs:create_template | Có biến và spintax | Đã xuất `Templates.createTemplate` |
| SET-14 | SET | Sửa template đa kênh | TemplateCenter.svelte | src-tauri/src/commands.rs:update_template | Cập nhật toàn bộ | Đã xuất `Templates.updateTemplate` |
| SET-15 | SET | Xóa template đa kênh | TemplateCenter.svelte | src-tauri/src/commands.rs:delete_template | Xóa cứng | Đã xuất `Templates.deleteTemplate` |
| SET-16 | SET | Liệt kê folder template | TemplateCenter.svelte | src-tauri/src/commands.rs:get_template_folders | Kèm màu | Đã xuất `Templates.folders` |
| SET-17 | SET | Tạo folder template | TemplateCenter.svelte | src-tauri/src/commands.rs:create_template_folder | Tên và màu | Đã xuất `Templates.createFolder` |
| SET-18 | SET | Xóa folder template | TemplateCenter.svelte | src-tauri/src/commands.rs:delete_template_folder | Template trong folder được bỏ folder | Đã xuất `Templates.deleteFolder` |
| SET-19 | SET | Tăng bộ đếm dùng template | TemplatePickerModal.svelte, SequenceBuilder.svelte | src-tauri/src/commands.rs:increment_template_usage | Dùng cho xếp hạng | Đã xuất `Templates.incrementUsage` |

### 2.10 Nhóm DESK — 8 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| DESK-01 | DESK | Đọc cài đặt thông báo desktop | NotificationSettings.svelte | src-tauri/src/notification.rs:get_notification_settings | enabled, sound, privacy, mute groups | Đã xuất `Settings.notifications` |
| DESK-02 | DESK | Lưu cài đặt thông báo desktop | NotificationSettings.svelte | src-tauri/src/notification.rs:update_notification_settings | Ghi vào app_metadata | Đã xuất `Settings.updateNotifications` |
| DESK-03 | DESK | Đồng bộ hội thoại đang mở để chặn thông báo trùng | ConversationWorkspace.svelte | src-tauri/src/notification.rs:set_active_conversation_focus | Có sequence để chống event cũ | Đã xuất `Desktop.setActiveConversationFocus` |
| DESK-04 | DESK | Gửi thông báo desktop thử | NotificationSettings.svelte | src-tauri/src/notification.rs:test_desktop_notification | Phát desktop-notification-sent với test=true | Đã xuất `Desktop.testNotification` — bọc, chưa gọi thử (cần desktop notification daemon) |
| DESK-05 | DESK | Đọc cấu hình tray | NotificationSettings.svelte | src-tauri/src/tray.rs:get_tray_state | close_to_tray và dnd | Đã xuất `Settings.tray` |
| DESK-06 | DESK | Bật hoặc tắt thu nhỏ xuống tray khi đóng cửa sổ | NotificationSettings.svelte | src-tauri/src/tray.rs:set_close_to_tray | Ghi app_metadata | Đã xuất `Settings.setCloseToTray` |
| DESK-07 | DESK | Bật hoặc tắt Không làm phiền | NotificationSettings.svelte | src-tauri/src/tray.rs:set_tray_dnd | Phát tray-dnd-changed | Đã xuất `Settings.setDoNotDisturb` |
| DESK-08 | DESK | Tải và cài bản .deb mới trên Linux portable | AppUpdateSettings.svelte, UpdateModal.svelte | src-tauri/src/commands.rs:install_portable_linux_update | Tải từ rustsale.trantrongtri.com, validate version | Không xuất dạng lệnh: port `Updater` bám `nsh/updater` (`Host.onUpdaterRequest` / `Host.answerUpdater`) |

### 2.11 Nhóm UTIL — 3 command

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| UTIL-01 | UTIL | Command demo mẫu của Tauri | Không có | src-tauri/src/lib.rs:greet | Vẫn đăng ký trong invoke_handler | Đã xuất `Util.greet` |
| UTIL-02 | UTIL | Sinh hardware ID hash cho license | LicenseModal.svelte | src-tauri/src/commands.rs:get_hardware_id | Hash OS, arch, hostname, user | Đã xuất `Util.hardwareId` |
| UTIL-03 | UTIL | Lưu ảnh từ URL hoặc base64 qua hộp thoại chọn chỗ lưu | ImageGalleryModal.svelte, ImageMessageContent.svelte, FileMessageContent.svelte | src-tauri/src/commands.rs:save_image_file | Gọi qua utils/imageExport.ts | Đã xuất `Util.saveImage` — bọc, chưa gọi thử (mở hộp thoại hệ điều hành) |

**Đối chiếu:** tổng 22 + 47 + 25 + 22 + 5 + 16 + 5 + 5 + 19 + 8 + 3 = **177**.

## 2A. SCRAPE chi tiết — Google Maps (58 dòng, không phải command)

> Bảng này mô tả chức năng, tùy chọn, trạng thái, giao thức sidecar và dữ liệu place. Đây **không phải** danh sách Tauri command: 22 command vẫn nằm ở mục 2.4 và tổng command toàn tài liệu vẫn là 177. ID `SCRAPE-23..80` chỉ dùng cho bảng chi tiết này.

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| SCRAPE-23 | SCRAPE | Chọn chế độ cào keyword hoặc grid | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeMode, CreateScrapeJobRequest.mode | Keyword tìm theo chuỗi, grid chia ô bbox | |
| SCRAPE-24 | SCRAPE | Từ khóa bắt buộc, trim và tối đa 200 ký tự | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/commands.rs:create_job | Rỗng hoặc quá dài bị từ chối trước khi tạo job | |
| SCRAPE-25 | SCRAPE | Location tự do cho keyword mode, ghép thành "keyword location" | GoogleMapsScraper.svelte | src-tauri/src/scrapers/google_maps.rs:search_url | URL dùng `hl=en`; location rỗng thì chỉ tìm keyword | |
| SCRAPE-26 | SCRAPE | Deep scan theo quốc gia và khu vực hành chính | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/commands.rs:plan_initial_units | Location = tên quốc gia, districts = canonical_path từng area; mỗi district một unit; tối đa 500; dedupe lowercase | |
| SCRAPE-27 | SCRAPE | Grid mode bắt buộc bbox và kiểm tra hợp lệ | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/validation.rs:validate_bbox | Finite, đúng khoảng lat/lng, min nhỏ hơn max | |
| SCRAPE-28 | SCRAPE | Grid mode bắt buộc step_km dương | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/validation.rs:validate_and_estimate_grid | UI mặc định 2.0 km | |
| SCRAPE-29 | SCRAPE | Ước lượng số ô trước khi chạy và gợi ý step tối thiểu | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/validation.rs:GridEstimate | Nếu vượt 10.000 ô thì trả recommended_min_step_km | |
| SCRAPE-30 | SCRAPE | Giới hạn an toàn và điều kiện chia ô của lưới | GoogleMapsScraper.svelte | src-tauri/src/scrapers/grid.rs:GridLimits, should_subdivide, grid_budget_exhausted | Tối đa 10.000 ô ban đầu, 25.000 ô tổng, depth 6, ô nhỏ nhất 250 m; chia ô khi feed chưa kết thúc hoặc số listing thô từ 55 trở lên, còn depth, kích thước ô và zoom cho phép; dừng khi tỉ lệ mới dưới 5% hai lần liên tiếp; cạn budget thì paused_limit_reached, coverage_truncated, resume là chấp nhận thiếu coverage và job kết thúc partial | |
| SCRAPE-31 | SCRAPE | Headless bật mặc định, có thể tắt để xem trình duyệt | GoogleMapsScraper.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeJob.headless | Truyền xuống sidecar qua launch payload | |
| SCRAPE-32 | SCRAPE | Profile trình duyệt tự sinh theo job và giữ nguyên khi resume | Không có | src-tauri/src/google_maps_jobs/commands.rs:create_job | `<app_data>/google-maps-profiles/<job_id>/profile`; không cho chọn trong UI; xóa sau 7 ngày | |
| SCRAPE-33 | SCRAPE | Sinh job id và metadata ban đầu | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:create_job | `gmaps-<epoch_ms>-<16 hex>`; status queued, stage scraping, event_sequence 1, version 0, run_generation 0 | |
| SCRAPE-34 | SCRAPE | Chuẩn bị runtime CloakBrowser trước khi tạo job | GoogleMapsRuntimePanel.svelte | src-tauri/src/scrapers/browser.rs:prepare_runtime | UI chặn start nếu runtime chưa installed | |
| SCRAPE-35 | SCRAPE | Chọn vùng grid theo khu vực hành chính hoặc geocode tay | GoogleMapsScraper.svelte, AdministrativeAreaSelector.svelte | SCRAPE-10, SCRAPE-11 | Administrative lấy bounds từ GeoNames; manual geocode chuỗi thành bbox | |
| SCRAPE-36 | SCRAPE | Chọn quốc gia, tỉnh/thành preset, step, headless và deep scan trên form | GoogleMapsScraper.svelte | SCRAPE-03, SCRAPE-04, SCRAPE-05..11 | 63 tỉnh/thành Việt Nam hard-code trong backend | |
| SCRAPE-37 | SCRAPE | Mười trạng thái job | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeJobStatus | queued, running, paused, paused_requires_action, paused_limit_reached, interrupted, partial, completed, failed, cancelled | |
| SCRAPE-38 | SCRAPE | Ma trận chuyển trạng thái hợp lệ | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/repository.rs:is_valid_transition | Partial hoặc failed chỉ về queued khi UserRetry; terminal không quay lại running | |
| SCRAPE-39 | SCRAPE | Sáu stage của job | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeStage | planning, installing_browser_runtime, scraping, email_enrichment, importing, cleanup; code hiện dùng scraping, email_enrichment, importing | |
| SCRAPE-40 | SCRAPE | Sáu trạng thái unit | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeUnitStatus | pending, running, retry_scheduled, completed, failed, cancelled | |
| SCRAPE-41 | SCRAPE | Mười bốn loại lỗi job | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/types.rs:ScrapeErrorKind | disconnected, navigation_timeout, evaluation_timeout, transient_network, captcha, consent_required, rate_limited, selector_changed, invalid_response, browser_startup, runtime_unavailable, invalid_input, database, cancelled | |
| SCRAPE-42 | SCRAPE | Retry policy cho unit lỗi tạm thời | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:RetryPolicy | Tối đa 3 retry, delay 2s, 4s, 8s; tổng 4 lần chạy mỗi unit; chỉ lỗi retryable | |
| SCRAPE-43 | SCRAPE | Phân biệt lỗi lẻ và lỗi hệ thống | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:FailureTolerance | Place unit được bỏ qua nếu lỗi lẻ; search/grid unit không; 5 lỗi liên tiếp, 50 lỗi tổng hoặc hơn 50% sau 20 unit là systemic | |
| SCRAPE-44 | SCRAPE | Tạm dừng chờ người dùng khi gặp CAPTCHA, consent hoặc rate limit | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:action_required | status paused_requires_action; lưu action_required và blocked_url; mở lại trình duyệt visible cùng profile | |
| SCRAPE-45 | SCRAPE | Resume job chờ thao tác chỉ khi trang đã sẵn sàng | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:confirm_action | health_check phải ready hoặc empty_results; nếu vẫn bị chặn thì giữ nguyên paused | |
| SCRAPE-46 | SCRAPE | Pause, resume, cancel và delete đúng trạng thái | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/repository.rs | Pause chỉ từ running; cancel từ mọi trạng thái chưa terminal; delete chỉ terminal; resume từ paused, interrupted, paused_limit_reached | |
| SCRAPE-47 | SCRAPE | Khôi phục job sau khi app tắt đột ngột | GoogleMapsJobHistory.svelte | src-tauri/src/google_maps_jobs/repository.rs:recover_interrupted_jobs | Lúc khởi động: job running thành interrupted, unit running thành pending; người dùng resume | |
| SCRAPE-48 | SCRAPE | Retry các unit lỗi của job partial hoặc failed | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/commands.rs:retry_failed_units | Import kết quả sẵn sàng trước; reset unit failed có last_error_kind trong danh sách retry: disconnected, navigation_timeout, evaluation_timeout, transient_network, browser_startup, runtime_unavailable, selector_changed; queued rồi start; run_generation tăng 1 | |
| SCRAPE-49 | SCRAPE | Retry riêng bước tìm email | GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:retry_email_enrichment | Chỉ job partial; reset enrichment retry_wait hoặc failed về pending; stage email_enrichment | |
| SCRAPE-50 | SCRAPE | Dọn dẹp theo thời hạn | GoogleMapsJobHistory.svelte | src-tauri/src/google_maps_jobs/commands.rs:cleanup_retention | Profile quá 7 ngày và job terminal quá 30 ngày bị xóa; chạy lúc khởi động và khi job terminal | |
| SCRAPE-51 | SCRAPE | Hợp đồng event tiến độ job | GoogleMapsScraper.svelte, GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/events.rs:ScrapeEventPublisher | Event `google-maps-scrape-job-event`; có sequence, status, stage, units, businesses, leads, percentage, message, error, action_required | |
| SCRAPE-52 | SCRAPE | Frontend bỏ qua event cũ hoặc event của job khác | GoogleMapsScraper.svelte | src/lib/services/googleMapsScraperService.ts:reduceGoogleMapsJobEvent | So sequence, chỉ nhận cùng jobId, dừng khi terminal | |
| SCRAPE-53 | SCRAPE | Chụp screenshot và URL chẩn đoán khi bị chặn | Không có | src-tauri/src/google_maps_jobs/processor.rs:capture_scrape_diagnostics | Lưu PNG theo unit, attempt, error kind trong thư mục diagnostics; gắn diagnostic_url vào message | |
| SCRAPE-54 | SCRAPE | Khung truyền tin JSON line giữa Rust và sidecar | Không có | sidecars/cloak-runtime/src/protocol.ts:RuntimeRequest | Mỗi dòng một JSON; request có id, command, payload; response có id, ok, result hoặc error | |
| SCRAPE-55 | SCRAPE | Lệnh runtime_status | GoogleMapsRuntimePanel.svelte | sidecars/cloak-runtime/src/protocol.ts:RuntimeCommand | Trả binary info và wrapperVersion; không tải gì | |
| SCRAPE-56 | SCRAPE | Lệnh ensure_runtime | GoogleMapsRuntimePanel.svelte | sidecars/cloak-runtime/src/runtime.ts:ensureRuntime | Tải hoặc xác nhận binary CloakBrowser đúng phiên bản; timeout 15 phút | |
| SCRAPE-57 | SCRAPE | Lệnh launch | Không có | sidecars/cloak-runtime/src/runtime.ts:launch | Payload profileDir, headless, startUrl tùy chọn; đóng browser cũ trước; trả launched và url | |
| SCRAPE-58 | SCRAPE | Lệnh navigate | Không có | sidecars/cloak-runtime/src/runtime.ts:navigate | Chỉ http/https; waitUntil domcontentloaded; timeoutMs; trả url và title | |
| SCRAPE-59 | SCRAPE | Lệnh evaluate | Không có | sidecars/cloak-runtime/src/runtime.ts:evaluate | Chạy expression trong page, race với timeout; trả value | |
| SCRAPE-60 | SCRAPE | Lệnh screenshot | Không có | sidecars/cloak-runtime/src/runtime.ts:screenshot | Payload path và fullPage; trả path | |
| SCRAPE-61 | SCRAPE | Lệnh health_check và bảy trạng thái trang | GoogleMapsJobPanel.svelte | sidecars/cloak-runtime/src/googleMaps.ts:inspectGoogleMapsPage | Trả state, url, title; state gồm ready, empty_results, captcha, consent_required, rate_limited, blank_page, selector_changed; phát hiện bằng cấu trúc feed, consent dialog, recaptcha và text tiếng Anh hoặc Việt | |
| SCRAPE-62 | SCRAPE | Lệnh shutdown | Không có | sidecars/cloak-runtime/src/runtime.ts:shutdown | Đóng context, browser và kill process tree theo profile; trả stopped | |
| SCRAPE-63 | SCRAPE | Mười ba loại lỗi sidecar và cờ retryable | Không có | sidecars/cloak-runtime/src/protocol.ts:RuntimeErrorKind | disconnected, navigation_timeout, evaluation_timeout, transient_network, captcha, consent_required, rate_limited, selector_changed, invalid_response, browser_startup, runtime_unavailable, invalid_input, cancelled | |
| SCRAPE-64 | SCRAPE | Chuẩn hóa lỗi và loại bỏ chuỗi điều khiển terminal | Không có | sidecars/cloak-runtime/src/protocol.ts:runtimeError | Timeout thành transient_network; closed hoặc disconnected thành disconnected; xóa ANSI escape khỏi message | |
| SCRAPE-65 | SCRAPE | Ghim phiên bản CloakBrowser và chọn transport | Không có | sidecars/cloak-runtime/src/runtime.ts | Wrapper 0.5.1; binary theo platform; playwright-pipe mặc định, Windows dùng CDP websocket; DevTools chỉ loopback | |
| SCRAPE-66 | SCRAPE | Timeout và vòng đời sidecar | GoogleMapsRuntimePanel.svelte | src-tauri/src/scrapers/browser.rs:BrowserSupervisor | Request thường 45s, ensure 15 phút, launch 120s, navigate/evaluate 30s cộng 2s, shutdown 3s; một request một lúc; stderr gắn tiền tố `[cloak-runtime]` | |
| SCRAPE-67 | SCRAPE | Wrapper Rust điều khiển sidecar | GoogleMapsScraper.svelte | src-tauri/src/scrapers/browser.rs:BrowserManager | start, runtime_status, prepare_runtime, relaunch_for_action, navigate, evaluate, screenshot, health_check, shutdown; semaphore 1 phiên | |
| SCRAPE-68 | SCRAPE | Đọc trang kết quả tìm kiếm và thu URL place | Không có | src-tauri/src/scrapers/google_maps.rs:collect_search_listing | Navigate, chờ health, scroll feed đến khi ổn định, dedupe URL, ghi raw_listing_count và reached_end | |
| SCRAPE-69 | SCRAPE | Các trường lấy từ một place | Không có | src-tauri/src/scrapers/google_maps.rs:PlaceCandidate | place_id, canonical_url, name, phone, address, website, facebook, zalo, latitude, longitude | |
| SCRAPE-70 | SCRAPE | Các trường lưu trong kết quả cào | Không có | src-tauri/src/google_maps_jobs/types.rs:ScrapeResult | Xem bảng trường place bên dưới | |
| SCRAPE-71 | SCRAPE | Khóa chống trùng của place | Không có | src-tauri/src/google_maps_jobs/processor.rs:candidate_dedup_key | Ưu tiên place_id, rồi canonical URL, rồi số điện thoại chuẩn hóa, cuối cùng name và address | |
| SCRAPE-72 | SCRAPE | Alias nhận diện cùng một place | Không có | src-tauri/src/scrapers/google_maps.rs:place_identity_aliases | place id, geo kèm slug, URL bỏ viewport; dùng để không lên lịch trùng | |
| SCRAPE-73 | SCRAPE | Kiểm tra place có nằm trong vùng yêu cầu | Không có | src-tauri/src/scrapers/google_maps.rs:candidate_matches_requested_area | Có tọa độ thì so bbox; không có thì so token địa chỉ với location; ngoài vùng bị skipped_outside_area | |
| SCRAPE-74 | SCRAPE | Điều kiện tối thiểu để thành lead | Không có | src-tauri/src/google_maps_jobs/importer.rs:qualifies_for_lead | Số di động Việt Nam 10 số đầu 03, 05, 07, 08, 09 hoặc email hợp lệ; còn lại rejected | |
| SCRAPE-75 | SCRAPE | Khi nào chạy tìm email | Không có | src-tauri/src/google_maps_jobs/processor.rs:candidate_to_result | Chỉ khi không có số điện thoại chuẩn hóa và có website; ngược lại enrichment not_found | |
| SCRAPE-76 | SCRAPE | Quy tắc HTTP an toàn khi tìm email | Không có | src-tauri/src/google_maps_jobs/email.rs:SafeHttpClient | Chỉ http/https, không credentials, chặn IP private, loopback, link-local, multicast, metadata; cùng registrable domain; 3 redirect; 5s kết nối, 15s request; 2 MB; chỉ HTML hoặc text | |
| SCRAPE-77 | SCRAPE | Tôn trọng robots.txt và crawl trang liên hệ | Không có | src-tauri/src/google_maps_jobs/email.rs:find_email | Đọc robots.txt; tối đa 5 trang gồm homepage; ưu tiên contact, about, support; lấy mailto, HTML hiển thị, regex; chuẩn hóa và loại email example | |
| SCRAPE-78 | SCRAPE | Trạng thái tìm email | Không có | src-tauri/src/google_maps_jobs/email.rs:EnrichmentStatus | pending, running, found, not_found, retry_wait, failed; lỗi retryable thành retry_wait | |
| SCRAPE-79 | SCRAPE | Map place thành lead | LeadsList.svelte | src-tauri/src/google_maps_jobs/importer.rs:import_results | Chỉ result pending và inside_requested_area; import ngay sau mỗi unit có kết quả đã sẵn sàng, và lần cuối ở finalize; xem bảng map bên dưới | |
| SCRAPE-80 | SCRAPE | Chống trùng lead và cập nhật bộ đếm | LeadsList.svelte | src-tauri/src/google_maps_jobs/importer.rs:import_results | Trùng số chuẩn hóa hoặc email lowercase thì duplicate; businesses_qualified tăng created cộng duplicates; leads_created tăng created | |

### 2A.1 Trường của một place và kết quả cào

| Trường | Nguồn | Ghi chú |
|---|---|---|
| `place_id` | URL Google Maps | Feature id; có thể null |
| `canonical_url` | URL place | Bỏ viewport, chuẩn hóa |
| `name` | Tên doanh nghiệp | Dùng làm tên lead và company |
| `phone` | Số hiển thị | Giữ nguyên bản gốc |
| `normalized_phone` | Chuẩn hóa Việt Nam | 10 số, đầu 03, 05, 07, 08, 09; nhận 0084 và 84 |
| `email` | Enrichment | Chỉ có sau bước tìm email |
| `normalized_email` | Chuẩn hóa email | Lowercase, validate cú pháp, loại example.com |
| `address` | Địa chỉ place | Dùng cho lead và kiểm tra vùng |
| `website` | Website doanh nghiệp | Đầu vào enrichment |
| `facebook` | Link Facebook | Lưu trong result |
| `zalo` | Link hoặc số Zalo | Lưu trong result |
| `latitude`, `longitude` | Tọa độ place | Dùng kiểm tra inside_requested_area |
| `dedup_key` | Sinh khi lưu result | place_id, canonical URL, phone hoặc name cộng address |
| `inside_requested_area` | So bbox hoặc địa chỉ | false thì skipped_outside_area |
| `detail_status` | Runner | completed khi lấy chi tiết xong |
| `enrichment_status` | Bước email | pending, found, not_found, retry_wait, failed, skipped_outside_area |
| `import_status` | Bước import | pending, imported, duplicate, rejected, skipped_outside_area |
| `last_error_kind`, `last_error_message` | Lỗi gần nhất | Từ unit hoặc enrichment |
| `first_seen_at`, `last_seen_at` | Thời điểm | RFC3339 |

### 2A.2 Map place thành lead

| Trường lead | Giá trị khi import |
|---|---|
| `name` | Tên place, rỗng thì `Google Maps Lead` |
| `company` | Cùng giá trị với `name` |
| `status` | `New` |
| `value` | `0` |
| `last_activity` | Thời điểm import, RFC3339 |
| `owner` | `Google Maps` |
| `email` | Email đã chuẩn hóa, có thể rỗng |
| `phone` | Số đã chuẩn hóa, có thể rỗng |
| `address` | Địa chỉ place |

- Chỉ import result có `inside_requested_area = 1` và `import_status = 'pending'`.
- `qualifies_for_lead` quyết định tạo lead hay `rejected`.
- Trùng số điện thoại chuẩn hóa hoặc email lowercase với lead có sẵn, hoặc trùng trong cùng batch, thì `duplicate`.
- `businesses_qualified` tăng bằng `created + duplicates`; `leads_created` tăng bằng `created`.
- `businesses_discovered` luôn bằng số result hiện có của job.

### 2A.3 Ví dụ message Rust và sidecar

```json
// Rust gửi sidecar qua stdin, mỗi dòng một JSON
{"id":"17","command":"navigate","payload":{"url":"https://www.google.com/maps/search/cafe/@10.75,106.65,15z?hl=en","timeoutMs":30000}}

// Sidecar trả qua stdout, mỗi dòng một JSON
{"id":"17","ok":true,"result":{"url":"https://www.google.com/maps/search/cafe/@10.75,106.65,15z?hl=en","title":"cafe - Google Maps"}}

// Lỗi có kind và retryable
{"id":"18","ok":false,"error":{"kind":"captcha","message":"unusual traffic","retryable":false}}
```

- Request hợp lệ bắt buộc có `id` khác rỗng, `command` khác rỗng và `payload` là object.
- Response thành công có `ok: true` và `result`; thất bại có `ok: false` và `error`.
- Log chẩn đoán của sidecar đi qua stderr, Rust gắn tiền tố `[cloak-runtime]`; stdout chỉ dành cho response JSON.


---

## 3. Event backend → frontend (26 tên)

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| EVT-01 | EVT | Đẩy ảnh QR Zalo cho màn hình đăng nhập | settings/ZaloAccountsSettings.svelte:201 | src-tauri/src/commands.rs:handle_zalo_qr_event | Payload image_base64 | |
| EVT-02 | EVT | Báo QR Zalo đã được quét | Không có listener | src-tauri/src/commands.rs:handle_zalo_qr_event | Phát nhưng UI không nghe | |
| EVT-03 | EVT | Báo QR Zalo hết hạn | Không có listener | src-tauri/src/commands.rs:handle_zalo_qr_event | Payload null | |
| EVT-04 | EVT | Báo người dùng từ chối đăng nhập trên điện thoại | Không có listener | src-tauri/src/commands.rs:handle_zalo_qr_event | Payload null | |
| EVT-05 | EVT | Báo đăng nhập Zalo thành công kèm IMEI | settings/ZaloAccountsSettings.svelte:220 | src-tauri/src/commands.rs:start_zalo_qr_login | UI tải lại danh sách tài khoản | |
| EVT-06 | EVT | Báo lỗi đăng nhập Zalo | settings/ZaloAccountsSettings.svelte:240 | src-tauri/src/commands.rs:emit_zalo_qr_error | Có message | |
| EVT-07 | EVT | Báo tiến độ chiến dịch outreach Zalo | ZaloOutreach.svelte:631 | src-tauri/src/commands.rs:send_zalo_outreach, start_zalo_outreach | progress, status, message | |
| EVT-08 | EVT | Gửi một dòng log của outreach Zalo | ZaloOutreach.svelte:638 | src-tauri/src/commands.rs:send_zalo_outreach, start_zalo_outreach | Có thể rất nhiều dòng | |
| EVT-09 | EVT | Báo outreach Zalo hoàn tất | ZaloOutreach.svelte:643 | src-tauri/src/commands.rs:send_zalo_outreach, start_zalo_outreach | total_sent và message | |
| EVT-10 | EVT | Báo tiến độ chiến dịch Zalo kiểu cũ | Không có listener | src-tauri/src/commands.rs:send_zalo_campaign | Legacy | |
| EVT-11 | EVT | Gửi log chiến dịch Zalo kiểu cũ | Không có listener | src-tauri/src/commands.rs:send_zalo_campaign | Legacy | |
| EVT-12 | EVT | Báo chiến dịch Zalo kiểu cũ hoàn tất | Không có listener | src-tauri/src/commands.rs:send_zalo_campaign | Legacy | |
| EVT-13 | EVT | Báo tiến độ gửi email | EmailOutreach.svelte:516 | src-tauri/src/commands.rs:send_email_outreach_internal | progress, status, message | |
| EVT-14 | EVT | Gửi một dòng log gửi email | EmailOutreach.svelte:520 | src-tauri/src/commands.rs:send_email_outreach_internal | Có thể rất nhiều dòng | |
| EVT-15 | EVT | Báo gửi email hoàn tất | EmailOutreach.svelte:521 | src-tauri/src/commands.rs:send_email_outreach_internal | total_sent và message | |
| EVT-16 | EVT | Báo tiến độ import lead | LeadImportModal.svelte (qua IPC Channel) | src-tauri/src/commands.rs:execute_lead_import | Event song song với Channel; frontend không listen event | |
| EVT-17 | EVT | Báo hội thoại vừa thay đổi để UI làm mới | services/conversationService.ts:690 | src-tauri/src/conversation/events.rs:spawn_tauri_forwarder | reason: message, receipt, reaction, metadata, link, account, history, attachment | |
| EVT-18 | EVT | Đẩy delta tin nhắn mới vào UI | services/conversationService.ts:701 | src-tauri/src/conversation/events.rs:spawn_tauri_forwarder | Có sequence để phát hiện gap | |
| EVT-19 | EVT | Báo delta bị lag và cần đồng bộ lại | services/conversationService.ts:712 | src-tauri/src/conversation/events.rs:spawn_tauri_forwarder | skipped count | |
| EVT-20 | EVT | Đẩy QR đăng nhập WhatsApp | settings/WhatsAppAccountsSettings.svelte:180 | src-tauri/src/conversation/whatsapp/login.rs:emit_qr_code | Tên hằng EVENT_WHATSAPP_QR_CODE | |
| EVT-21 | EVT | Báo đăng nhập WhatsApp thành công | Không có listener | src-tauri/src/conversation/whatsapp/login.rs | Tên hằng EVENT_WHATSAPP_LOGIN_SUCCESS | |
| EVT-22 | EVT | Báo lỗi đăng nhập WhatsApp | Không có listener | src-tauri/src/conversation/whatsapp/login.rs:handle_login_error | Tên hằng EVENT_WHATSAPP_LOGIN_ERROR | |
| EVT-23 | EVT | Báo tiến độ job cào Google Maps | services/googleMapsScraperService.ts:164 | src-tauri/src/google_maps_jobs/events.rs:spawn_tauri_forwarder | Có sequence chống trùng | |
| EVT-24 | EVT | Báo vừa gửi thông báo desktop | services/desktopNotificationService.ts:381 | src-tauri/src/notification.rs:notify_inbound | Kèm conversationId, leadId, sound | |
| EVT-25 | EVT | Yêu cầu UI đánh dấu tất cả đã đọc từ tray | services/desktopNotificationService.ts:389 | src-tauri/src/tray.rs:init_tray | Menu tray | |
| EVT-26 | EVT | Báo trạng thái DND đổi từ tray hoặc settings | services/desktopNotificationService.ts:163 và 393 | src-tauri/src/tray.rs:init_tray, set_tray_dnd | Boolean | |

**Đối chiếu:** 26 tên event, 17 điểm listen (EVT-26 nghe hai nơi), 9 tên không có listener phía frontend (EVT-02, 03, 04, 10, 11, 12, 16, 21, 22).

---

## 4. Màn hình v2 và thao tác

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| UI-01 | UI | Nền app, nạp CSS, Toaster, kiểm tra update nền sau 30 giây | src/routes/+layout.svelte | src/lib/services/updaterService.ts | Không chặn khởi động | |
| UI-02 | UI | Trang duy nhất, chuyển 15 tab, license gate, khởi tạo cầu event và tải lead | src/routes/+page.svelte | src/lib/services/crmService.ts, licenseService.ts, desktopNotificationService.ts | Không có router nhiều trang | |
| UI-03 | UI | Điều hướng 15 tab, badge license, badge update, phiên bản | Sidebar.svelte | src/lib/services/licenseService.ts, updaterService.ts | Tab active là state | |
| UI-04 | UI | Dashboard KPI, mục tiêu doanh thu và stretch, chọn kỳ, deal và activity gần đây | Dashboard.svelte | get_leads, get_deals, get_recent_activities | Mục tiêu lưu localStorage | |
| UI-05 | UI | Bảng lead và contact, search, 5 bộ lọc, phân trang, chọn dòng, thêm lead, import, export CSV, gán category, mở chi tiết | LeadsList.svelte | LEAD-01..22, IMPORT-05 | Có modal thêm lead | |
| UI-06 | UI | Chọn file, đọc sheet và cột, map trường, xem trước, import có tiến độ, tải mẫu | LeadImportModal.svelte | IMPORT-01..04, EVT-16 | Dùng IPC Channel để báo tiến độ | |
| UI-07 | UI | Xem và sửa lead, danh sách deal, tạo và xóa deal, activity, gửi Zalo, gửi email, mở chat Zalo | LeadDetails.svelte | LEAD-04, 07, 09, 10, 11, 13, 14, 15, CAMP-05, CAMP-09 | Có nút mở conversation | |
| UI-08 | UI | Pipeline kanban, kéo thả deal giữa stage, tạo deal, xóa deal, search, đổi tiền tệ | KanbanBoard.svelte | LEAD-07, 08, 09, 10 | Không có edit deal đầy đủ | |
| UI-09 | UI | Tạo và xóa category, thêm và bớt lead, search lead trong category | CategoryManager.svelte | LEAD-16, 18, 19, 20, 21, 22 | Hai panel trái phải | |
| UI-10 | UI | Cào theo keyword hoặc grid, chọn quốc gia/tỉnh/khu vực, bbox tay, step km, headless, deep scan, runtime, start job | GoogleMapsScraper.svelte | SCRAPE-03, 04, 11, 14, 21, 22 | Có ước lượng số ô | |
| UI-11 | UI | Xem tiến độ job, pause, resume, cancel, retry ô lỗi, retry email enrichment, chọn job | GoogleMapsJobPanel.svelte | SCRAPE-15..20 | Nhận event EVT-23 | |
| UI-12 | UI | Danh sách job cào, chọn job, xóa job | GoogleMapsJobHistory.svelte | SCRAPE-21, 22 | Mới nhất trước | |
| UI-13 | UI | Xem trạng thái runtime trình duyệt và nút chuẩn bị runtime | GoogleMapsRuntimePanel.svelte | SCRAPE-12, 13 | Sidecar cloak-runtime | |
| UI-14 | UI | Chọn quốc gia, tải dữ liệu hành chính, duyệt cây khu vực, search, lấy bbox | AdministrativeAreaSelector.svelte | SCRAPE-05..10 | Dùng GeoNames | |
| UI-15 | UI | Workspace hội thoại: inbox, đa kênh, filter, search, chọn nhiều, xóa, timeline, gửi tin, đính kèm, reaction, reply, forward, pin, recall, mention, sticker, quick template, group member, tạo lead, link lead, AI copilot, runtime | conversation/ConversationWorkspace.svelte | CONV-01..43, FB-15, 16, AI-01..05, DESK-03 | Màn hình lớn nhất | |
| UI-16 | UI | Sidebar hội thoại: tab all/unread/priority/stuck, tab customer/group, search, ghim hội thoại, chọn nhiều, xóa hàng loạt | conversation/ConversationSidebar.svelte | CONV-01, 14 | Có virtual scroll | TSX (RS-62): tìm kiếm, tab Tất cả/Chưa đọc/Khách/Nhóm/Người lạ, chip kênh. Chưa làm: tab priority/stuck, ghim, chọn nhiều, xoá hàng loạt (core chưa hỗ trợ hoặc ngoài phạm vi) |
| UI-17 | UI | Timeline tin nhắn: text, ảnh, video, audio, file, sticker, link, unsupported, tìm trong thread, tải thêm, cuộn tới tin khớp | conversation/ConversationTimeline.svelte | CONV-03, 04, 41, 42, 43 | Có cửa sổ ảo | |
| UI-18 | UI | Composer: text, emoji, sticker, đính kèm file, ảnh clipboard, ghi âm, quick template, reply bar, mention, gửi batch | conversation/MessageComposer.svelte, ComposerToolbar.svelte | CONV-10, 16..21, 23..27, 32 | Có drag-drop | |
| UI-19 | UI | Menu tin nhắn: 6 reaction nhanh, reply, copy, pin, forward, recall | conversation/MessageActionsMenu.svelte | CONV-22, 28, 29, 33 | Ẩn action theo capability | |
| UI-20 | UI | Chọn nhiều đích và chuyển tiếp tin nhắn, xem kết quả từng đích, retry đích lỗi | conversation/ForwardConversationDialog.svelte, ForwardTargetResultList.svelte | CONV-29..31 | Có trạng thái từng đích | |
| UI-21 | UI | Danh sách thành viên nhóm, đồng bộ sâu, import thành lead, chọn hoặc tạo category | conversation/GroupMembersModal.svelte | CONV-36..38, LEAD-16..18 | Có CSV helper | |
| UI-22 | UI | Mở chat trực tiếp với số hoặc UID mới | conversation/NewChatDialog.svelte | CONV-15 | Chọn kênh và tài khoản | TSX (RS-62): `NewChatDialog`, nút "Tin nhắn mới" ở đầu hộp thư; chỉ Zalo và WhatsApp (core `startDirect` chỉ hiểu hai kênh này) |
| UI-23 | UI | Xác nhận xóa một hoặc nhiều hội thoại | conversation/DeleteConversationDialog.svelte | CONV-13, 14 | Có cảnh báo | |
| UI-24 | UI | Xác nhận thu hồi tin nhắn | conversation/RecallMessageDialog.svelte | CONV-22 | Chỉ tin của mình | |
| UI-25 | UI | Xem ảnh lớn, chuyển ảnh, tải ảnh về máy | conversation/ImageGalleryModal.svelte | CONV-41, UTIL-03 | Hỗ trợ Ctrl+S | |
| UI-26 | UI | Tìm và chọn sticker Zalo | conversation/ZaloStickerPicker.svelte | CONV-20, 21 | Cần tài khoản Zalo | |
| UI-27 | UI | Gợi ý trả lời AI, phân loại ý định, xem evidence | conversation/AICopilotBar.svelte | AI-01, 02, 05 | Cần provider AI | |
| UI-28 | UI | Hàng chờ proposal AI, xem evidence, duyệt hoặc từ chối | campaign/AIApprovalQueue.svelte | AI-03, 04, 05 | Có trạng thái rỗng | |
| UI-29 | UI | Outreach Zalo: chọn tài khoản, CRUD mẫu kèm ảnh, lọc và phân trang lead, chọn lead, đính kèm ảnh, delay, chạy và xem tiến độ | ZaloOutreach.svelte | CAMP-01..06, LEAD-01, 02 | Nghe EVT-07..09 | |
| UI-30 | UI | Chiến dịch WhatsApp: danh sách và lọc, tạo với tài khoản, người nhận, template, rate limit, quiet hours, xem chi tiết, start, pause, resume, cancel | WhatsAppOutreach.svelte | CAMP-10..18, CONV-44..47 | Có preview spintax | |
| UI-31 | UI | Outreach email: chọn SMTP, CRUD mẫu, lọc và phân trang lead, chọn lead, gửi, xem tiến độ và log | EmailOutreach.svelte | CAMP-09, SET-03, 07..10, LEAD-01, 02 | Nghe EVT-13..15 | |
| UI-32 | UI | Campaign center: danh sách, tạo và sửa chiến dịch nhiều bước, start, pause, ghi danh lead, xem analytics | campaign/CampaignCenter.svelte | CAMP-19..25 | Ghi chú: chưa thấy scheduler chạy thật. Sửa chiến dịch đang chạy giữ id bước, ghi danh và lịch sử (RS D3); bỏ bước có khách thì hỏi "Sẽ bỏ N khách đang ở bước X" trước khi lưu | |
| UI-33 | UI | Builder bước chiến dịch: thêm, xóa, đổi thứ tự, chọn kênh, delay, điều kiện, nội dung, biến, spintax, chọn và lưu template | campaign/SequenceBuilder.svelte | CAMP-24, SET-11..19 | Kênh Zalo, WhatsApp, Email | |
| UI-34 | UI | Thư viện template: folder, filter, tạo, sửa, xóa, preview, randomize spintax, biến | template/TemplateCenter.svelte | SET-11..19 | Có đếm template theo folder | |
| UI-35 | UI | Chọn template khi soạn bước chiến dịch hoặc lưu nội dung thành template | template/TemplatePickerModal.svelte, SaveTemplateModal.svelte | SET-11, 12, 13, 19 | Dùng trong SequenceBuilder | |
| UI-36 | UI | Facebook Hub: chọn page, tạo bài hẹn giờ hoặc nháp, media, link, danh sách bài, publish now, cancel, rule comment, audit log, page health, compliance banner | facebook/FacebookHub.svelte | FB-05..14, FB-01 | Có mockup preview | TSX (RS-70): `FacebookHubScreen.tsx` + `stores/facebookHub.ts` (3 sub-tab: Đăng bài & Lịch đăng có preview Desktop/Mobile, Tự động bình luận CRUD + Nhật ký có lọc, Sức khoẻ Page & banner Tuân thủ Meta); nối route `facebook_hub` trong `Shell.tsx`; test headless `UiFacebookHub.test.tsx` (stub `Facebook.*`, chưa kiểm Graph API thật) |
| UI-37 | UI | Settings: tab Zalo, Email, Facebook, WhatsApp, notification, language, update | Settings.svelte | DESK-01..07, SET-01..10 | Truy cập QR login từ ngoài | |
| UI-38 | UI | Tài khoản Zalo: QR login, verify, reconnect, xóa, trạng thái auth và runtime | settings/ZaloAccountsSettings.svelte | ZALO-01..04, CONV-12 | Nghe EVT-01, 05, 06 | |
| UI-39 | UI | Tài khoản email: danh sách, thêm, sửa, xóa, test SMTP | settings/EmailAccountsSettings.svelte | SET-03..06 | Không hiện mật khẩu | |
| UI-40 | UI | Tài khoản Facebook: OAuth hoặc token tay, chọn page, xóa, trạng thái token | settings/FacebookAccountsSettings.svelte | FB-01..04 | Có validate redirect URI | |
| UI-41 | UI | Tài khoản WhatsApp: danh sách, QR pairing, ngắt kết nối | settings/WhatsAppAccountsSettings.svelte | CONV-44..47 | Nghe EVT-20 | |
| UI-42 | UI | Cài đặt thông báo và tray: bật thông báo, âm thanh, riêng tư, mute nhóm, DND, close-to-tray, autostart, test | settings/NotificationSettings.svelte | DESK-01..07 | Autostart qua plugin | |
| UI-43 | UI | Cài đặt ngôn ngữ, tiền tệ gốc và tỷ giá, preview định dạng | settings/LanguageSettings.svelte | SET-01, 02 | vi và en | |
| UI-44 | UI | Cài đặt cập nhật: kiểm tra, tải và cài, hiện phiên bản và release notes | settings/AppUpdateSettings.svelte | DESK-08, updaterService | Có nút kiểm tra tay | |
| UI-45 | UI | Modal cập nhật: tiến độ tải, cài, relaunch | UpdateModal.svelte | updaterService | Mở từ badge sidebar | |
| UI-46 | UI | Modal license: đăng nhập, chọn gói, thanh toán, poll trạng thái, danh sách thiết bị, thu hồi, copy HWID | LicenseModal.svelte | UTIL-02, licenseService | Gate khi khởi động | |
| UI-47 | UI | Dialog tạo lead từ người lạ, liên kết hoặc bỏ liên kết lead, tạo chat trực tiếp | conversation/CreateLeadDialog.svelte, LinkedLeadActions.svelte, UnknownContactActions.svelte | CONV-06..09, 15, LEAD-05 | Dùng trong workspace | |
| UI-48 | UI | Panel phụ của composer: hàng chờ đính kèm, ghi âm, tìm trong thread, banner tin ghim, banner policy Facebook 24h | conversation/AttachmentPreviewQueue.svelte, VoiceRecorderBar.svelte, InThreadSearchBar.svelte, PinnedMessageBanner.svelte, Facebook24hPolicyBanner.svelte | CONV-17..19, 22, 33..35 | Có kiểm tra capability | |

**Đối chiếu:** 48 màn hình và dialog chính; 1 route Svelte duy nhất.

---

## 5. Tác vụ nền và tích hợp

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| BG-01 | BG | Zalo supervisor khởi động toàn bộ tài khoản, giữ listener, tự kết nối lại và backfill lịch sử | ConversationWorkspace.svelte | src-tauri/src/conversation/zalo/supervisor.rs:start_all, restart_account, backfill_group_messages | Chạy khi app start; history resume 2 giây | |
| BG-02 | BG | Gửi webhook Zalo ra ngoài theo lô cho hệ thống khác | Không có | src-tauri/src/conversation/zalo/webhook.rs:ZaloWebhook | Quiet 120 giây, max wait 180 giây, 3 lần thử, allowlist CSV, fail closed | |
| BG-03 | BG | WhatsApp supervisor drain outbox, kiểm tra kết nối, nạp session khi khởi động | ConversationWorkspace.svelte, WhatsAppAccountsSettings.svelte | src-tauri/src/conversation/whatsapp/supervisor.rs:run_loop, drain_all_connected_accounts, load_active_sessions_on_startup | Poll 500 ms | |
| BG-04 | BG | Worker gửi chiến dịch WhatsApp theo từng người nhận, có delay, burst, quiet hours | WhatsAppOutreach.svelte | src-tauri/src/conversation/whatsapp/campaign.rs:run_campaign_worker | Spawn từ start_whatsapp_campaign | |
| BG-05 | BG | Facebook Messenger supervisor đồng bộ định kỳ và nhận webhook event | ConversationWorkspace.svelte | src-tauri/src/conversation/facebook/supervisor.rs:sync_all, start_background_loop, ingest_webhook_events | Loop 60 giây; kiểm tra page_id | |
| BG-06 | BG | Facebook scheduler xử lý bài hẹn giờ, đối soát và upload session | FacebookHub.svelte | src-tauri/src/facebook/scheduler.rs:run_promotion_cycle, run_reconciliation_cycle | Thread riêng, chu kỳ 30 giây | |
| BG-07 | BG | Nhận webhook Facebook qua Supabase Realtime rồi đưa vào SQLite | ConversationWorkspace.svelte | src/lib/services/metaRealtimeService.ts | Đọc bảng meta_webhook_buffer, gọi FB-15, FB-16 | |
| BG-08 | BG | Runner cào Google Maps: chia ô, chạy unit, subdivide, enrich email, lưu lead, dọn retention | GoogleMapsScraper.svelte, GoogleMapsJobPanel.svelte | src-tauri/src/google_maps_jobs/runner.rs:ScrapeJobRunner, processor.rs, email.rs | Mỗi job một tokio task; browser sidecar | |
| BG-09 | BG | Legacy Google Maps scraper chuyển request cũ thành job mới | Không có | src-tauri/src/commands.rs:legacy_start_google_maps_scrape | Giữ tương thích IPC | |
| BG-10 | BG | Campaign scheduler, dispatcher, evaluator và throttler cho chiến dịch đa kênh | CampaignCenter.svelte | src-tauri/src/campaign/scheduler.rs:start_background_worker, process_due_executions | Có code nhưng không thấy gọi trong lib.rs; cả Default lẫn LiveChannelDeliveryProvider hiện delegate sang SimulatedChannelDeliveryProvider | |
| BG-11 | BG | Worker thông báo desktop: hàng đợi 128, xử lý blocking, lọc theo focus và DND | Không có | src-tauri/src/lib.rs:notification worker | Rate limit 1,5 giây mỗi hội thoại | |
| BG-12 | BG | Local HTTP API chỉ loopback: 5 endpoint REST và 5 tool MCP | Không có | src-tauri/src/local_api/server.rs:spawn | Port 8787 hoặc RUSTSALE_LOCAL_API_PORT; token theo client; scope inbox, timeline, read, send | |
| BG-13 | BG | Import lead chạy nền, parse file ngoài DB lock rồi ghi atomically | LeadImportModal.svelte | src-tauri/src/lead_importer/importer.rs | Có progress qua Channel và event | |
| BG-14 | BG | Kiểm tra license và thiết bị khi khởi động | Sidebar.svelte, LicenseModal.svelte | src/lib/services/licenseService.ts:verifyDeviceAndSubscription | Gọi Supabase; lỗi thì khóa mềm | |
| BG-15 | BG | Kiểm tra cập nhật nền sau 30 giây và kiểm tra tay | UpdateModal.svelte, settings/AppUpdateSettings.svelte | src/lib/services/updaterService.ts | Endpoint R2 latest.json | |
| BG-16 | BG | Tải dữ liệu hành chính GeoNames theo nước và cache bbox | AdministrativeAreaSelector.svelte | src-tauri/src/administrative_catalog/service.rs:load_country, download_country | Tải zip từ download.geonames.org, giới hạn dung lượng | |

### 5.1 Nhóm API — local API và webhook (8)

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| API-01 | API | Đọc danh sách hội thoại qua local API | Không có, dịch vụ ngoài | src-tauri/src/local_api/server.rs:get_inbox | Scope inbox; compact chỉ unread, limit 1..50; mặc định trả full inbox | |
| API-02 | API | Đọc timeline phân trang qua local API | Không có, dịch vụ ngoài | src-tauri/src/local_api/server.rs:get_timeline | Scope timeline; compact limit 1..50, full 1..100; cursor before | |
| API-03 | API | Tải bytes ảnh đính kèm qua local API | Không có, dịch vụ ngoài | src-tauri/src/local_api/server.rs:get_attachment | Scope timeline; trả content-type và bytes | |
| API-04 | API | Đánh dấu hội thoại đã đọc qua local API | Không có, dịch vụ ngoài | src-tauri/src/local_api/server.rs:mark_read | Scope read; trả rowsMarked | |
| API-05 | API | Gửi tin nhắn text qua local API | Không có, dịch vụ ngoài | src-tauri/src/local_api/server.rs:send_message | Scope send; text tối đa 2.000 ký tự; reply và channel account tùy chọn | |
| API-06 | API | MCP mini qua JSON-RPC | Không có, dịch vụ ngoài | src-tauri/src/local_api/mcp.rs:handle | 5 tool: inbox_unread, read_timeline, mark_read, send_message, read_attachment; scope kiểm tra theo tool | |
| API-07 | API | Gửi webhook Zalo ra ngoài theo lô | Không có, dịch vụ ngoài | src-tauri/src/conversation/zalo/webhook.rs:ZaloWebhook | Quiet 120 giây, max wait 180 giây, 3 lần thử, allowlist CSV, fail closed | |
| API-08 | API | Nhận webhook Facebook qua Supabase Realtime | ConversationWorkspace.svelte | src/lib/services/metaRealtimeService.ts | Đọc meta_webhook_buffer, gọi FB-16 rồi FB-15 | |

**Vận hành local API:**
- Bind chỉ `127.0.0.1`, port `8787` hoặc `RUSTSALE_LOCAL_API_PORT`.
- Bearer token lưu dạng SHA-256 trong `local-api-clients.json`, reload khi mtime đổi; token legacy qua env có toàn quyền.
- Rate limit 60 request mỗi phút mỗi client, 10 send mỗi phút; body tối đa 64 KB.
- Mỗi request ghi một dòng log, không log token, query hay body.
- Zalo webhook chỉ gửi cho khách có trong allowlist; thiếu hoặc hỏng file thì không gửi, nhưng tin vẫn lưu DB.
- Facebook webhook đi qua bảng `meta_webhook_buffer` của Supabase rồi mới vào SQLite.

---

## 6. Tính năng desktop (không phải command)

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| DSK-01 | DESK | Tray icon với menu Mở RustSale, Đánh dấu tất cả đã đọc, Bật tắt DND, Thoát; click trái ẩn hoặc hiện cửa sổ | Không có | src-tauri/src/tray.rs:init_tray | Menu tiếng Việt | |
| DSK-02 | DESK | Đóng cửa sổ thì thu xuống tray thay vì thoát | settings/NotificationSettings.svelte | src-tauri/src/tray.rs:TrayState, set_close_to_tray | Mặc định bật | |
| DSK-03 | DESK | Autostart cùng hệ điều hành, chạy với cờ --autostart và ẩn xuống tray | settings/NotificationSettings.svelte | src-tauri/src/lib.rs:determine_autostart_action, tauri-plugin-autostart | Nếu tray lỗi thì hiện cửa sổ | |
| DSK-04 | DESK | Thông báo desktop khi có tin mới: bật tắt, âm thanh, chế độ riêng tư, mute nhóm, DND, bỏ qua hội thoại đang mở | settings/NotificationSettings.svelte | src-tauri/src/notification.rs:notify_inbound | Linux dùng notify-send, fallback plugin | |
| DSK-05 | DESK | Updater plugin: kiểm tra bản mới từ Cloudflare R2, tải và cài, relaunch | UpdateModal.svelte, settings/AppUpdateSettings.svelte | src-tauri/tauri.conf.json:plugins.updater, src/lib/services/updaterService.ts | Windows dùng installMode passive | |
| DSK-06 | DESK | Hộp thoại chọn file và chọn chỗ lưu cho import, export CSV, template, ảnh | LeadImportModal.svelte, LeadsList.svelte, ImageGalleryModal.svelte | tauri-plugin-dialog | Có filter phần mở rộng | |
| DSK-07 | DESK | Mở link ngoài bằng trình duyệt mặc định | conversation/messageMedia.ts, LeadDetails.svelte | tauri-plugin-opener | Zalo chat, link trong tin nhắn | |
| DSK-08 | DESK | Sidecar cloak-runtime cho Google Maps: spawn, giao thức JSON line, timeout, kill | GoogleMapsRuntimePanel.svelte | src-tauri/src/scrapers/browser.rs:BrowserManager | externalBin, 1 browser đồng thời | |
| DSK-09 | DESK | Process plugin để relaunch app sau khi cập nhật | UpdateModal.svelte | tauri-plugin-process | Có fallback nếu lỗi | |
| DSK-10 | DESK | Kéo thả file vào cửa sổ và dán ảnh clipboard vào composer | conversation/MessageComposer.svelte | src/lib/services/conversationPlatform.ts:onDragDropEvent, readClipboardImage | Giới hạn ảnh 10 MB | |

---

## 7. Cài đặt người dùng và i18n

| ID | Nhóm | Tính năng (1 câu) | Màn hình v2 (file .svelte) | Backend (file:hàm hoặc command) | Ghi chú | Trạng thái v3 |
|---|---|---|---|---|---|---|
| SET-20 | SET | Chọn ngôn ngữ Việt hoặc Anh, lưu localStorage, đổi toàn bộ chuỗi Paraglide | settings/LanguageSettings.svelte | src/lib/i18n/state.svelte.js:setLocale | 724 key mỗi ngôn ngữ, mặc định vi | |
| SET-21 | SET | Chọn tiền tệ gốc VND hoặc USD và sửa tỷ giá, preview định dạng | settings/LanguageSettings.svelte | SET-01, SET-02 | Tỷ giá mặc định 25.000 | |
| SET-22 | SET | Bật tắt thông báo desktop, âm thanh, riêng tư, mute nhóm, DND | settings/NotificationSettings.svelte | DESK-01, 02, 07 | Có nút gửi thử | |
| SET-23 | SET | Bật tắt close-to-tray và autostart | settings/NotificationSettings.svelte | DESK-05, 06, tauri-plugin-autostart | Trạng thái đọc từ plugin | |
| SET-24 | SET | Quản lý tài khoản 4 kênh: Zalo, Email SMTP, Facebook, WhatsApp | Settings.svelte | ZALO, FB, SET, CONV tương ứng | Tab trong Settings | |
| SET-25 | SET | Kiểm tra và cài cập nhật, xem phiên bản và release notes | settings/AppUpdateSettings.svelte, UpdateModal.svelte | DSK-05, DESK-08 | Có kiểm tra nền | |
| SET-26 | SET | License: đăng nhập, chọn gói, thanh toán, poll, quản lý thiết bị, copy HWID | LicenseModal.svelte | UTIL-02, licenseService | Dùng Supabase | |
| SET-27 | SET | Mục tiêu doanh thu quý và stretch, lưu localStorage | Dashboard.svelte | Không có backend | Chỉ lưu máy | |

**i18n:** `messages/en.json` và `messages/vi.json`, mỗi file 724 key, đủ cặp; base locale `en` nhưng sản phẩm mặc định `vi`. Có formatter tiền tệ, ngày, số, thời gian tương đối.

---

## 8. Điểm cần chú ý khi port sang v3

1. **Campaign đa kênh chưa chạy thật:** `CampaignScheduler::start_background_worker` không được gọi trong `lib.rs`; `DefaultChannelDeliveryProvider` là `SimulatedChannelDeliveryProvider`, và `LiveChannelDeliveryProvider` cũng đang delegate sang simulated. UI Campaign Center và analytics có thật, nhưng gửi thật đang đi qua ZaloOutreach, EmailOutreach và WhatsApp campaign. Khi port, cần quyết định giữ nguyên hành vi hay nối dispatcher thật.
2. **Command legacy còn đăng ký nhưng không có UI gọi:** `send_zalo_campaign`, `get_campaign_leads`, `start_google_maps_scrape`, `stop_google_maps_scrape`, `verify_all_zalo_sessions`. Vẫn phải giữ nếu muốn parity IPC, nhưng có thể đánh dấu deprecated.
3. **Event không có listener:** `zalo-qr-scanned`, `zalo-qr-expired`, `zalo-qr-declined`, `campaign-progress`, `campaign-log`, `campaign-complete`, `whatsapp-login-success`, `whatsapp-login-error`, `lead-import-progress` (chỉ dùng IPC Channel). v3 có thể bỏ hoặc nối lại cho đủ.
4. **IPC Channel song song event:** import lead vừa gửi `tauri::ipc::Channel` vừa emit `lead-import-progress`.
5. **Local API là hợp đồng ổn định:** 5 endpoint REST, 5 tool MCP, 4 scope, rate limit, token hash; đã có test riêng. Port phải giữ đúng hình dạng JSON và scope.
6. **Zalo webhook fail closed:** thiếu hoặc hỏng allowlist thì không gửi cho ai; file đọc lại tối đa 30 giây một lần. Không được đổi sang fail open.
7. **Toàn bộ query hội thoại và lead là single-user:** không có `tenant_id`; đừng thêm tenant filter máy móc khi port.
8. **Số liệu hiệu năng cần đo lại:** mục tiêu trong `ARCHITECTURE.md` là cold start dưới 1 giây, cuộn 60 fps, lọc 4.6k lead dưới 100 ms, idle dưới 150 MB.

---

## 9. Tổng hợp số đếm và lệnh grep

### 9.1 Command theo file

| File | Số command |
|---|---:|
| `src-tauri/src/commands.rs` | 74 |
| `src-tauri/src/conversation/commands.rs` | 55 |
| `src-tauri/src/google_maps_jobs/commands.rs` | 11 |
| `src-tauri/src/facebook/commands.rs` | 10 |
| `src-tauri/src/administrative_catalog/commands.rs` | 7 |
| `src-tauri/src/campaign/commands.rs` | 7 |
| `src-tauri/src/ai/commands.rs` | 5 |
| `src-tauri/src/notification.rs` | 4 |
| `src-tauri/src/tray.rs` | 3 |
| `src-tauri/src/lib.rs` | 1 |
| **Tổng** | **177** |

### 9.2 Command theo nhóm trong tài liệu

| Nhóm | Số dòng |
|---|---:|
| LEAD | 22 |
| CONV | 47 |
| CAMP | 25 |
| SCRAPE | 22 |
| ZALO | 5 |
| FB | 16 |
| AI | 5 |
| IMPORT | 5 |
| SET | 19 |
| DESK | 8 |
| UTIL | 3 |
| **Tổng** | **177** |

### 9.3 Lệnh đã dùng

```bash
# Đếm command (kết quả 177)
rg -n '#\[tauri::command' src-tauri/src --glob '*.rs' | wc -l

# Đếm command theo file
rg -n --no-heading '#\[tauri::command' src-tauri/src --glob '*.rs' | sed 's/:.*//' | sort | uniq -c | sort -rn

# Đếm emit site (kết quả 107 khi loại file test)
rg -n '\.emit(_to)?\(' src-tauri/src --glob '*.rs' --glob '!*test*' | wc -l

# Liệt kê tên event tĩnh quanh emit (đọc thêm 3 dòng sau mỗi emit)
rg -n -A3 '\.emit(_to)?\(' src-tauri/src --glob '*.rs' --glob '!*test*'

# Tên event động của Zalo QR (4 tên)
rg -n '"zalo-qr-(code|scanned|expired|declined|success|error)"' src-tauri/src/commands.rs

# Hằng số event WhatsApp (3 tên)
rg -n 'EVENT_WHATSAPP' src-tauri/src/conversation/whatsapp/login.rs

# Đếm listen site (17 call site; 2 nằm ở dòng nối tiếp hoặc dùng hằng số)
rg -n "listen(<[^>]*>)?\(['\"]" src --glob '*.svelte' --glob '*.ts' --glob '!*test*' --glob '!*spec*' | wc -l
rg -n 'listen' src/lib/services/googleMapsScraperService.ts src/lib/components/settings/WhatsAppAccountsSettings.svelte

# Kiểm tra command nào không được frontend invoke
rg -o "invoke(<[^>]*>)?\(\s*['\"][a-zA-Z0-9_]+['\"]" src --glob '*.svelte' --glob '*.ts' | sed -E "s/.*\(['\"]//;s/['\"]//" | sort -u

# Kiểm tra command nào không có trong generate_handler
rg -n 'generate_handler' -A220 src-tauri/src/lib.rs
```

### 9.4 Đối chiếu event

- Backend emit: **107** call site (không tính file test), gộp còn **26** tên event.
- Frontend listen: **17** call site, **16** tên event.
- Không có listener: 9 tên (xem mục 8, điểm 3).
- Event dày nhất: `outreach-log` (36 lần emit), `outreach-progress` (22), `campaign-log` (8).

### 9.5 Kiểm tra khác

- 177 command đều có trong `generate_handler!` (không thiếu, không thừa).
- 177 command đều có hàm Rust tương ứng (không có attribute mồ côi).
- `src/routes`: 3 file (`+layout.svelte`, `+layout.ts`, `+page.svelte`), 1 trang thực.
- 15 tab trong Sidebar, 48 màn hình và dialog chính.
- 8 mục API: 5 endpoint REST, 1 MCP, 1 webhook Zalo, 1 webhook Facebook.
- 58 dòng SCRAPE chi tiết ở mục 2A, không tính vào 177 command.
- i18n: 2 locale, 724 key mỗi locale, không thiếu key chéo.

---

*Hết tài liệu. Cột Trạng thái v3 để trống cho các phase P0b–P2.*
