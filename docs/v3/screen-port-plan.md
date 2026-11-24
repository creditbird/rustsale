# RS v3 — kế hoạch port 15 tab v2 sang NSH (UI2)

- **Nhánh:** `feat/nsh-v3-ui2` (gốc `4f9df71` + UI cũ). Người lập: Ishar, 2026-11-22.
- **Nguồn danh sách màn:** `docs/v3/feature-parity.md` mục 4 (UI-01..UI-48). ID trong file này (UI-xx, LEAD/CONV/CAMP…) là ID ở đó.
- **Cách dùng:** Nakroth giao lô cho Lữ Bố theo mục 4; Ishar review diff của từng lô. Lô "giữ lại" do Ishar làm.
- **Quy ước:** `Module.method` là export của `rustsale/native` (kiểu ở `app/.nsh/types/rustsale/native.d.ts`). Lỗi luôn đi qua `report(e, {refresh, retry})` trong `app/stores/toast.ts`; không tự `catch` rồi bỏ.

## 1. Bốn màn đã nối native (mẫu để chép)

| Tab | File | Mẫu cho | Còn thiếu so với v2 (chưa làm trong UI2) |
|---|---|---|---|
| Khách hàng (`leads`) | `app/screens/LeadsScreen.tsx` | bảng cuộn vô hạn + panel chi tiết + dialog thêm | lọc category, 5 bộ lọc nâng cao, chọn nhiều, import/export (UI-06), sửa lead, deal trong chi tiết (UI-07), gửi Zalo/email từ chi tiết |
| Hội thoại (`conversation`) | `app/screens/ConversationScreen.tsx` | master/detail, timeline phân trang ngược, composer, sự kiện `Host.onEvent` | lọc/tab inbox (UI-16), media/đính kèm, reaction/reply/forward/pin/recall (UI-18..20, 24), nhóm (UI-21), tạo chat mới (UI-22), AI copilot (UI-27). Chưa thử gửi thật (không có tài khoản kênh) |
| Thư viện Mẫu (`template_center`) | `app/screens/TemplatesScreen.tsx` | list + form dialog + confirm xoá | folder và lọc (UI-34), preview spintax/biến, UI-35 (picker/save template) |
| Thu thập Maps (`scraper`) | `app/screens/ScraperScreen.tsx` | form + danh sách job + điều khiển + panel kết quả | grid/bbox/vùng hành chính (UI-14), retry ô lỗi và email (UI-11), chuẩn bị runtime (UI-13), lọc/sắp xếp kết quả, nhập kết quả thành lead |

Dùng chung: `stores/core.ts` (`startCore`, `failure`), `stores/paged.ts` (`pager`: phân trang con trỏ, bỏ kết quả cũ), `stores/toast.ts` + `ui/Toast.tsx`, `ui/*`.

## 2. 15 tab v2: đã xong và còn lại

| # | Tab (id) | UI v2 | Trạng thái | Ghi chú |
|---|---|---|---|---|
| 1 | Tổng quan `dashboard` | UI-04 | còn | KPI + deal/activity gần đây |
| 2 | Thu thập Maps `scraper` | UI-10..14 | **xong phần lõi** | mục 1 |
| 3 | Khách hàng `leads` | UI-05..07 | **xong phần lõi** | mục 1 |
| 4 | Danh mục `categories` | UI-09 | còn | |
| 5 | Hội thoại `conversation` | UI-15..27, 47, 48 | **xong phần lõi** | mục 1 |
| 6 | Chiến dịch `campaign_center` | UI-32, 33 | còn | khó, giữ lại |
| 7 | Thư viện Mẫu `template_center` | UI-34, 35 | **xong phần lõi** | mục 1 |
| 8 | Duyệt AI `ai_approval` | UI-28 | còn | |
| 9 | Facebook `facebook_hub` | UI-36, 40 | còn | khó, giữ lại |
| 10 | Zalo Outreach `outreach` | UI-29 | còn | |
| 11 | WhatsApp `whatsapp_outreach` | UI-30, 41 | còn | |
| 12 | Email `email_outreach` | UI-31, 39 | còn | |
| 13 | Pipeline `pipeline` | UI-08 | còn | kéo thả: cần thiết kế |
| 14 | Danh bạ `contacts` | UI-05 (chế độ contact) | còn | |
| 15 | Cài đặt `settings` | UI-37..45 | còn | chia 2 phần |

Ngoài 15 tab: UI-01/02/03 (khung app, sidebar) đã có ở `Shell.tsx` nhưng chưa có badge license/update; UI-45, UI-46 (update, license) chưa làm, thuộc lô Ishar.

## 3. Tab còn lại: mẫu, export, panel/dialog, test

Bài test TSX luôn chạy headless trên DB tạm: `Host.start("/tmp/…")`, `Demo.seed()` nếu cần hội thoại/scrape, rồi `open("./App.tsx")`. Mỗi tab tối thiểu: (a) hiện dữ liệu từ core, (b) thao tác ghi đi qua core, (c) lỗi `invalid_request` hiện cho người dùng sửa, (d) `not_found` làm mới danh sách.

| Tab | Mẫu | Export dùng | Panel/dialog cần | Test TSX cần viết |
|---|---|---|---|---|
| `categories` | Leads (bảng) + Templates (form/confirm) | `Categories.list/create/delete/addLeads/removeLeads/leads`, `Leads.page` | 2 panel trái/phải, dialog tạo, confirm xoá, ô tìm lead | tạo+xoá category; thêm/bớt lead; xoá category không tồn tại → làm mới |
| `contacts` | Leads | `LeadsMore.list`, `Leads.page`, `Leads.details` | cùng panel chi tiết với Leads (tách `LeadDetail` dùng chung) | lọc contact; mở chi tiết |
| `ai_approval` | Conversation (list + chi tiết) | `Ai.pendingProposals/reviewProposal/leadEvidenceContext` | hàng chờ, panel evidence, nút duyệt/từ chối, trạng thái rỗng | rỗng; duyệt; từ chối; lỗi provider → toast thử lại |
| `outreach` (Zalo) | Leads (chọn nhiều) + Templates | `Campaigns.outreachTemplates/create…/delete…`, `LeadsMore.outreachPage/outreachIds`, `Zalo.accounts`, `Campaigns.sendZaloOutreach/startZaloOutreach` | chọn tài khoản, CRUD mẫu, lọc, chọn lead, delay, tiến độ (`Host.onEvent`) | lọc+phân trang; chọn tất cả/loại trừ; chạy với `invalid_request` khi thiếu tài khoản |
| `whatsapp_outreach` | Scraper (danh sách + điều khiển) | `Campaigns.whatsappCampaign(s)/create…/populate…/start…/pause…/resume…/cancel…/whatsappCampaignRecipients`, `ConversationNetwork.whatsappAccounts` | danh sách + lọc, dialog tạo, chi tiết, spintax preview | tạo; start/pause/resume/cancel đổi trạng thái; chi tiết phân trang người nhận |
| `email_outreach` | Templates + Leads | `Templates.emailTemplates/create…/update…/delete…`, `Templates.smtpAccounts`, `Campaigns.sendEmailOutreach`, `LeadsMore.outreachPage` | chọn SMTP, CRUD mẫu email, chọn lead, tiến độ + log | CRUD mẫu; gửi với SMTP thiếu → `invalid_request` |
| `dashboard` | Leads (đọc) | `Leads.page`, `Deals.list`, `LeadsMore.recentActivities`, `Settings.currency` | thẻ KPI, chọn kỳ, mục tiêu (lưu bằng `Settings`, không localStorage), danh sách gần đây | KPI khớp seed; đổi kỳ |
| `settings` (A: thông báo, tiền tệ, cập nhật) | Templates (form) | `Settings.*`, `Desktop.testNotification`, `Host.onUpdaterRequest/answerUpdater` | tab, form, nút kiểm tra cập nhật | lưu rồi đọc lại; `invalid_request` khi tỷ giá sai |
| `settings` (B: tài khoản Zalo/Email/Facebook/WhatsApp) | Scraper (danh sách + điều khiển) | `Zalo.accounts/startQrLogin/verifySession/deleteSession`, `Templates.saveSmtpServer/…`, `Facebook.accounts/saveAccount/…`, `ConversationNetwork.startWhatsappQr/…` | QR, form SMTP, token tay, trạng thái | form SMTP validate; xoá tài khoản; **không đăng nhập thật trong test** |
| `pipeline` | cần thiết kế | `Deals.list/updateStage/create/delete`, `Leads.page` | kanban 5+ cột, kéo thả, tạo/xoá deal | chuyển stage ghi vào core; xoá deal |
| `campaign_center` | cần thiết kế | `Campaigns.unifiedCampaigns/saveUnifiedCampaign/start…/pause…/enroll…/unifiedCampaignAnalytics/deleteUnifiedCampaign` | danh sách, builder bước, analytics | tạo 2 bước; start/pause; analytics |
| `facebook_hub` | cần thiết kế | `Facebook.*` (15 export) | chọn page, lịch bài, media, rule comment, log, health | lập lịch/huỷ; rule comment CRUD |

## 4. Chia lô

### Lô cho Lữ Bố (lặp lại theo mẫu có sẵn, không cần thiết kế mới)

| Lô | Tab | Dựa vào | Việc | Kích thước |
|---|---|---|---|---|
| **L1** | `categories`, `contacts` | Leads + Templates | 2 màn, tách `LeadDetail` dùng chung | nhỏ |
| **L2** | `outreach` (Zalo), `email_outreach` | Leads (chọn nhiều) + Templates | 2 màn cùng khung: chọn lead, mẫu, chạy, tiến độ | vừa |
| **L3** | `whatsapp_outreach`, `ai_approval` | Scraper (danh sách + điều khiển), Conversation | 2 màn danh sách + hành động | vừa |
| **L4** | `settings` phần A và B | Templates (form), Scraper (danh sách) | nhiều form nhỏ; B không đăng nhập thật | vừa |
| **L5** | khoảng trống trong 4 màn đã xong | mục 1, cột "Còn thiếu" | lọc category, sửa lead, folder mẫu, retry ô lỗi, lọc inbox, tạo chat | lớn, chia nhỏ theo dòng |

Mỗi lô: 1 nhánh `feat/nsh-v3-ui-L<n>` từ `feat/nsh-v3-ui2`; mỗi màn kèm test headless; `nsh check` + `nsh test` qua `heavy-job`; Ishar review diff trước khi merge.

### Giữ lại cho Ishar (cần thiết kế mới)

- `pipeline`: kéo thả kanban trong NSH chưa có mẫu; cần chọn cách làm (gesture hay nút chuyển stage).
- `campaign_center` + builder bước (UI-32/33): form lồng nhau, điều kiện, biến, spintax.
- `facebook_hub` (UI-36): nhiều tab, lịch, media, tuân thủ; phụ thuộc OAuth.
- Hội thoại nâng cao: media/đính kèm, reaction/forward/pin/recall, nhóm, AI copilot (UI-18..27, 48).
- UI-45/46 (modal cập nhật, license) và badge sidebar.

## 5. Lưu ý cho người port

- Danh sách dài: dùng `pager` + `<List onLoadMore>`; không dựng lại mảng bằng spread mỗi trang.
- Không chờ DB hay mạng trên luồng UI: mọi export là promise; chặn bấm lặp bằng cờ `sending`/`busy`.
- `Host.start` chỉ chạy một lần mỗi tiến trình; test dùng chung một thư mục tạm nên không giả định bảng rỗng (so sánh trước/sau).
- Kiểu trả về là `unknown`; khai báo interface cục bộ (xem `Lead`, `InboxItem`, `Job`) và đọc theo tên trường thật của core (`snake_case` cho Templates/Leads/Scrape, `camelCase` cho Conversations).
- `GoogleMaps.runtimeStatus()` ném `unavailable` khi thiếu CloakBrowser (sửa ở 6094f4e); màn Scraper chỉ coi `unavailable` là "chưa sẵn sàng", mã khác hiện "không kiểm tra được" kèm thông báo (`app/stores/scraperRuntime.ts`).
- Ảnh chụp màn: `nsh run --target gpui` với `XDG_DATA_HOME` trỏ thư mục tạm đã seed (`Demo.seed` chỉ chạy trong thư mục tạm).
