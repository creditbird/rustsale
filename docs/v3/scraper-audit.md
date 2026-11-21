# RS v3: Kiểm toán Google Maps scraper của v2 (P0d)

Ngày: 2026-11-22. Người làm: Elsu. Giao bởi Nakroth. Chỉ đọc và chạy thử, chưa sửa code v2.
Mã nguồn đã rà: `RustSale_v2` ở commit `ffc4dc4`:
`src-tauri/src/google_maps_jobs/*`, `src-tauri/src/scrapers/*`, `sidecars/cloak-runtime/src/*`, UI `src/**/GoogleMaps*.svelte`,
cùng sổ lỗi cũ `docs/superpowers/plans/2026-07-2*-google-maps-*.md` (các lỗi AUD/REV đã sửa không được báo lại).

Tài liệu này không chứa dữ liệu khách (tên, số điện thoại, email). Chỉ dùng mã lỗi, đường dẫn code, đếm số lượng và 6 ký tự cuối của job id.

## 1. Kết quả tóm tắt

**50 lỗi (đã loại trùng): 5 cao, 25 vừa, 20 thấp.** Thêm 9 mục "chưa chắc" (cần xác nhận bằng chạy thật) không tính vào số trên,
và 4 lỗi trong số 50 được gắn cờ "chưa chắc" vì phụ thuộc DOM thật của Google (C6, C10, C18, C19).

| Mức | Số lỗi | Mã |
|---|---|---|
| Cao | 5 | A1, A2, A3, B1, C1 |
| Vừa | 25 | A4-A7, A10, B2-B9, C2, C3, C5-C12, D1, D2 |
| Thấp | 20 | A8, A9, A11-A13, B10, B11, B13-B15, C13-C19, C21, D3, D4 |

Cách đọc mã: `A` = vòng đời job (runner/processor/commands), `B` = dữ liệu, lưới, email (repository/importer/email/grid),
`C` = trình duyệt, selector, sidecar, giao diện, `D` = lỗi chỉ lộ ra từ log thật. Lỗi bị hai người đọc báo trùng chỉ giữ một mã:
- B12 trùng A9 (import sau checkpoint làm job `Failed`); chi tiết ở A9.
- C4 trùng B1 (fallback website lấy link ngoài bất kỳ); chi tiết ở B1.
- C20 trùng B11 (chuẩn hoá SĐT VN); chi tiết ở B11.
- A2 và A10 cùng nằm ở danh sách kind được reset khi "thử lại" (`repository.rs:386-390`); sửa một chỗ, nhưng test riêng từng trường hợp.
- Các mục "Chưa chắc" đánh lại mã: phần A là `A-U1..A-U5`, phần B là `B-U1..B-U4` (trong bản gốc của hai phần đều ghi là C1..C5, C1..C4).

Ba lỗi cao đáng sửa trước vì chúng cùng một chuỗi: rate limit ở trang place (A1) làm job `Failed` sai nhãn, các unit bị mất khỏi bộ đếm (A2),
và job dừng chờ captcha không resume được sau khi mở lại app (A3). Đó là đường hỏng thật khi chạy job lớn.

## 2. Bằng chứng từ log và DB thật

Nguồn: `~/.local/share/com.rustsale.crm/logs/google-maps-scraper.jsonl.*` (chỉ đọc, 919.414 dòng, từ 2026-07-26 đến 2026-11-21,
trong đó 38.412 dòng thuộc `target=google_maps_scraper`) và bản sao DB `~/.local/share/com.rustsale.crm.v3-dev/rustsale.db` (chỉ đọc).

| Số liệu | Giá trị |
|---|---|
| Job khác nhau thấy trong log | 25 (40 lần `job_started`, vì resume phát lại sự kiện bắt đầu) |
| Job có sự kiện kết thúc (`job_completed` hoặc `job_partial`) | 7 (2 completed, 5 partial) |
| Job không có bất kỳ sự kiện kết thúc nào | 18 trên 25 (72%) |
| `unit_started` / `unit_completed` / `unit_failed` | 19.197 / 19.159 / 6 (38 unit bắt đầu nhưng không có kết cục, do dừng hoặc tắt app giữa chừng) |
| `unit_retry_scheduled` | 3, đều cùng một lỗi khởi động (D1) |
| Lỗi theo loại | `selectorchanged` 3, `browserstartup` 3 (kể cả lần retry) |
| `job_partial` ở stage `importing` với `failed_units=0` | 3 trên 5 (bằng chứng cho B9) |
| Lỗi mức ERROR của scraper | 1: `startup_retention_cleanup_failed` ("database is locked") ngày 2026-08-26 (D3) |
| Bảng `google_maps_scrape_jobs/units/results` trong bản sao DB | 0 dòng (retention xoá job, unit, kết quả sau 30 ngày; job cuối trong log là 2026-10-14) |
| Bảng `leads` trong bản sao DB | 4.607 dòng; 0 số điện thoại trùng, 0 email trùng (so khớp theo chữ thường); 460 không có số; 2.171 số bắt đầu bằng `0`, 4 bắt đầu bằng `84` hoặc `+84` |

Hệ quả: không còn dòng DB nào của job để đối chiếu từng lỗi A, B, C với dữ liệu thật. Bằng chứng của chúng là `file:dòng` kèm kịch bản test
tất định. Số liệu log ở trên được dùng làm bằng chứng cho D1-D4, B9, C7, C10.
Số liệu `leads` cho thấy khâu chống trùng lead khi import hiện hoạt động tốt (không trùng số, không trùng email); đây là điểm cần giữ nguyên ở v3.
Điều này cũng có nghĩa: v3 phải giữ chính sách retention 30 ngày cho kết quả thô và 7 ngày cho profile trình duyệt (`task15_test.rs:327-441`), vì lead đã import nằm ở bảng `leads`, không phụ thuộc.

Tái hiện từng lỗi dùng bộ test trong `rustsale-core` của v3 (xem phần 3), không cần chạy Google thật. Riêng nhóm "chưa chắc" cần job thật hoặc fixture DOM.

Đã chạy thử: (1) phân tích log bằng script đọc-chỉ, (2) đếm DB bằng `sqlite3 -readonly`, (3) mô phỏng hàm `radius_to_zoom` của `grid.rs:46-62`
cho mọi bán kính từ 300 m đến 40 km để kiểm B7 (kết quả: ô có bán kính 2400-2450 m và từ 20000 m trở lên không chia nhỏ được vì ô con cùng mức zoom). Chưa chạy `cargo test` của v2 (không build lại v2 trong lượt này).

## 3. Lỗi chỉ lộ ra từ log thật (D)

### D1. Bản build sidecar thiếu `playwright-core`: mọi unit lỗi `browser_startup` (vừa)

**Bằng chứng.** Log 2026-08-17 11:59-12:00, job `...d69ff6`: `unit_retry_scheduled` ba lần (attempt 1, 2, 3), rồi `unit_failed` ở attempt 4 và 5, error_kind `browserstartup`,
message `ResolveMessage: Cannot find module '/home/runner/work/rustsale/rustsale/node_modules/playwright-core/package.json'` (đường dẫn của máy CI nằm trong binary đóng gói bằng bun).
Job kế tiếp lúc 15:31 cùng ngày chạy được, nên bản đó đã được sửa bằng phát hành mới, nhưng không có test nào chặn nó.
**Cách tái hiện.** Chạy binary `cloak-runtime-<triple>` đã đóng gói trên máy không có thư mục `node_modules` của CI, gửi lệnh `launch`.
**Cách sửa.** Thêm bước CI "smoke test sidecar" chạy binary đã đóng gói trong thư mục trống, gửi `launch` rồi `close`, fail nếu có `ResolveMessage`. Lỗi này đúng loại không nên retry ba lần (cấu hình hỏng, không phải tạm thời): phân loại `ResolveMessage` thành lỗi cấu hình `runtime_unavailable` không retry và báo thẳng cho người dùng.
**Test hồi quy.** `sidecar_packaged_binary_launches_without_repo_node_modules` (CI); `browser_startup_resolve_error_is_not_retried` (core: processor giả trả `BrowserStartup` có "Cannot find module", assert `execute` gọi đúng một lần).

### D2. Log thiếu sự kiện dừng, 18 trên 25 job không có kết cục (vừa)

**Bằng chứng.** Chỉ có 8 loại sự kiện trong 38.412 dòng: `job_started`, `unit_started`, `unit_completed`, `unit_failed`, `unit_retry_scheduled`, `job_partial`, `job_completed`, `startup_retention_cleanup_failed`.
Không có sự kiện cho pause, cancel, resume, `failed`, `paused_requires_action`, captcha/chặn, hay recover khi khởi động. Job `...614b40` bắt đầu 5 lần trong 2 ngày với 941 `unit_started` và 937 kết cục (4 unit không biết số phận), và không có dòng nào cho thấy vì sao nó dừng lại giữa chừng.
**Hệ quả.** Không chẩn đoán được job treo hoặc bị chặn từ log (đây là nguyên nhân tôi không đối chiếu được các lỗi A1-A5 với sự cố thật).
**Cách sửa.** Phát (tracing, `target=google_maps_scraper`) mọi chuyển trạng thái của `transition_job`, `pause_job`, `pause_for_action`, `fail_job`, `recover_interrupted_jobs`, kèm `job_id`, trạng thái cũ và mới, lý do, `run_generation`. Mọi `job_started` phải có `resumed=true/false`.
**Test hồi quy.** `events::tests::every_job_status_transition_emits_a_log_event` (duyệt mọi cặp chuyển trạng thái hợp lệ, assert một dòng log mỗi lần).

### D3. Dọn retention lúc khởi động lỗi "database is locked" và không thử lại (thấp)

**Bằng chứng.** Log 2026-08-26T14:10:11: `startup_retention_cleanup_failed error="database is locked"`. `lib.rs:290-303` chạy `recover_startup` rồi `cleanup_retention` đúng một lần, không retry; lần dọn kế tiếp chỉ xảy ra khi có một job kết thúc (`lib.rs:264-283`). App khởi động cùng lúc với các tác vụ khác (Zalo, Facebook) tranh khoá ghi.
**Cách sửa.** Bật `busy_timeout` cho kết nối retention và thử lại 3 lần với khoảng nghỉ tăng dần; dọn thêm theo lịch (ví dụ mỗi 6 giờ).
**Test hồi quy.** `retention_retries_when_database_is_locked` (giữ khoá ghi 200 ms từ kết nối khác, assert cleanup vẫn thành công).

### D4. File log tên `google-maps-scraper.jsonl.*` chứa toàn bộ log của app (thấp)

**Bằng chứng.** 919.414 dòng, chỉ 38.412 (4%) thuộc scraper; phần còn lại là Zalo, Facebook, v.v. Riêng lỗi `Failed to sync Facebook Messenger page ... UNIQUE constraint failed: message_attachments.message_id, message_attachments.provider_attachment_id` lặp 48.604 lần từ 10 đến 21/11 (ngoài phạm vi scraper, nên ghi chú cho owner của module Facebook).
**Hệ quả.** Log scraper khó lọc, file nặng (hàng MB mỗi ngày), spam che mất sự kiện thật.
**Cách sửa.** v3 tách file log theo target (scraper riêng), giới hạn số ngày giữ log; tách lỗi lặp bằng rate-limit log.
**Test hồi quy.** `logging::tests::scraper_events_go_to_scraper_file_only`.

## 4. Danh sách tính năng cần giữ ở v3 (parity)

Xem mục "DANH SÁCH TÍNH NĂNG" ở cuối Phần C bên dưới (chi tiết theo từng màn và lệnh), cộng các hành vi sau từ Phần A và B:
chạy một job tại một thời điểm, pause/resume/cancel/retry unit lỗi, dừng chờ captcha và mở lại trình duyệt, khôi phục khi khởi động, lưới thích nghi chia nhỏ ô theo mật độ,
import lead có chống trùng theo số và email, làm giàu email từ website qua HTTP an toàn (chặn SSRF, robots.txt, giới hạn 2 MB), retention 30 ngày, sự kiện thời gian thực với số đếm.

## 5. Thứ tự sửa đề xuất

1. Nhóm vòng đời (A1, A2, A3, A10, A4, A5): thiết kế lại bộ đếm unit và chính sách retry cùng lúc (một cột `failure_count`, danh sách "có thể thử lại" theo cờ chứ không theo kind).
2. Nhóm sidecar (C1, C9, C3, D1): đóng sidecar sạch, dọn tiến trình mồ côi khi `launch`, kiểm đóng gói trong CI.
3. Nhóm chất lượng dữ liệu (B1, B2, C5, C6, B11): bỏ fallback link ngoài, siết regex email, bắt buộc fixture DOM cho trích xuất.
4. Nhóm lưới (B7, B8): một nguồn hình học duy nhất cho planner, validation và sinh ô.
5. Còn lại theo mức độ. Mỗi lỗi sửa phải có test hồi quy ghi trong mục tương ứng (đúng yêu cầu của `ARCHITECTURE.md`).

---

# Phần A. Vòng đời job


Phạm vi: `src-tauri/src/google_maps_jobs/{runner,processor,commands,events,diagnostics,types,validation,mod}.rs`. Có đọc thêm các đoạn liên quan trong `repository.rs`, `importer.rs`, `email.rs`, `scrapers/browser.rs`, `scrapers/google_maps.rs`, `lib.rs` để lần theo chuỗi gọi.

Cách làm: chỉ đọc code, không sửa code, không chạy app, không ghi vào `~/.local/share/com.rustsale.crm*`. Mọi `file:dòng` bên dưới đã đọc lại trước khi ghi. Không dùng tên, số điện thoại hay email của khách hàng.

Đã loại các lỗi mà ledger `2026-07-26-google-maps-scraper-production-ledger.md` ghi là đã sửa (AUD-001..012, REV-001..007) và các mục trong hai plan `failed-job-recovery`, `realtime-counters`. Các lỗi dưới đây là lỗi còn hiện diện trong code ở HEAD. Không lỗi nào trùng với danh sách AUD/REV.

Quy ước mức độ: cao = mất dữ liệu im lặng hoặc job kẹt không thoát được bằng UI. Vừa = sai trạng thái hoặc lãng phí nhưng còn đường phục hồi. Thấp = hiếm gặp hoặc chỉ gây khó chịu.

### Bảng tóm tắt

| Mã | Tiêu đề | Mức | Vị trí chính |
|---|---|---|---|
| A1 | RateLimited ở cấp place hết retry thì job `Failed` (kind `database`) thay vì `PausedRequiresAction` | cao | processor.rs:171-174, 337-340; runner.rs:584-588 |
| A2 | Unit bị `fail_job` đánh dấu `database` không thể reset, retry xong job thành `Completed` với `failed_units=0` | cao | repository.rs:386-390, 913-919; runner.rs:427, 731-737 |
| A3 | Job `PausedRequiresAction` không thể resume sau khi khởi động lại app (hoặc khi relaunch trình duyệt thất bại) | cao | runner.rs:189-193, 608-611; processor.rs:337-340; repository.rs:459 |
| A4 | Race pause và `claim_next_unit` để lại unit `running` mồ côi, job kẹt `running` không có runner | vừa | runner.rs:383-395; repository.rs:588-611 |
| A5 | Pause, crash và captcha ăn vào ngân sách retry của unit | vừa | repository.rs:604-607, 150-155, 788-793, 454-462; runner.rs:548 |
| A6 | `finalize` (email enrichment) không huỷ được, pause/cancel trả lỗi sau 5s | vừa | processor.rs:279-307; runner.rs:281, 302, 402 |
| A7 | `wait_for_retry_or_pending` trả `Ok(false)` khi bị huỷ, runner đi vào finalize trên job đã pause/cancel | vừa | runner.rs:396-402, 717-719 |
| A8 | `cancel()` job không active: lỗi timeout giả sau 5s và đóng nhầm browser của job khác | thấp | runner.rs:277-281; processor.rs:353-357 |
| A9 | Lỗi import sau khi checkpoint đã commit làm cả job `Failed` | thấp | repository.rs:719-722; runner.rs:496-504 |
| A10 | Danh sách kind được reset khi retry thiếu `invalid_response` (loại lỗi tạm thời hết retry) | vừa | repository.rs:386-390 |
| A11 | `pause()`/`cancel()` huỷ token trước khi ghi DB, nếu ghi DB lỗi thì job `running` mà không có runner | thấp | runner.rs:295-296 |
| A12 | `retry_failed_units` không atomic: job đã sang `Queued` dù `start` lỗi | thấp | commands.rs:137-150 |
| A13 | Backoff retry quá ngắn, không jitter, không phân biệt rate limit | thấp | runner.rs:344-349; processor.rs:82 |

Chưa chắc (cần xác nhận): A-U1 panic trong task runner làm job treo, A-U2 nhiều instance app cùng DB, A-U3 sidecar và Chrome mồ côi khi thoát app, A-U4 `recover_startup` chạy song song với thao tác người dùng, A-U5 thời gian email enrichment trong trường hợp xấu nhất.

---

### A1. RateLimited cấp place hết retry: job `Failed` thay vì `PausedRequiresAction`

**Mô tả.** Lỗi RateLimited ở trang chi tiết place được tạo với `retryable=true` và đi qua `handle_scrape_error`, hàm này đóng browser. Ba lần retry đầu dùng backoff 2s, 4s, 8s. Ở lần thứ 4, `attempt_count > max_retries` nên nhánh retry không khớp, runner rơi vào nhánh `action_required`. Nhánh đó gọi `action_health`, hàm này cần browser đang mở. Browser vừa bị đóng nên `acquire()` trả `BrowserStartup: browser is not started`. Dấu `?` làm `run_job` trả `Err`, `record_runner_failure` gọi `fail_job` và job thành `Failed` với kind `database` và message "scrape runner failed". Người dùng không được yêu cầu xử lý captcha.

Đường tìm search-level (`listing.blocked_reason`) không đi qua `handle_scrape_error` nên không bị. Chỉ các block ở trang place bị.

**Bằng chứng.**
- `scrapers/google_maps.rs:516-519`: `GoogleMapsPageState::RateLimited => scrape_error(ScrapeErrorKind::RateLimited, ..., true)`.
- `processor.rs:171-174`: `if error.retryable { if let Err(close_error) = manager.close().await {`.
- `runner.rs:547-548`: `Err(error) if error.retryable && unit.attempt_count <= self.retry_policy.max_retries`.
- `runner.rs:573` và `584-588`: nhánh `action_required(error.kind)` rồi `.action_health(&job).await.map_err(|error| anyhow!(error))?;`.
- `processor.rs:337-340`: `action_health` là `browser_for(job)` rồi `manager.acquire().await?.health_snapshot()`.
- `scrapers/browser.rs:708-713`: `acquire` trả `BrowserStartup "browser is not started"` khi `session` là `None`.
- `runner.rs:249-251` và `731-737`: `Err` được `record_runner_failure` chuyển thành `fail_job` với `ScrapeErrorKind::Database`.

**Tái hiện.** Dùng processor giả: `execute` luôn trả `Err(ScrapeJobError{kind: RateLimited, retryable: true, ..})` cho một place unit. `action_health` trả `Err(BrowserStartup)` sau khi `execute` đã gọi `shutdown`. Chạy `run_job` với `RetryPolicy::for_tests()`. Quan sát job `Failed` sau 4 lần `execute`.

**Mức độ: cao.** Rate limit là sự cố phổ biến nhất khi scrape Google Maps lớn. Nó dẫn tới `Failed` sai nhãn và kéo theo A2, tức mất dữ liệu im lặng.

**Đề xuất sửa.**
1. Trong nhánh `action_required`, nếu `action_health` lỗi thì không dùng `?`. Dùng `blocked_url = None`, hoặc lấy URL từ `error.message` (đã có `diagnostic_url=`), rồi vẫn gọi `pause_for_action`. `relaunch_for_action` phải mở được trình duyệt từ `blocked_url` hoặc từ URL của unit.
2. Hoặc `handle_scrape_error` không đóng browser khi `kind` thuộc `action_required`.
3. Nên bổ sung: với RateLimited, bỏ retry ngắn và chuyển sang cooldown dài (xem A13).

**Test hồi quy.** `runner::tests::place_rate_limit_after_retries_pauses_for_action_instead_of_failing`. Assert: `job.status == PausedRequiresAction`, unit `status == Pending`, `job.last_error_kind == Some(RateLimited)`, `action_required.is_some()`. Thêm test mức processor: dùng `BrowserManager::with_automation` giả, `scrape_place` trả trạng thái RateLimited bốn lần, assert `action_health` không trả lỗi `BrowserStartup`.

---

### A2. Unit `database` không reset được; retry xong job `Completed` với `failed_units=0`

**Mô tả.** Khi runner gặp lỗi hạ tầng bất kỳ (A1, A9, lỗi DB `?`, v.v.), `fail_job` đánh dấu mọi unit `running` là `failed` với `last_error_kind='database'`. Nó không tăng `processed_units` hay `failed_units`. Khi người dùng bấm "thử lại", `transition_job(.. UserRetry)` chỉ reset các unit thuộc danh sách kind cố định, không có `database`. Số unit reset là 0 nên job sang `Queued` rồi `Running`. Không còn unit pending, `finalize` chạy, `current.failed_units == 0`, `complete_job_if_exhausted` thấy `open_units == 0` và job thành `Completed`. Unit bị mất (có thể cả một quận hay một ô lưới, cùng toàn bộ place con chưa được lập kế hoạch) không bao giờ được scrape lại và UI báo hoàn tất.

**Bằng chứng.**
- `repository.rs:913-919`: `fail_job` có `UPDATE ... SET status='failed' ... WHERE job_id=?4 AND status='running'`, không cộng bộ đếm.
- `repository.rs:386-390`: `last_error_kind IN ('disconnected', 'navigation_timeout', 'evaluation_timeout', 'transient_network', 'browser_startup', 'runtime_unavailable', 'selector_changed')`, không có `database`.
- `runner.rs:427`: `if outcome.partial || current.failed_units > 0 || current.coverage_truncated`, nhưng `failed_units` không bị `fail_job` đụng tới.
- `repository.rs:877`: `complete_job_if_exhausted` chỉ đếm `pending/running/retry_scheduled`, bỏ qua `failed`.

**Tái hiện.** Tạo job một unit `keyword`. Giả lập `execute` trả `Err` retryable cuối cùng kèm `action_health` lỗi (như A1), hoặc `repository.fail_job` trực tiếp. Gọi `service.retry_failed_units(job_id)`. Assert hiện tại là `Completed` và `failed_units == 0` dù unit vẫn `failed`.

**Mức độ: cao.** Mất dữ liệu im lặng, kết quả hiển thị là "xong".

**Đề xuất sửa.**
1. `fail_job` phải tăng `processed_units` và `failed_units` cho từng unit nó chuyển sang `failed` (dùng `changes()` của câu UPDATE đầu).
2. Thêm `'database'` vào danh sách reset, hoặc tốt hơn gán cho unit kind thật của lỗi gốc thay vì `database`.
3. `complete_job_if_exhausted` nên từ chối `Completed` khi còn unit `failed`, trả `Partial`.

**Test hồi quy.** `repository::tests::fail_job_counts_failed_units_and_retry_resets_them`: sau `fail_job`, assert `failed_units == 1`. Sau `retry_failed_units`, assert unit `pending`, `processed_units` giảm tương ứng. Thêm `job_with_failed_unit_never_completes`: `complete_job_if_exhausted` phải trả `None` hoặc `Partial` khi còn unit `failed`.

---

### A3. Job `PausedRequiresAction` không resume được sau khi khởi động lại app

**Mô tả.** `resume_job` chuyển job `PausedRequiresAction` sang `confirm_action`. `confirm_action` gọi `action_health` để kiểm tra trình duyệt đang hiển thị trang Ready. `action_health` lấy `BrowserManager` trong bộ nhớ qua `browser_for`. Sau khi app khởi động lại, `active_browser` trống nên `browser_for` tạo manager mới chưa start và `acquire()` trả lỗi `BrowserStartup`. `resume` luôn lỗi. `start()` từ chối trạng thái này tường minh. `recover_interrupted_jobs` chỉ xử lý `status='running'` nên job vẫn `paused_requires_action` mãi. Lối thoát duy nhất là huỷ job và mất tiến độ.

Cùng triệu chứng xảy ra khi `relaunch_for_action` lỗi (sidecar không spawn được): `?` ở 608-611 làm runner lỗi, `fail_job` không làm gì (job không còn `running`), và job `PausedRequiresAction` không có trình duyệt.

**Bằng chứng.**
- `commands.rs:124-131`: `if job.status == ScrapeJobStatus::PausedRequiresAction { self.runner.confirm_action(job_id).await }`.
- `runner.rs:166-170`: `start()` trả lỗi "requires action confirmation before it can resume".
- `runner.rs:189-193`: `confirm_action` gọi `self.processor.action_health(&current).await.map_err(|error| anyhow!(error))?;`.
- `processor.rs:337-340` và `51-68`: `browser_for` tạo manager chưa start.
- `repository.rs:454-467`: `recover_interrupted_jobs` chỉ khớp `status='running'`.
- `runner.rs:608-611`: `relaunch_for_action(&paused).await.map_err(...)?;`.

**Tái hiện.** Test mức `GoogleMapsJobService`: tạo job, ép sang `paused_requires_action` bằng `pause_for_action`, tạo `GoogleMapsUnitProcessor` mới (mô phỏng restart), gọi `resume_job`. Assert hiện tại là `Err` ("browser is not started").

**Mức độ: cao.** Captcha thường xuất hiện trong job dài, và người dùng hay đóng app trong lúc chờ xử lý.

**Đề xuất sửa.** Trong `confirm_action`, nếu `action_health` lỗi vì browser chưa chạy thì gọi `relaunch_for_action` (headed, tới `blocked_url`) và trả job không đổi với thông báo "đã mở lại trình duyệt, hãy giải xác minh rồi bấm tiếp tục". Lần bấm sau mới kiểm tra health. Phương án khác: khi khởi động, chuyển `paused_requires_action` sang `interrupted`/`paused` vì bối cảnh trình duyệt đã mất, và cho phép `start` bình thường.

**Test hồi quy.** `commands::tests::resume_after_restart_reopens_action_browser`: `resume_job` không trả `Err` và processor ghi nhận một lần `relaunch_for_action`. Hoặc `recover_startup_moves_paused_requires_action_to_resumable`.

---

### A4. Race pause và `claim_next_unit`: unit `running` mồ côi, job kẹt `running`

**Mô tả.** `run_job` kiểm tra `cancellation` và `status == Running` đầu vòng lặp, sau đó mới `claim_next_unit`. Hai bước tách rời. Nếu `pause()` chen vào giữa (huỷ token, `pause_job` commit: job `paused`, unit `running` được reset thành `pending`), `claim_next_unit` vẫn chạy vì nó không kiểm tra trạng thái job. Nó chuyển một unit sang `running` và tăng `attempt_count`. Ngay sau đó `select! { biased; cancelled => return Ok(()) }` thoát mà không trả unit về `pending`. Unit nằm lại ở `running` trong job `paused`.

Khi resume, `claim_next_unit` chỉ lấy `pending` hoặc `retry_scheduled`, nên unit `running` đó không bao giờ được xử lý. Khi hết việc, `complete_job_if_exhausted` thấy `open_units != 0` và trả `None`, `run_job` thoát với `Ok(())` và job vẫn `running` trong DB mà không còn runner (`clear_active` đã xoá). `start` từ chối trạng thái `Running`, `pause` báo "not the active job". Chỉ huỷ hoặc khởi động lại app (khi `recover_interrupted_jobs` reset unit) mới thoát.

**Bằng chứng.**
- `runner.rs:383-395`: `if cancellation.is_cancelled() { return Ok(()); } ... let job = ...get_job ...; if job.status != Running {return Ok(())} ... let Some(unit) = self.repository.claim_next_unit(job_id).await? else {`.
- `repository.rs:588-599` và `604-611`: câu SELECT/UPDATE của `claim_next_unit` không có điều kiện `job.status`.
- `runner.rs:483-487`: `select! { biased; _ = cancellation.cancelled() => return Ok(()), ...}` thoát mà không giải phóng unit.
- `repository.rs:877-880`: `open_units != 0` trả `None`.

**Tái hiện.** Cửa sổ rất hẹp (một đến hai lượt `spawn_blocking`). Viết test xác định thay vì chạy đua: gọi trực tiếp `repository.pause_job(job)` rồi `repository.claim_next_unit(job)` và assert `claim_next_unit` trả `None` (hiện trả `Some`). Hoặc dùng processor giả có hook chặn để chèn `pause` giữa `get_job` và `claim`.

**Mức độ: vừa.** Xác suất thấp mỗi lần pause, nhưng hậu quả là job treo vĩnh viễn trong phiên đó. Pause là thao tác thường xuyên.

**Đề xuất sửa.**
1. `claim_next_unit` đặt điều kiện `AND EXISTS (SELECT 1 FROM google_maps_scrape_jobs WHERE id=?1 AND status='running')` trong cùng transaction (hoặc `UPDATE ... WHERE job status='running'`).
2. Khi `run_job` thoát do cancel sau claim, trả unit về `pending` (hoặc để `pause_job`/`cancel_job` là nơi duy nhất sửa unit, và đảm bảo claim không thể xảy ra sau chúng như mục 1).
3. Phòng thủ: khi hết việc mà còn unit `running` thì đừng im lặng thoát, chuyển job `Interrupted` hoặc reset unit.

**Test hồi quy.** `repository::tests::claim_next_unit_returns_none_for_paused_job`: assert `None` và attempt_count của unit không đổi. `runner::tests::run_job_does_not_leave_running_unit_after_pause`.

---

### A5. Pause, crash và captcha ăn vào ngân sách retry của unit

**Mô tả.** `attempt_count` tăng ở mỗi lần `claim_next_unit`. `pause_job`, `pause_for_action` và `recover_interrupted_jobs` đặt unit về `pending` nhưng không trả lại lượt đã đếm. Điều kiện retry là `attempt_count <= max_retries (3)` tính trên số lần claim tích luỹ. Một unit bị pause hai lần hoặc gặp hai captcha đã ở `attempt_count=3`. Lỗi tạm thời đầu tiên sau đó chỉ còn một lần retry, và từ lần claim thứ 4 mọi lỗi tạm thời đều không được retry: place unit bị đánh dấu `failed` ngay, keyword/grid unit làm cả job `Partial`.

**Bằng chứng.**
- `repository.rs:604-607`: `SET status='running', attempt_count=attempt_count + 1`.
- `repository.rs:150-155` (pause_job), `788-793` (pause_for_action), `454-462` (recover): chỉ đổi `status='pending'`.
- `runner.rs:547-548`: `error.retryable && unit.attempt_count <= self.retry_policy.max_retries`.
- `processor.rs:82`: `unit.attempt_count > 1` còn quyết định có khởi động lại browser, nên attempt_count phản ánh sai cả điều đó.

**Tái hiện.** Unit đã được claim, pause (hoặc `pause_for_action`) hai lần, claim lần thứ ba cho `execute` trả `NavigationTimeout`. Assert hiện tại: `schedule_retry` chỉ được gọi 1 lần thay vì 3.

**Mức độ: vừa.** Làm suy giảm khả năng chịu lỗi đúng lúc job chạy lâu, nhiều lần dừng, nhiều captcha.

**Đề xuất sửa.** Thêm cột `failure_count` (số lần thực sự lỗi), hoặc trả `attempt_count` lại (`MAX(attempt_count-1,0)`) khi `pause_job`/`pause_for_action`/`recover` reset unit chưa hoàn thành. Dùng `failure_count` cho quyết định retry và dùng cờ riêng "browser cần khởi động lại".

**Test hồi quy.** `repository::tests::pause_does_not_consume_retry_budget`: sau claim, `pause_job`, claim lại, assert `attempt_count == 1`. `runner::tests::unit_still_gets_three_retries_after_two_pauses`.

---

### A6. `finalize` không huỷ được, pause/cancel báo lỗi giả sau 5s

**Mô tả.** Giai đoạn `finalize` được chạy ngoài `select!`, không nhìn `CancellationToken`. Vòng lặp enrichment xử lý tuần tự từng website (mỗi request tối đa 15s, tối đa 5 trang HTML mỗi site). Khi người dùng pause hoặc cancel trong lúc đó, DB đổi trạng thái ngay, nhưng task runner vẫn tiếp tục gọi HTTP và ghi `save_enrichment_outcome` cho tới khi gặp `set_stage(Importing)` (vì job không còn `running` nên lỗi). `wait_for_idle(5s)` hết hạn nên `pause()`/`cancel()` trả `Err("timed out waiting for scrape job runner")` dù thao tác đã thành công trong DB. UI hiện lỗi. Tiếp theo `finalize` lỗi, `fail_job` không làm gì vì job không còn `running`. Công việc enrichment bị bỏ phí.

**Bằng chứng.**
- `runner.rs:402`: `let outcome = match self.processor.finalize(&job).await {` (không có `select!` với `cancellation`).
- `processor.rs:279-307`: vòng `for result in pending { ... enricher.find_email(url).await ... save_enrichment_outcome ...}` không kiểm tra huỷ.
- `runner.rs:281` và `302`: `self.wait_for_idle(Duration::from_secs(5)).await?;` trả lỗi khi timeout.
- `repository.rs:183`: `set_stage ... WHERE id=?3 AND status='running'`.

**Tái hiện.** Processor giả với `finalize` ngủ 10s (hoặc `EmailEnricher` giả chậm). Chạy job tới finalize, gọi `runner.pause(job)`. Assert hiện tại: `pause` trả `Err` sau ~5s, nhưng `repository.get_job` cho thấy `paused`.

**Mức độ: vừa.** Thông báo sai và chặn thao tác kế tiếp trong vài chục giây, đôi khi vài phút.

**Đề xuất sửa.** Truyền `CancellationToken` vào `finalize` và bọc mỗi `find_email` bằng `select!`. Khi bị huỷ thì trả unit enrichment (`claimed`) về `pending`. Hoặc để `pause()` chỉ chờ idle ngắn rồi trả `Ok` kèm cảnh báo. Đồng thời reset kết quả `running` về `pending` khi bị huỷ (xem phần cải tiến).

**Test hồi quy.** `runner::tests::pause_during_finalize_returns_promptly`: `finalize` giả chờ `cancellation.cancelled()`; assert `pause` trả `Ok` trong dưới 1s và không còn result nào ở `enrichment_status='running'`.

---

### A7. `wait_for_retry_or_pending` trả `Ok(false)` khi bị huỷ nên runner finalize trên job đã dừng

**Mô tả.** Hàm này trả `Ok(true)` nghĩa là còn việc, `Ok(false)` nghĩa là không còn việc. Khi token bị huỷ trong lúc chờ unit `retry_scheduled` tới hạn, nó trả `Ok(false)`. Chỗ gọi hiểu là "đã hết unit" và chạy `finalize` cho job vừa bị pause/cancel. Vì `pause()` huỷ token trước khi `pause_job` ghi DB, `set_stage(EmailEnrichment)` vẫn thành công trong khoảng thời gian đó, nên enrichment và import có thể chạy cho job mà người dùng vừa dừng (kết hợp với A6). Đường vào: resume job còn unit `retry_scheduled` chưa tới hạn (pause không reset unit này), sau đó người dùng pause hoặc cancel lần nữa trong lúc chờ.

**Bằng chứng.**
- `runner.rs:717-720`: `tokio::select! { _ = cancellation.cancelled() => Ok(false), _ = tokio::time::sleep(delay) => Ok(true), }`.
- `runner.rs:396-402`: `if self.wait_for_retry_or_pending(job_id, &cancellation).await? { continue; } let outcome = match self.processor.finalize(&job).await {`.
- `runner.rs:420`: kiểm tra `cancellation.is_cancelled()` chỉ nằm sau `finalize`.
- `repository.rs:150-155`: `pause_job` chỉ reset unit `running`, không đụng `retry_scheduled`.

**Tái hiện.** Tạo job có một unit `retry_scheduled` với `next_retry_at` sau 30s. `start`, rồi `pause` sau 100ms. Assert `processor.finalize` được gọi 0 lần (hiện gọi 1 lần).

**Mức độ: vừa.** Tác dụng phụ là chạy HTTP và có thể import lead cho job người dùng đã dừng.

**Đề xuất sửa.** Đổi kiểu trả về thành enum `{HasWork, Drained, Cancelled}` hoặc kiểm tra `cancellation.is_cancelled()` ngay sau khi hàm trả về false.

**Test hồi quy.** `runner::tests::cancel_while_waiting_for_retry_does_not_finalize`: assert `finalize_calls == 0` và job không vào stage `email_enrichment`.

---

### A8. `cancel()` job không active: lỗi timeout giả và đóng nhầm browser của job khác

**Mô tả.** `cancel(job_id)` luôn gọi `processor.shutdown()` và `wait_for_idle(5s)` dù job cần huỷ không phải job đang chạy (ví dụ job `paused`, `interrupted`, `queued`). `shutdown` đóng browser đang giữ trong `active_browser` bất kể `job_id`, nên đóng nhầm browser của job khác đang chạy. Job kia gặp `Disconnected` (retryable) và tốn một lượt retry. `wait_for_idle` chờ vô ích tới 5s rồi trả `Err("timed out waiting for scrape job runner")`, dù `cancel_job` đã ghi DB. UI báo lỗi cho thao tác thành công.

**Bằng chứng.**
- `runner.rs:277-281`: `self.processor.shutdown().await ...?; self.wait_for_idle(Duration::from_secs(5)).await?;`.
- `runner.rs:351-366`: `wait_for_idle` chỉ trả `Ok` khi `active` rỗng hoặc handle xong; job khác đang chạy làm nó chờ hết thời gian.
- `processor.rs:353-357`: `shutdown` không so khớp `job_id`.

**Tái hiện.** Job A `running`, job B `paused`. Gọi `cancel(B)`. Assert hiện tại: `Err` sau 5s, và browser của A bị đóng.

**Mức độ: thấp.** Chỉ xảy ra khi có từ hai job cùng lúc (chỉ một job chạy, các job khác dừng). Nhưng lỗi nhìn thấy được.

**Đề xuất sửa.** Chỉ gọi `shutdown` và `wait_for_idle` khi `job_id` khớp job đang active. Thêm `shutdown_for(job_id)` vào `ScrapeUnitProcessor`.

**Test hồi quy.** `runner::tests::cancel_inactive_job_does_not_touch_active_job`: assert `Ok` ngay, và `shutdown_calls == 0`.

---

### A9. Lỗi import sau checkpoint đã commit làm cả job `Failed`

**Mô tả.** `checkpoint_completed_unit` commit transaction (unit hoàn thành, kết quả lưu, unit con thêm) rồi mới gọi `import_ready_results` bằng `?`. Nếu import lỗi (khoá DB, ràng buộc bảng leads) hàm trả `Err` dù checkpoint đã bền vững. Runner dùng `?` nên `run_job` lỗi, `record_runner_failure` đặt job `Failed` với kind `database`. Không có event `Completed unit` nào được phát. Kết quả vẫn ở `import_status='pending'` và sẽ được import ở `finalize` hoặc ở `retry_failed_units`. Do đó dữ liệu không mất, nhưng job chết vì một lỗi mà thiết kế đã có chỗ phục hồi.

**Bằng chứng.**
- `repository.rs:718-722`: `tx.commit()?; if should_import_ready { crate::google_maps_jobs::importer::import_ready_results(&mut conn, &job_id).map_err(anyhow::Error::from)?; }`.
- `runner.rs:496-504`: `.checkpoint_completed_unit(...).await?;`.

**Tái hiện.** Chèn trigger hoặc ràng buộc làm bảng leads từ chối một dòng, chạy một place unit, assert job `Failed` dù unit `completed`.

**Mức độ: thấp.** Cần lỗi import, nhưng hậu quả là job dừng hẳn.

**Đề xuất sửa.** Bắt lỗi import, ghi log và tiếp tục (kết quả còn `pending` sẽ được import ở `finalize`). Bộ đếm phản ánh đúng số lead đã tạo.

**Test hồi quy.** `repository::tests::checkpoint_survives_import_failure`: assert `checkpoint_completed_unit` trả `Ok` và `import_status='pending'` còn nguyên.

---

### A10. Danh sách kind được reset khi retry thiếu `invalid_response`

**Mô tả.** Danh sách kind trong `transition_job` quyết định unit `failed` nào được đưa về `pending` khi người dùng bấm "thử lại". Danh sách thiếu `invalid_response`. Nhưng `place_state_error` trả `InvalidResponse` với `retryable=true` cho trường hợp panel của place chưa render (EmptyResults/Ready), một lỗi tạm thời. Sau 3 lần retry, unit thành `failed` với kind `invalid_response` và nút "thử lại" không làm gì: `reset_count=0`, job quay lại `Partial` vì `failed_units > 0`. Người dùng rơi vào vòng lặp không có hiệu lực. Ngoài ra cùng danh sách này thiếu `database` (A2).

**Bằng chứng.**
- `scrapers/google_maps.rs:531-535`: `EmptyResults | Ready => scrape_error(InvalidResponse, "... did not render a place panel", true)`.
- `repository.rs:386-390`: danh sách kind không có `invalid_response`.
- `runner.rs:427`: `current.failed_units > 0` đưa job về `Partial` ngay.

**Tái hiện.** Place unit với `execute` luôn trả `InvalidResponse retryable=true`. Sau khi job `Partial`, gọi `retry_failed_units`. Assert hiện tại: unit vẫn `failed`.

**Mức độ: vừa.** Làm mất khả năng khôi phục của các place bị lỗi tạm thời.

**Đề xuất sửa.** Lưu cờ `retry_exhausted` hoặc `retryable` cùng unit, reset theo cờ đó thay vì theo danh sách kind cố định. Cách nhanh: thêm `invalid_response` vào danh sách.

**Test hồi quy.** `repository::tests::retry_resets_retry_exhausted_invalid_response_units`.

---

### A11. `pause()`/`cancel()` huỷ token trước khi ghi DB

**Mô tả.** `pause` gọi `cancellation.cancel()` rồi mới `pause_job`. Nếu `pause_job` lỗi (DB busy hơn 5s, hoặc job vừa sang trạng thái khác) thì runner đã dừng im lặng ở lần kiểm tra token kế tiếp mà DB vẫn ghi `running`. Job không có runner: `start` từ chối `Running`, `clear_active` đã xoá entry nên `pause` báo "not the active job". Thoát chỉ bằng cancel hoặc khởi động lại app.

**Bằng chứng.** `runner.rs:295-296`: `cancellation.cancel(); let paused = self.repository.pause_job(job_id).await?;`.

**Mức độ: thấp.** Cần lỗi ghi DB hiếm gặp.

**Đề xuất sửa.** Ghi DB trước, rồi huỷ token. Hoặc nếu `pause_job` lỗi thì đặt lại cờ (token mới) và để runner tiếp tục. Chú ý thứ tự này tương tác với A4, nên sửa A4 trước.

**Test hồi quy.** `runner::tests::pause_failure_keeps_runner_alive`: repository giả khiến `pause_job` lỗi, assert `run_job` vẫn tiếp tục claim unit.

---

### A12. `retry_failed_units` không atomic

**Mô tả.** Hàm này `import_ready_results`, `transition_job(Queued, UserRetry)` (reset unit, tăng `run_generation`, xoá `completed_at`), rồi `runner.start`. Nếu `start` lỗi (đang có job khác active, hoặc phiên bản cũ) thì lệnh trả `Err` nhưng DB đã đổi: job `Queued`, unit đã reset. UI nhận lỗi nhưng trạng thái thực đã đổi. Job `Queued` vẫn bắt đầu được qua nút tiếp tục nên không mất dữ liệu.

**Bằng chứng.** `commands.rs:137-150`: `transition_job(..., Queued, UserRetry)` rồi `self.runner.start(&queued.id).await`.

**Mức độ: thấp.**

**Đề xuất sửa.** Kiểm tra trước (runner có job active hay không) khi chưa đổi DB, hoặc phát event `Queued` để UI đồng bộ.

**Test hồi quy.** `commands::tests::retry_while_other_job_active_leaves_job_unchanged`: assert `status` và số unit `failed` không đổi khi `retry_failed_units` trả `Err`.

---

### A13. Backoff retry quá ngắn và không jitter

**Mô tả.** `retry_delay = base(2s) * 2^(attempt-1)`, tức 2s, 4s, 8s, cộng dồn 14s, không jitter. Mọi loại lỗi dùng cùng lịch. Với RateLimited đó là quá ngắn (Google thường chặn hàng phút). Mỗi lần retry còn khởi động lại browser (`attempt_count > 1`) và gửi thêm request trong lúc đang bị chặn, có thể kéo dài thời gian bị chặn. Trong thời gian chờ, `run_job` ngủ tại chỗ nên các unit khác đang `pending` cũng bị chặn (head-of-line).

**Bằng chứng.** `runner.rs:344-349` (`saturating_mul(1u32 << exponent)`); `runner.rs:560-571` (ngủ trong vòng lặp); `processor.rs:82`.

**Mức độ: thấp.** Là vấn đề thiết kế, nhưng tăng khả năng gặp A1.

**Đề xuất sửa.** Chính sách theo loại lỗi: rate limit cooldown dài (1-5 phút) và chuyển sang `PausedRequiresAction` sau lần đầu. Thêm jitter ±25%. Không ngủ tại chỗ, thay vào đó lấy unit khác (claim đã hỗ trợ `next_retry_at`).

**Test hồi quy.** `runner::tests::rate_limit_pauses_for_action_without_burning_retries`, `retry_delay_has_jitter_within_bounds`.

---

### Mục chưa chắc (cần xác nhận)

**A-U1. Panic trong task runner làm job treo `running`.** Task `tokio::spawn` ở `runner.rs:247-253` không có `catch_unwind`. Panic làm `clear_active` không chạy, job vẫn `running` trong DB. Entry `active` vẫn giữ handle đã kết thúc nên `pause` còn dùng được, nhưng UI báo `running` mãi. Chưa tìm thấy `unwrap`/`expect`/chỉ mục/chia cho 0 nào có thể panic trong đường production của các file trong phạm vi (`processor.rs:71` có `expect` nhưng luôn an toàn vì ngay trên đó đã gán `Some`; `validation.rs` có kiểm tra hữu hạn và `max(1e-9)`). Cần: tìm panic trong `scrapers/*.rs`, `importer.rs`; hoặc thêm test cho processor panic để xem job có kẹt không. Sửa: bọc `run_job` trong `AssertUnwindSafe(...).catch_unwind()` hoặc theo dõi `JoinHandle` bằng task giám sát để gọi `fail_job`.

**A-U2. Hai instance app dùng chung DB.** Grep không thấy plugin single-instance trong `Cargo.toml`, `lib.rs`, `capabilities`. Instance thứ hai chạy `recover_startup` sẽ chuyển các job `running` của instance đầu sang `interrupted` và reset unit của chúng, và hai tiến trình có thể cùng dùng một thư mục profile trình duyệt. Cần: khởi chạy hai instance trên máy thử. Sửa: `tauri-plugin-single-instance`, hoặc lease/heartbeat (cột `owner_id` và `heartbeat_at`) để `recover` chỉ nhận job quá hạn.

**A-U3. Sidecar và Chrome mồ côi khi thoát app.** `lib.rs:693-703` chỉ có `on_window_event` cho tray và `.run(...)`, không có `RunEvent::Exit`/`ExitRequested` để gọi `processor.shutdown()`. Sidecar chỉ có `kill_on_drop(true)` (`scrapers/browser.rs:200`) và `kill_on_drop` chỉ tác dụng khi `Child` bị drop, mà khi tiến trình thoát bằng `process::exit` thì không có drop. Chrome là tiến trình con của sidecar nên có thể sống tiếp và giữ khoá thư mục profile (khiến lần mở sau gặp `BrowserStartup`). Cần: chạy app, bắt đầu job, thoát bằng menu tray, rồi `pgrep` tìm sidecar/Chrome (không được thực hiện theo ràng buộc không chạy app). Sửa: `app.run(|handle, event| if let RunEvent::Exit = event { block_on(shutdown) })`, và trên Linux/Windows đặt job object hoặc `pdeathsig`.

**A-U4. `recover_startup` chạy bất đồng bộ song song với thao tác người dùng.** `lib.rs:290-292` chạy `recover_startup` trong `spawn`. Nếu người dùng (hoặc lệnh tự động) bắt đầu job trong vài ms đầu, `recover_interrupted_jobs` có thể đánh dấu job vừa `running` thành `interrupted`. Runner đang chạy sẽ thoát ở lần kiểm tra trạng thái kế tiếp, còn checkpoint đang dở sẽ lỗi `requires a running job`. Cần: test chèn `recover_interrupted_jobs` ngay sau `start`. Sửa: chờ `recover_startup` xong (hoặc dùng `OnceCell`) trước khi nhận lệnh `start`.

**A-U5. Thời gian enrichment xấu nhất.** `email.rs:15-19` đặt timeout kết nối 5s, request 15s, tối đa 3 redirect và 5 trang HTML mỗi website, tuần tự. Trường hợp xấu nhất khoảng 5 trang × 4 request × 15s = 5 phút cho một site chết chậm, nhân với số result chưa có số điện thoại. Chưa đo được tần suất thực tế. Cần: đo trên job thật hoặc test với server chậm.

---

### Cải tiến đáng làm (có căn cứ trong code)

**Tốc độ**
1. **Thứ tự claim là theo chiều rộng.** `claim_next_unit` sắp xếp `created_at ASC, id ASC` (`repository.rs:594`). Unit place con được tạo sau mọi unit ban đầu nên với 500 quận, toàn bộ 500 trang tìm kiếm chạy trước, rồi mới tới từng place. Lead đầu tiên xuất hiện rất muộn, và nếu bị chặn hay crash giữa chừng thì chưa import được gì. Đề xuất ưu tiên `unit_type='place'` (hoặc ưu tiên theo `depth`) để import sớm và giảm thời gian tới lead đầu tiên.
2. **Mỗi unit tìm kiếm tải lại toàn bộ khoá.** `listing_to_execution` gọi `list_result_dedup_keys` và `list_place_unit_keys` cho cả job (`processor.rs:189-201`). Chi phí O(N) mỗi unit tìm kiếm nên O(N²) với job hàng chục nghìn place. Dùng truy vấn `WHERE unit_key IN (...)` chỉ cho các URL của trang hiện tại.
3. **`run_job` đọc DB nhiều lần mỗi vòng.** Hai lần `get_job` (386 và 723 qua `job_is_running`), cộng `count_place_unit_outcomes` mỗi lần lỗi (636-637). Gộp bằng một truy vấn hoặc dùng `version` trả về từ checkpoint.
4. **Enrichment tuần tự** (`processor.rs:279-307`). Chạy song song 4-8 luồng có giới hạn theo domain (đã có chính sách domain trong `email.rs`), kèm hủy được (A6).
5. **Ngủ tại chỗ khi retry** (`runner.rs:568-571`) chặn các unit khác. Lấy unit sẵn sàng khác trong lúc chờ.

**Độ bền**
1. **Lease và heartbeat.** Hiện chỉ có `status='running'` và `recover_interrupted_jobs` ở khởi động. Thêm `owner_id` và `heartbeat_at` cập nhật theo checkpoint, để (a) phát hiện job treo khi runner chết (A4, C1), (b) an toàn khi nhiều instance (C2).
2. **Reset result enrichment `running` khi huỷ hoặc crash.** `claim_result_for_enrichment` đặt `enrichment_status='running'` (`repository.rs:262-267`) nhưng `recover_interrupted_jobs` chỉ reset unit, không reset result. Một crash giữa enrichment để result ở `running` mãi, và `list_results_for_enrichment` lọc `('pending','retry_wait')` nên chúng không bao giờ được thử lại, và do đó lead đó không được import (vì `import_status` phụ thuộc enrichment). Nên reset `running` về `pending` trong `recover_interrupted_jobs` và khi pause/cancel. Chưa kiểm chứng đầy đủ đường import, nhưng phần lọc đã rõ ràng trong code.
3. **Cho phép "mở lại trình duyệt xử lý captcha" như một lệnh riêng** (A3), kèm hướng dẫn khi `confirm_action` thấy trang vẫn bị chặn (hiện `confirm_action` lặng lẽ trả job không đổi, `runner.rs:194-199`).
4. **Hook thoát app** để đóng sidecar sạch (C3).
5. **Dọn log và diagnostics.** `tracing_appender::rolling::daily` (`diagnostics.rs:15`) không có giới hạn số tệp giữ lại, ảnh chụp chẩn đoán được ghi mỗi lần lỗi (`processor.rs:407-421`). Thêm retention theo số ngày/dung lượng.
6. **`last_sequences` trong `ScrapeEventPublisher` chỉ tăng** (`events.rs:8, 30`): mỗi job thêm một khoá không bao giờ xoá. Kích thước nhỏ nhưng nên xoá khi job vào trạng thái terminal.

**Chất lượng dữ liệu**
1. **Cố định `failed_units` khi runner fail** (A2) và hiển thị tách "unit lỗi do hạ tầng" với "place lỗi".
2. **Message lỗi được ghi cùng URL chẩn đoán** (`processor.rs:413-415`, `diagnostic_url=`). URL Google khi bị chặn có thể chứa tham số `continue`, token. `redact_sensitive_fields` chỉ lọc theo tên khoá JSON (`diagnostics.rs:76-85`), không lọc giá trị chuỗi. Nên cắt query string trước khi ghi vào message và log.
3. **Ngưỡng `FailureTolerance` tính từ đầu job.** `max_total_failures=50` tính trên toàn bộ place `failed` (`repository.rs:544-558`), nên job đã retry vẫn mang số lỗi cũ và dừng sớm khi gặp lỗi mới. Cân nhắc đếm trong cửa sổ trượt hoặc theo `run_generation`.
4. **Retry thất bại im lặng.** `retry_failed_units` trả job `Partial` ngay khi `failed_units>0` mà không báo bao nhiêu unit đã được reset. Trả `reset_count` cho UI.

### Ghi chú về phạm vi

Không đọc `*_test.rs`. Các test hồi quy đề xuất dùng mô hình test hiện có trong `runner.rs` (processor giả theo trait `ScrapeUnitProcessor`, `RetryPolicy::for_tests()`) và `repository` trên SQLite tạm. Chưa chạy bất kỳ test nào.

---

# Phần B. Dữ liệu, lưới, email


Phạm vi: chỉ đọc code, không sửa code, không chạy app, không đụng `~/.local/share/com.rustsale.crm*`. Báo cáo không chứa tên, số điện thoại hay email của khách. Mọi ví dụ dùng dữ liệu giả.

Gốc: `/home/arch/Project/test/RustSale_v2/src-tauri/src/`. Số dòng lấy từ lần đọc cuối (không phải bản sao cũ).

Mức "chưa chắc" nghĩa là đọc code thấy nguy cơ nhưng chưa có bằng chứng chạy thật. Mỗi mục ghi rõ cần gì để xác nhận.

---

### 0. Schema 3 bảng `google_maps_scrape_*` (từ `db.rs:385-511`)

Không có file migration SQL riêng. Schema nằm inline trong `db.rs`, dùng `CREATE TABLE IF NOT EXISTS` kèm `add_column_if_missing` cho 3 cột thêm sau (`profile_dir`, `blocked_url`, `coverage_truncated`).

#### `google_maps_scrape_jobs` (db.rs:386-416)
| Cột | Kiểu / ràng buộc |
|---|---|
| id | TEXT PRIMARY KEY |
| mode, keyword | TEXT NOT NULL |
| location, bbox_json | TEXT |
| step_km | REAL |
| headless | INTEGER NOT NULL CHECK IN (0,1) |
| profile_dir | TEXT NOT NULL DEFAULT '' |
| blocked_url | TEXT |
| status, stage | TEXT NOT NULL |
| total_units, processed_units, failed_units, businesses_discovered, businesses_qualified, leads_created | INTEGER NOT NULL DEFAULT 0 CHECK >= 0 |
| last_error_kind, last_error_message, action_required | TEXT |
| event_sequence, version, run_generation | INTEGER NOT NULL DEFAULT 0 CHECK >= 0 |
| created_at, updated_at | TEXT NOT NULL |
| started_at, completed_at | TEXT |
| coverage_truncated | INTEGER NOT NULL DEFAULT 0 CHECK IN (0,1) |

#### `google_maps_scrape_units` (db.rs:440-460)
| Cột | Kiểu / ràng buộc |
|---|---|
| id | TEXT PRIMARY KEY |
| job_id | TEXT NOT NULL, FK -> jobs(id) ON DELETE CASCADE |
| unit_key, unit_type, payload_json, status | TEXT NOT NULL |
| depth | INTEGER NOT NULL DEFAULT 0 CHECK >= 0 |
| attempt_count | INTEGER NOT NULL DEFAULT 0 CHECK >= 0 |
| next_retry_at, last_error_kind, last_error_message, completed_at | TEXT |
| created_at, updated_at | TEXT NOT NULL |
| UNIQUE | (job_id, unit_key), (job_id, id) |

#### `google_maps_scrape_results` (db.rs:463-495)
| Cột | Kiểu / ràng buộc |
|---|---|
| id | TEXT PRIMARY KEY |
| job_id | TEXT NOT NULL, FK -> jobs(id) ON DELETE CASCADE |
| source_unit_id | TEXT NOT NULL, FK (job_id, source_unit_id) -> units(job_id, id) ON DELETE CASCADE |
| place_id, canonical_url | TEXT |
| dedup_key, name | TEXT NOT NULL |
| phone, normalized_phone, email, normalized_email, address, website, facebook, zalo | TEXT |
| latitude, longitude | REAL |
| inside_requested_area | INTEGER CHECK IN (0,1) (nullable) |
| detail_status, enrichment_status, import_status | TEXT NOT NULL |
| last_error_kind, last_error_message | TEXT |
| first_seen_at, last_seen_at | TEXT NOT NULL |
| UNIQUE | (job_id, dedup_key) |

#### Index (db.rs:497-511, và bảng `leads` dùng bởi importer)
- `idx_google_maps_jobs_status` ON jobs(status, updated_at)
- `idx_google_maps_units_job_status` ON units(job_id, status, next_retry_at)
- `idx_google_maps_results_job_import` ON results(job_id, import_status, enrichment_status)
- Ngoài ra: `idx_leads_phone` ON leads(phone) (db.rs:513), `idx_leads_email_normalized`.
- Không có CHECK cho giá trị `status` / `import_status` / `enrichment_status` (chỉ là TEXT tự do, kiểm bằng code Rust).
- Dedup chỉ có phạm vi từng job (UNIQUE (job_id, dedup_key)). Không có khoá chống trùng xuyên job. Trùng với `leads` chỉ được chặn lúc import bằng điện thoại và email (xem B8, B12).

---

### 1. Bảng tóm tắt

| Mã | Tiêu đề | Mức | File:dòng |
|---|---|---|---|
| B1 | Fallback website lấy bừa link ngoài đầu tiên, dẫn tới email lead sai | Cao | scrapers/google_maps.rs:822-826 |
| B2 | Trích email web ra rác (`image@2x.png`, đường dẫn có `/`, hash, placeholder) | Vừa-cao | google_maps_jobs/email.rs:835-843, 592-640 |
| B3 | Kết quả enrichment `running` mồ côi sau crash, không bao giờ được claim lại | Vừa | google_maps_jobs/repository.rs:208-210, 263-266, 450-474 |
| B4 | Một trang con lỗi làm hỏng toàn bộ enrichment của website (bỏ mất email đã/chưa tìm) | Vừa | google_maps_jobs/email.rs:473 |
| B5 | `robots.txt` trả 401/403 bị coi là lỗi cứng thay vì "cho phép"; lỗi Content-Type cũng hỏng cả lượt | Vừa | google_maps_jobs/email.rs:296-315 |
| B6 | Import nạp toàn bộ số điện thoại của `leads` vào RAM mỗi checkpoint, lookup email không dùng index | Vừa | google_maps_jobs/importer.rs:123-135, 141-146 |
| B7 | Bước nhảy zoom làm mất khả năng chia nhỏ ở dải radius [2400,2500) và >= 20000 m | Vừa | scrapers/grid.rs:46-62, 180-210 |
| B8 | Planner/validation và lưới thật dùng công thức khác nhau; bước `step_km` không bị chặn ở backend | Vừa | scrapers/grid_planner.rs:68-97, google_maps_jobs/commands.rs:429-439 |
| B9 | Cờ Partial của job bị bật bởi lỗi enrichment không retry được | Vừa | google_maps_jobs/processor.rs:302 |
| B10 | `upsert_result` ghi đè `detail_status`/`enrichment_status`/`import_status`/lỗi bằng giá trị mới khi gặp lại cùng địa điểm | Thấp-vừa | google_maps_jobs/repository.rs:1475-1479 |
| B11 | Chuẩn hoá SĐT VN thiếu trường hợp (trùng tiền tố sai, số bàn, `84` + 9 số) | Thấp | google_maps_jobs/importer.rs:24-46 |
| B12 | Import chạy sau khi checkpoint đã commit; lỗi import làm job lỗi dù dữ liệu đã lưu | Thấp | google_maps_jobs/repository.rs:718-722 |
| B13 | SSRF: bỏ sót NAT64/6to4 trong blocklist IP | Thấp | google_maps_jobs/email.rs:642-680 |
| B14 | `depth as u8` ép kiểu cắt giá trị | Thấp | google_maps_jobs/repository.rs:1341 |
| B15 | `claim_next_unit` ORDER BY không có index khớp | Thấp | google_maps_jobs/repository.rs:581-600 |

Chưa chắc (xem phần 3): B-U1 icon glyph trong địa chỉ, B-U2 span viewport so với zoom, B-U3 version cũ trong runner sau import.

---

### 2. Chi tiết lỗi

#### B1. Fallback website lấy bừa link ngoài đầu tiên (Cao)
- Mô tả: nếu không tìm được nút "authority" (website chính thức), script quét mọi `a[href]` trong trang. Link `http` đầu tiên không chứa `google.`, `gstatic.com`, `facebook.com`, `zalo.me` được gán làm `website`. Trên trang Maps, link ngoài đầu tiên thường là nền tảng đặt chỗ, báo, link quảng cáo, đối tác giao hàng hoặc bình luận người dùng, chứ không phải site của cơ sở.
- Hệ quả: enrichment crawl site sai, rút email của bên thứ ba, rồi email đó được import thành lead của cơ sở này. Đây là lỗi chất lượng dữ liệu âm thầm, khó phát hiện về sau, và có thể gửi mail cho người không liên quan.
- Bằng chứng:
  - `scrapers/google_maps.rs:822` `if (!website && href.startsWith('http') &&`
  - `scrapers/google_maps.rs:823-825` loại trừ chỉ `google.`, `gstatic.com`, `facebook.com`, `zalo.me`
  - `scrapers/google_maps.rs:826` `website = href;`
- Repro: mở 1 địa điểm không có website nhưng có link "Đặt chỗ"/"Đặt hàng" tới một domain bên thứ ba trong panel. `website` sẽ là domain đó. Kiểm lại bằng test script thuần: dựng DOM giả có 1 `a[href="https://booking.example/..."]` và không có `authority`, gọi script, kỳ vọng `website === ''`.
- Mức: Cao vì ảnh hưởng trực tiếp lead sai và nằm trong đường chính của mọi job.
- Đề xuất: bỏ hẳn nhánh fallback. Chỉ lấy `data-item-id="authority"` (đã có ở dòng 815). Nếu cần fallback, giới hạn trong container nút hành động (`[data-item-id]`/`[role="region"]` của phần thông tin) và loại danh sách domain đặt chỗ, mạng xã hội, link `/url?q=`.
- Test hồi quy: `google_maps_script_does_not_use_arbitrary_external_link_as_website`; assert `website` rỗng khi chỉ có link ngoài không nằm trong nút `authority`.

#### B2. Trích email web ra rác (Vừa-cao)
- Mô tả:
  - Regex `[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@...` cho phép `/` và nhiều ký tự hiếm trong phần local, nên chuỗi URL hay đường dẫn kề `@` có thể ra một "email" có `/`.
  - `normalize_email` chấp nhận các domain kiểu `2x.png` (TLD `png` toàn chữ cái, hợp lệ), nên chuỗi tên file ảnh retina như `logo@2x.png` thành email.
  - Chỉ loại `example.com/.org/.net`. Không loại: `sentry.io`/`wixpress.com` (hash@sentry...), `domain.com`, `email.com`, `yourdomain.com`, `test.com`, đuôi file (`.png/.jpg/.webp/.svg/.gif/.css/.js`), và địa chỉ no-reply/noreply.
- Bằng chứng:
  - `google_maps_jobs/email.rs:839` regex có `/` trong class local
  - `google_maps_jobs/email.rs:604-608` kiểm local bằng bộ ký tự cho phép `/`
  - `google_maps_jobs/email.rs:626-635` TLD chỉ cần >= 2 chữ cái; chỉ loại example.*
- Repro (dữ liệu giả): HTML chứa `<img src="/img/logo@2x.png">` thì `extract_primary_email` trả `logo@2x.png`. Hoặc văn bản `bán tại https://shop.example/contact/info@shop.example` cho local có `/`.
- Mức: Vừa-cao. Số email rác vào `leads` rất lớn trên site dùng template, gây bounce và hại uy tín gửi.
- Đề xuất:
  1. Thu hẹp regex: local `[a-z0-9._%+-]+`, không `/`.
  2. Từ chối khi TLD nằm trong danh sách đuôi file/asset; từ chối local dạng hash dài (>= 20 ký tự hex) và domain trong blocklist (`sentry.io`, `wixpress.com`, `domain.com`, `email.com`, `yourdomain.com`, `test.com`).
  3. Ưu tiên `mailto:` trước text thường; loại no-reply/noreply/donotreply.
  4. Loại trừ khi match nằm ngay sau `src=`/`url(`.
- Test hồi quy: `email_extraction_rejects_asset_names_and_placeholder_domains`; assert các chuỗi `logo@2x.png`, `a/b@site.vn`, `x@sentry.io`, `name@domain.com` trả `None`, còn `lienhe@congty.vn` vẫn trả `Some`.

#### B3. Enrichment `running` mồ côi sau crash (Vừa)
- Mô tả: claim đặt `enrichment_status='running'`. Nếu app chết giữa chừng (trước `save_enrichment_outcome`), hàng đó kẹt ở `running`. Cả hai đường reset/claim chỉ nhìn `pending`/`retry_wait` (và retry của người dùng chỉ reset `retry_wait`/`failed`). `recover_interrupted_jobs` chỉ reset unit `running` chứ không đụng kết quả.
- Hệ quả: các kết quả đó không bao giờ được enrich lại; job vẫn hoàn tất, nhưng thiếu email âm thầm.
- Bằng chứng:
  - `repository.rs:263` `SET enrichment_status='running'`
  - `repository.rs:208-210` retry: `... enrichment_status IN ('retry_wait', 'failed')`
  - `repository.rs:454-461` recover chỉ `UPDATE google_maps_scrape_units ... status='running'`
- Repro: claim 1 kết quả bằng `claim_next_result_for_enrichment`, bỏ qua `save_enrichment_outcome`, chạy `recover_interrupted_jobs`, rồi `claim_next_result_for_enrichment` lần nữa thì trả `None` dù hàng vẫn `running`.
- Mức: Vừa; chỉ xảy ra khi crash nhưng mất dữ liệu im lặng.
- Đề xuất: trong `recover_interrupted_jobs` (cùng transaction) thêm `UPDATE google_maps_scrape_results SET enrichment_status='pending' WHERE enrichment_status='running' AND job_id IN (jobs đang running)`. Hoặc dùng lease (`claimed_at`) và coi `running` quá hạn là claim được.
- Test hồi quy: `recover_interrupted_jobs_resets_running_enrichment`; assert sau recover, `claim_next_result_for_enrichment` trả lại đúng hàng đó.
- Xác nhận cuối: grep thấy không có chỗ nào khác reset `running` của enrichment (chỉ các dòng trên), nên kết luận theo code là chắc; nên viết test để khoá.

#### B4. Một trang con lỗi làm hỏng cả enrichment (Vừa)
- Mô tả: `find_email` duyệt các link con (liên hệ, giới thiệu...) bằng `fetch_html(link).await?`. Lỗi 404/timeout/Content-Type không phải HTML ở bất kỳ trang con nào sẽ thoát cả hàm với `Err`, bỏ qua các link còn lại và loại luôn kết quả của các trang đã lướt.
- Bằng chứng: `google_maps_jobs/email.rs:473` `let response = self.client.fetch_html(link).await?;`
- Repro: homepage không có email, trang con thứ nhất trả 404, trang con thứ hai có email. Kết quả hiện tại là `Err` (đánh dấu lỗi/retry), kỳ vọng là `Some(email)` từ trang thứ hai.
- Mức: Vừa; site Việt hay có link chết `/lien-he` nên làm giảm tỉ lệ tìm được email.
- Đề xuất: với trang con, bắt lỗi không-retryable và `continue`; chỉ lan lỗi retryable (429/5xx/timeout) nếu cuối cùng không tìm thấy gì.
- Test hồi quy: `find_email_continues_after_dead_subpage`; assert trả email từ trang sau trang 404.

#### B5. `robots.txt` 401/403 và lỗi Content-Type làm hỏng cả lượt (Vừa)
- Mô tả: chỉ 404 được coi là "không có robots". Theo RFC 9309, mọi 4xx của robots.txt nghĩa là cho phép crawl; chỉ 5xx là "unavailable" (thận trọng cấm). Code coi 401/403 là `InvalidResponse` không retry, nên enrichment website lỗi hẳn. Ngoài ra `validate_content_type` chạy cả cho robots/HTML, nên site trả `text/plain` hay `application/octet-stream` cho trang liên hệ đều bị từ chối cứng.
- Bằng chứng:
  - `google_maps_jobs/email.rs:296` chỉ `status == 404` cho Robots
  - `google_maps_jobs/email.rs:303-313` mọi non-2xx còn lại là lỗi (4xx không retry)
  - `google_maps_jobs/email.rs:315` `validate_content_type(...)?`
- Repro: robots trả 403 (Cloudflare hay làm vậy) cho UA lạ: `find_email` kết thúc lỗi `InvalidResponse`, thay vì tiếp tục crawl.
- Mức: Vừa; một phần website bị bỏ vì WAF.
- Đề xuất: robots 4xx là allow-all (body rỗng); 5xx/timeout cho robots là retryable lỗi. Content-Type thiếu/`text/plain` cho HTML: nới (sniff `<html`) hoặc bỏ qua trang đó (xem B4).
- Test hồi quy: `robots_403_is_treated_as_allow_all`; assert `find_email` vẫn crawl và trả email.

#### B6. Import nạp toàn bộ `leads` mỗi checkpoint (Vừa)
- Mô tả: `import_results` chạy sau mỗi checkpoint kết quả. Mỗi lần nó `SELECT phone FROM leads WHERE trim(phone) <> ''` toàn bảng, chuẩn hoá từng hàng vào `HashSet` (không dùng index, tốn O(N) bộ nhớ và CPU mỗi lượt). Kèm theo, lookup email `lower(trim(email))=?1` làm hàm hoá cột nên không tận dụng được `idx_leads_email_normalized` (nếu index nằm trên cột `email_normalized`), tức là quét toàn bảng cho mỗi hàng có email (N+1 x quét).
- Bằng chứng:
  - `google_maps_jobs/importer.rs:123-135` nạp toàn bộ phone
  - `google_maps_jobs/importer.rs:141-146` `phone_lookup`/`email_lookup` theo từng hàng, `lower(trim(email))`
- Repro: bảng `leads` 200k hàng, mỗi checkpoint (mỗi unit) tốn hàng trăm ms đến giây CPU trong transaction `Immediate` (chặn mọi writer khác, kể cả UI).
- Mức: Vừa; không sai dữ liệu nhưng càng dùng lâu càng chậm và giữ khoá ghi lâu.
- Đề xuất: bỏ `known_phones` nạp toàn bộ. Dùng cột `leads.phone_normalized` (hoặc chỉ mục biểu thức) và truy vấn `WHERE phone_normalized=?1`; email dùng `email_normalized=?1` đúng index.
- Test hồi quy: `import_does_not_scan_all_leads` (kiểm bằng `EXPLAIN QUERY PLAN` rằng lookup dùng index, assert không có `SCAN leads`).

#### B7. Bước nhảy zoom chặn chia nhỏ ô (Vừa)
- Mô tả: `should_subdivide` chỉ chia khi zoom của ô con lớn hơn zoom của ô cha. Các ngưỡng `radius_to_zoom` (10000/5000/2500/1200/600/300) cách nhau gần đúng 2x, nhưng tỉ lệ 2500/1200 = 2.083 nên:
  - radius cha trong [2400, 2500) có zoom 15, con có radius [1200, 1250) cũng zoom 15, không chia nhỏ dù ô còn rộng và đã bão hoà;
  - radius cha >= 20000 m: zoom 12, con >= 10000 vẫn zoom 12, nên không bao giờ chia.
  Kết quả: ô bão hoà ở dải đó giữ nguyên, bỏ sót địa điểm (mất coverage không báo, vì `coverage_truncated` không bật cho trường hợp này).
- Bằng chứng:
  - `scrapers/grid.rs:46-62` các ngưỡng
  - `scrapers/grid.rs:209-210` `radius_to_zoom(child.radius_m) > radius_to_zoom(cell.radius_m)`
- Repro (số học): ô radius 2450 m, bão hoà. `radius_to_zoom(2450)=15`, con 1225 m `=15`, `should_subdivide` trả false.
- Mức: Vừa; coverage hụt trong dải hẹp, nhưng lưới với bước 5 km (radius ~3.5 km) hay đi qua dải gần đó.
- Đề xuất: không phụ thuộc zoom để quyết định chia; chia dựa vào `min_cell_size_m`, `max_depth`, bão hoà. Nếu cần zoom khác nhau cho viewport, tính zoom liên tục (`log2`) rồi làm tròn, và cho phép chia kể cả khi zoom không đổi. Bật `coverage_truncated` khi bị chặn mà ô vẫn bão hoà.
- Test hồi quy: `saturated_cell_with_radius_2450_subdivides`; assert `should_subdivide(...) == true` (hoặc `coverage_truncated` được bật nếu giữ hành vi cũ).

#### B8. Planner và lưới thật không khớp; step không bị chặn (Vừa)
- Mô tả:
  - Preview dùng `GridPlanner` (R=6371.0088 km, chiều rộng theo cos vĩ độ gần xích đạo nhất, step bị kẹp [0.2, 50]) trong khi `validation.rs` dùng 111.0 km/độ với cos lớn nhất, và lưới thật (`generate_initial_cells`) dùng phép đổi 111.195 km/độ. Ba công thức cho ba con số ô khác nhau, nên cảnh báo "vượt giới hạn" và "step đề xuất" có thể lệch so với số ô thực.
  - Backend nhận `step_km` thô: `plan_initial_units` truyền thẳng `step_km` vào `generate_initial_cells`, không kẹp [0.2, 50] như planner. Frontend chỉ chặn bằng slider 0.5-10 km, tức là không bảo vệ khi gọi lệnh trực tiếp. `step_km` rất nhỏ hoặc 0/âm phụ thuộc kiểm tra trong `generate_initial_cells`; cần xác nhận (xem "Cần xác nhận").
- Bằng chứng:
  - `scrapers/grid_planner.rs:68-97` công thức riêng + clamp
  - `google_maps_jobs/commands.rs:429-439` `generate_initial_cells(bounding_box, step_km, ...)` với `step_km` thô
  - `src/lib/components/GoogleMapsScraper.svelte:714` slider `min="0.5" max="10.0"` (chỉ UI)
- Repro: gọi `create_google_maps_scrape_job` với `step_km = 0.05` và bbox lớn; so số ô `GridPlanner::plan` (kẹp thành 0.2) với số unit thật từ `generate_initial_cells`.
- Mức: Vừa; sai lệch preview/thực, và thiếu hàng rào ở backend.
- Đề xuất: một nguồn duy nhất cho phép đổi độ và đếm ô (dùng chung hàm trong `grid.rs`), `GridPlanner` gọi lại hàm đó. Backend kẹp hoặc từ chối `step_km` ngoài [0.2, 50] và NaN/Inf.
- Test hồi quy: `planner_estimated_cells_matches_generate_initial_cells`; assert `plan.estimated_cells == generate_initial_cells(...).len()` trên nhiều bbox (kể cả vĩ độ cao). Và `create_job_rejects_out_of_range_step`.
- Cần xác nhận: đọc đầu của `generate_initial_cells` (grid.rs:64-137) cho `step <= 0` và `NaN`; tôi đã xem là có kiểm tra giới hạn ô, nhưng chưa ghi lại chắc chắn nhánh `step <= 0`.

#### B9. Cờ Partial bật bởi lỗi enrichment không retry được (Vừa)
- Mô tả: `unresolved |= outcome.error.is_some()` coi mọi lỗi (cả lỗi vĩnh viễn như site không tồn tại, 403, DNS lỗi) là "chưa giải quyết", nên job kết thúc ở trạng thái Partial/cần retry dù không còn gì có thể retry.
- Bằng chứng: `google_maps_jobs/processor.rs:302` `unresolved |= outcome.error.is_some();`
- Repro: job có 1 website chết (DNS NXDOMAIN, `retryable=false`): job không bao giờ về `completed`.
- Mức: Vừa; người dùng thấy job "dở dang" vĩnh viễn và bấm retry vô ích.
- Đề xuất: chỉ `unresolved` khi `outcome.error` retryable (hoặc kết quả ở `retry_wait`); lỗi vĩnh viễn ghi `failed`/`not_found` và không tính.
- Test hồi quy: `finalize_job_completes_when_only_permanent_enrichment_errors`; assert trạng thái job là `completed`, không Partial.

#### B10. `upsert_result` ghi đè trạng thái bằng giá trị mới (Thấp-vừa)
- Mô tả: khi cùng `(job_id, dedup_key)` gặp lại ở unit khác, `ON CONFLICT DO UPDATE` COALESCE các trường dữ liệu (tốt, không ghi đè bằng NULL) nhưng đặt thẳng `name`, `detail_status`, `enrichment_status`, `import_status`, `last_error_*`, `source_unit_id` theo giá trị mới. Hệ quả:
  - kết quả đã `imported`/`completed`/`found` có thể bị đặt lại về `pending` (nếu candidate mới mang `pending`), dẫn tới import/enrich lần hai (trùng hoặc phí) hoặc mất trạng thái "đã nhập";
  - `name` mới rỗng hay là chuỗi kém tốt hơn (ví dụ danh sách cho tên rút gọn) sẽ đè tên tốt.
- Bằng chứng: `repository.rs:1475-1479` `detail_status=excluded.detail_status, enrichment_status=excluded.enrichment_status, import_status=excluded.import_status, last_error_kind=..., last_error_message=...`; `repository.rs:1470-1471` `name=excluded.name`.
- Repro: kết quả A đã `import_status='imported'`; một unit lưới khác khám phá lại A với candidate `import_status='pending'`: sau upsert A quay về `pending`. Importer sau đó đánh dấu `duplicate` (do đã có lead), làm lệch bộ đếm `leads_created`/`duplicates`.
- Mức: Thấp-vừa; tần suất cao với lưới chồng lấn nhưng hậu quả chủ yếu là số liệu và phí xử lý.
- Đề xuất: giữ trạng thái "tiến xa hơn": `import_status=CASE WHEN old IN ('imported','duplicate','rejected') THEN old ELSE new END`, tương tự cho `enrichment_status` và `detail_status`; `name=CASE WHEN length(excluded.name) >= length(old) THEN excluded.name ELSE old END`.
- Test hồi quy: `upsert_does_not_reset_imported_status`; assert `import_status` vẫn `imported` sau upsert lần hai.

#### B11. Chuẩn hoá SĐT VN thiếu trường hợp (Thấp)
- Mô tả: `normalize_vn_mobile` chỉ giữ số có đúng 10 chữ số và tiền tố 03/05/07/08/09. Hệ quả:
  - Số bàn VN (02x) và 1800/1900 bị bỏ (đúng nếu mục tiêu chỉ là di động, nhưng cần xác nhận với yêu cầu: lead nhà hàng/khách sạn nhiều số bàn).
  - Số đã đổi đầu số mới hoặc ghi `+84 (0) 912...`: sau lọc chữ số thành `840912...`, tiền tố `84` bị cắt thành `00912...`, độ dài 11 nên bị loại (mất lead hợp lệ).
  - Chuỗi có nhiều số (ví dụ "0912 345 678 / 0987 654 321") bị nối chữ số thành 20 số và bị loại.
  - Chữ cái/ghi chú (ví dụ "ext 12") chen vào cũng nối chữ số.
- Bằng chứng: `google_maps_jobs/importer.rs:25-28` lọc chữ số toàn chuỗi; `importer.rs:29-35` cắt `0084`/`84`; `importer.rs:37-44` điều kiện độ dài 10 và tiền tố.
- Repro (số giả): `+84 (0) 912 000 000` (11 chữ số sau cắt) trả `None`; `0912 000 000 / 0987 000 000` trả `None`.
- Mức: Thấp; mất một ít lead.
- Đề xuất: tách chuỗi theo `/`, `,`, `;`, "hoặc", thử từng phần; bỏ `(0)` sau mã quốc gia trước khi lọc; nếu cần, lưu số bàn riêng thay vì loại.
- Test hồi quy: `normalize_vn_mobile_handles_plus84_with_parenthesized_zero_and_multiple_numbers`; assert trả số di động đầu tiên hợp lệ.

#### B12. Import chạy sau commit checkpoint (Thấp)
- Mô tả: `checkpoint_completed_unit` commit trước, rồi mới gọi `import_ready_results`. Nếu import lỗi, hàm trả `Err`, runner coi checkpoint thất bại dù unit đã `completed` và kết quả đã lưu. Không mất dữ liệu (kết quả ở `pending` sẽ được import ở lần sau, `retry_failed_units` cũng gọi lại import), nhưng bộ đếm và trạng thái job có thể lệch.
- Bằng chứng: `repository.rs:718` `tx.commit()?;` rồi `repository.rs:719-722` import với `?`.
- Mức: Thấp. Đề xuất: lỗi import chỉ log và đánh dấu để thử lại ở finalize, không trả Err cho checkpoint.
- Test hồi quy: `checkpoint_succeeds_even_if_import_fails`.

#### B13. SSRF: thiếu NAT64/6to4 trong blocklist IP (Thấp)
- Mô tả: blocklist chặn loopback, private, link-local, v.v. nhưng không chặn `64:ff9b::/96` (NAT64) và `2002::/16` (6to4) có thể nhúng IPv4 nội bộ. DNS pinning và redirect cùng domain giảm rủi ro, nên chỉ khả thi khi kẻ tấn công kiểm soát AAAA của site.
- Bằng chứng: `google_maps_jobs/email.rs:642-680`.
- Mức: Thấp. Đề xuất: giải nén IPv4 nhúng từ NAT64/6to4/IPv4-mapped và áp lại blocklist IPv4.
- Test hồi quy: `blocks_nat64_and_6to4_embedding_private_v4`; assert `64:ff9b::7f00:1` và `2002:c0a8:0101::` bị chặn.

#### B14. `depth as u8` cắt giá trị (Thấp)
- `repository.rs:1341` `depth: row.get::<_, i64>(6)? as u8`. Với `CHECK (depth >= 0)` nhưng không có chặn trên, giá trị > 255 từ DB bị bọc vòng. Thực tế `max_depth = 6` nên không xảy ra trong luồng bình thường; chỉ cần với DB bị sửa tay. Đề xuất `u8::try_from(...)` trả lỗi. Test: `unit_from_row_rejects_out_of_range_depth`.

#### B15. `claim_next_unit` không có index khớp (Thấp)
- `repository.rs:589-598` `WHERE job_id=?1 AND (status='pending' OR (status='retry_scheduled' AND ...)) ORDER BY created_at ASC, id ASC LIMIT 1`. Index hiện có `(job_id, status, next_retry_at)` giúp lọc nhưng không phục vụ `ORDER BY`, nên SQLite phải sort tạm tập khớp. Với job lưới hàng chục nghìn unit chưa chạy, mỗi lần claim sort O(n). Đề xuất index `(job_id, status, created_at, id)` hoặc `ORDER BY` theo một cột có thứ tự trong index. Test: `EXPLAIN QUERY PLAN` không có `USE TEMP B-TREE FOR ORDER BY`.

---

### 3. Chưa chắc (cần xác nhận)

- B-U1. Địa chỉ có thể mang ký tự icon (glyph Private Use U+E0C8) ở đầu chuỗi vì selector địa chỉ ở `scrapers/google_maps.rs:812-813` lấy text của nút có thể gồm icon. Cần: một mẫu `address` thật. Việc đọc DB trong `~/.local/share/com.rustsale.crm*` bị cấm nên không kiểm. Cách xác nhận: lấy fixture DOM của panel địa điểm và assert `address` không chứa ký tự trong khoảng U+E000-U+F8FF; đề xuất thêm bước lọc ký tự PUA trong `text()`.
- B-U2. Viewport so với span zoom: `radius_to_zoom` giả định khung nhìn phủ đủ ô. Nếu cửa sổ trình duyệt nhỏ hơn dự kiến thì span thực nhỏ hơn bán kính ô và ô bị thiếu phủ. Cần: kích thước viewport trong `scrapers/browser.rs` và sidecar (grep `viewport|window-size` không ra kết quả trong browser.rs; cần xem sidecar `sidecars/cloak-runtime`).
- B-U3. `version` trong runner sau import: `import_results` tăng `version+1` (importer.rs:262-283). Runner giữ `job.version` cũ sau checkpoint có thể làm lần transition sau ném lỗi xung đột phiên bản. Cần đọc `runner.rs` để biết nó nạp lại job sau `checkpoint_completed_unit` (dòng cuối `load_job` ở repository.rs:723 có vẻ làm việc đó).
- B-U4. Hiệu lực của `GridScraper` kiểu cũ (`scrapers/grid.rs:296-396`): nếu không còn đường dùng trong production thì mã chết nên xoá để tránh lệch với `generate_initial_cells`.
- Hai tài liệu sổ cái (`2026-07-26-...production-ledger.md`, `2026-07-28-...failed-job-recovery.md`) tôi chỉ đọc phần đầu của tài liệu thứ hai; các lỗi nêu ở trên được kiểm lại trực tiếp trong code nên vẫn còn tồn tại ở thời điểm đọc.

---

### 4. Các điểm tốt đã xác nhận (không cần sửa)

- `claim_next_unit`, `checkpoint_*` dùng transaction `Immediate` và `WHERE status='running'` nên không có race claim/complete đôi.
- Bộ đếm job cập nhật bằng `SET x = x + ?` trong transaction (không read-modify-write ngoài DB).
- Dedup địa điểm theo `!1s` feature id / tọa độ `!3d!4d` / URL bỏ viewport, kèm `UNIQUE (job_id, dedup_key)`.
- SafeHttpClient: DNS pinning, redirect thủ công chỉ trong cùng registrable domain, giới hạn 2 MB, robots.txt.
- Không thấy `unwrap/expect` trong đường chạy thật của repository/importer/email (chỉ `expect("valid email regex")` ở email.rs:841 trên regex hằng số, an toàn vì được test; vẫn nên đổi sang `?` ở nơi khởi tạo nếu muốn tuân thủ quy tắc không panic).
- Importer dùng `ORDER BY id` (importer.rs:98), xử lý theo lô trong một transaction.

---

### 5. Cải tiến đáng làm

1. Một nguồn duy nhất cho hình học lưới (đổi độ-km, đếm ô, clamp step) dùng chung cho `GridPlanner`, `validation.rs` và `generate_initial_cells` (B8). Lý do: preview đáng tin, tránh job quá lớn bất ngờ.
2. Bỏ fallback website, thu hẹp regex email, thêm blocklist domain/đuôi file (B1, B2). Lý do: chất lượng lead là giá trị chính; một email rác gây bounce và hại uy tín gửi.
3. Cột chuẩn hoá `phone_normalized` (và index) trên `leads`, bỏ nạp toàn bộ phone (B6). Lý do: thời gian giữ khoá ghi không tăng theo số lead.
4. Lease cho enrichment (`claimed_at`) thay cho reset thủ công (B3). Lý do: tự phục hồi sau crash mà không cần mã đặc biệt ở recover.
5. Enrichment song song có giới hạn (ví dụ 4-8 đồng thời, theo host) thay vì tuần tự trong `finalize_job` (processor.rs:263-324). Lý do: tuần tự mỗi site tới `MAX_HTML_PAGES` trang, job lớn mất hàng giờ.
6. Tách hai pha quyết định "có nên chia ô" khỏi zoom (B7) và bật `coverage_truncated` khi bị chặn. Lý do: coverage minh bạch, người dùng biết vùng nào chưa phủ.
7. Giảm độ trễ cố định 8-15 giây mỗi địa điểm (`scrapers/google_maps.rs:98-99, 242`) thành ngẫu nhiên có điều chỉnh theo phản hồi (backoff khi bị chậm/chặn, nhanh hơn khi ổn). Lý do: là điểm nghẽn tốc độ lớn nhất; cần cân nhắc rủi ro bị chặn.
8. Ghi `source_url`/`source_job_id` vào lead khi import để truy vết và chống trùng xuyên job (hiện chỉ trùng theo phone/email). Lý do: gom nhiều job cho cùng khu vực không tạo lead trùng và dễ xoá hàng loạt khi sai.
9. Thêm CHECK hoặc bảng tham chiếu cho `status`/`import_status`/`enrichment_status` hoặc enum Rust có `try_from` nghiêm ngặt. Lý do: chặn giá trị lạ làm kẹt máy trạng thái.
10. Test tích hợp với DB tạm cho các luồng B3, B6, B10, B15 (`EXPLAIN QUERY PLAN`). Lý do: các lỗi này chỉ lộ ở quy mô và sau crash, test đơn vị thường không bắt được.

---

# Phần C. Trình duyệt, selector, sidecar, giao diện


Phạm vi: chỉ đọc code (không sửa, không chạy app, không mở Google Maps thật, không ghi vào `~/.local/share/com.rustsale.crm*`).
Repo: `/home/arch/Project/test/RustSale_v2`. Không có tên/SĐT/email khách trong báo cáo này.

Đã đọc đầy đủ: `src-tauri/src/scrapers/{browser.rs,google_maps.rs}`, `sidecars/cloak-runtime/src/{protocol,index,googleMaps,runtime}.ts`, các component Svelte `GoogleMapsScraper / JobPanel / JobHistory / RuntimePanel`, `googleMapsScraperService.ts`, và các phần liên quan của `google_maps_jobs/{processor,runner,commands,importer}.rs`. Đã đối chiếu ledger `2026-07-26-google-maps-scraper-production*.md`, `2026-07-29-google-maps-realtime-counters.md`, `2026-07-28-google-maps-failed-job-recovery.md` để không lặp lại lỗi đã sửa (ví dụ: thứ tự captcha > feed > text, `blank_page` retry, `!3d/!4d` ưu tiên hơn `@lat,lng`, identity `!1s`, đếm realtime, retry failed-unit).

Quy ước: "đã xác minh" = đọc lại đúng dòng code; "chưa chắc" = suy luận từ code, cần trang Google Maps thật hoặc môi trường thật để chốt.

### Bảng tóm tắt

| Mã | Tiêu đề | Mức | File:dòng chính |
|----|---------|-----|-----------------|
| C1 | Sidecar bị SIGKILL, Chromium mồ côi giữ khóa profile; `launch` không dọn tiến trình cũ | cao | browser.rs:354-372, 200; runtime.ts:274-276, 324 |
| C2 | Mất `div[role=feed]` sau khi "ready" bị coi là "đã hết danh sách" → unit rỗng hoàn tất im lặng | vừa | google_maps.rs:777-782, 344-349; googleMaps.ts:55, 117 |
| C3 | Một dòng stdout không phải JSON làm hỏng request đang chờ và nuốt response thật | vừa | browser.rs:761-771 |
| C4 | Fallback website/facebook/zalo lấy link ngoài bất kỳ trong cả trang | vừa | google_maps.rs:819-826 |
| C5 | Fallback điện thoại quét mọi `button,a,div,span` (sai số điện thoại + chậm) | vừa | google_maps.rs:799-811 |
| C6 | Địa chỉ/website dùng `textContent` + class bị làm rối `.rogA2c` | vừa (chưa chắc) | google_maps.rs:812-815 |
| C7 | Cửa sổ chờ sẵn sàng 10s rồi `selector_changed` không retry | vừa | google_maps.rs:100-101, 279-300 |
| C8 | Phát hiện "hết danh sách" sai khi mạng chậm (3×800ms ổn định) | vừa | google_maps.rs:80, 342-349 |
| C9 | Timer `evaluation_timeout` không bị hủy → sidecar không thoát, bị SIGKILL | vừa | runtime.ts:378-381, 555-559; index.ts:95 |
| C10 | `h1` bất kỳ được coi là "ready" (trang lỗi 5xx / shell) | vừa (chưa chắc) | googleMaps.ts:55, 117 |
| C11 | UI bỏ event của job không phải active → danh sách lịch sử/đếm lệch, job kẹt "running" | vừa | GoogleMapsScraper.svelte:404 |
| C12 | UI: bấm "Start" nhiều lần tạo job trùng | vừa | GoogleMapsScraper.svelte:775, 316-378 |
| C13 | UI: rò listener khi unmount trước khi `subscribe` xong | thấp | GoogleMapsScraper.svelte:450-460, 481-483 |
| C14 | UI: "Resume" khi vẫn bị chặn báo thành công giả; banner luôn ghi "CAPTCHA" | thấp | GoogleMapsScraper.svelte:380-395; JobPanel.svelte:121-132 |
| C15 | UI: `logs` tăng không giới hạn, mỗi lần append copy mảng (O(n²)) | thấp | GoogleMapsScraper.svelte:214-217 |
| C16 | `browser_for` bỏ `previous` rồi mới `close()?`; `expect` ở đường production | thấp | processor.rs:56-59, 72-74; browser.rs:249 |
| C17 | `killProcessTree` khớp theo chuỗi con của cmdline | thấp | runtime.ts:482-484 |
| C18 | Query `?hl=en` bị cắt khi canonical hóa URL place → ngôn ngữ lẫn lộn | thấp (chưa chắc) | google_maps.rs:600-614, 146, 152 |
| C19 | Selector consent theo `aria-label` đứng trước kiểm tra "ready" | thấp (chưa chắc) | googleMaps.ts:50, 118-122 |
| C20 | `normalize_vn_mobile` loại cả chuỗi chứa 2 số / số có hậu tố | thấp (chưa chắc) | importer.rs:24-45 |
| C21 | `BrowserManager::new(_max_sessions)` bỏ qua tham số; `acquire` chờ vô hạn | thấp | browser.rs:582-590, 704-708 |

---

### Chi tiết lỗi

#### C1 — Sidecar bị SIGKILL, Chromium mồ côi giữ khóa profile; `launch` không dọn tiến trình cũ
Mức: **cao** — làm job không resume được (profile bị khóa), rò RAM/tiến trình zombie, và `browser_startup` là lỗi retryable nên runner lặp lại vô ích rồi fail.

Mô tả. Có ba yếu tố cộng hưởng:
1. Timeout shutdown chỉ 3 giây (`DEFAULT_SHUTDOWN_TIMEOUT`, browser.rs:21) áp cho cả request `shutdown` lẫn chờ tiến trình thoát. Sidecar phải `context.close()` + `browser.close()` (Chromium ghi profile) rồi mới trả lời; 3s dễ không đủ với headed/profile lớn.
2. Hết hạn thì `self.process.kill()` (SIGKILL) sidecar giữa chừng, không cho nó chạy `killProcessTree`. Ngoài ra `Command::kill_on_drop(true)` (browser.rs:200) cũng SIGKILL khi `BrowserSupervisor` bị drop.
3. Chromium trên Linux/macOS được spawn `detached: !isWindows` (runtime.ts:324, tức process group riêng) nên không chết theo sidecar. Transport mặc định trên Linux là Playwright pipe, `browserProcess` = null, chỉ dọn được khi sidecar còn sống để quét `/proc`.
Hơn nữa `launch()` mở đầu bằng `await this.shutdown()` khi `this.profileDir` vẫn là `null` (runtime.ts:275-276 — thứ tự: shutdown rồi mới gán profileDir), nên một Chromium mồ côi của lần chạy trước trên cùng profile không bao giờ bị dọn trước khi launch.

Bằng chứng:
- browser.rs:354 `.request("shutdown", json!({}), self.timeouts.shutdown)` và :358 `tokio::time::timeout(self.timeouts.shutdown, self.process.wait())`; :368 `self.process.kill().await`.
- browser.rs:200 `.kill_on_drop(true);`
- runtime.ts:275-276 `await this.shutdown();` rồi `this.profileDir = payload.profileDir;`
- runtime.ts:324 `detached: !isWindows,`

Tái hiện (mô phỏng, không cần Maps thật): chạy sidecar với `RUSTSALE_BROWSER_TRANSPORT=cdp-websocket` trỏ vào một "chromium giả" ngủ 10s khi nhận SIGTERM (hoặc fake `CloakRuntime.shutdown` chờ 5s); gọi `BrowserSupervisor::shutdown`; sau 3s sidecar bị SIGKILL, tiến trình giả vẫn sống (`pgrep` còn). Gọi `launch` lại cùng `profileDir` → lỗi khởi động (hành vi chính xác của Chromium với `SingletonLock` là "chưa chắc", nhưng việc không dọn là chắc chắn).

Đề xuất sửa:
- Tách timeout: request `shutdown` 20-30s, chờ thoát sau khi đóng stdin 5s; trước SIGKILL sidecar thì gửi SIGTERM trước (index.ts đã có handler) và chờ.
- Trong `launch()`: gán `profileDir` trước rồi gọi một bước "dọn mồ côi theo profile" (`killProcessTree(undefined, profileDir)` + xóa `SingletonLock/SingletonCookie/SingletonSocket` nếu không còn tiến trình sống).
- Rust: khi phải SIGKILL sidecar, chủ động diệt process group Chromium theo profile (hoặc ghi pid Chromium vào file trong profile để dọn lần sau).

Test hồi quy:
- `kill_after_shutdown_timeout_does_not_leave_browser_orphan` (browser_test.rs, fake sidecar script): assert tiến trình con giả không còn sau `shutdown()` kể cả khi sidecar chậm hơn 3s.
- `launch_cleans_stale_profile_processes_before_start` (sidecar `index.test.ts`): spawn tiến trình có `--user-data-dir=<tmp>` giả, gọi `launch` với `profileDir` đó, assert tiến trình bị dọn trước khi launch.

#### C2 — Mất `div[role=feed]` sau khi "ready" bị coi là "đã hết danh sách"
Mức: **vừa** — mất dữ liệu im lặng: unit "keyword/grid" kết thúc với 0 kết quả và `reached_end = true`, không retry, không subdivide.

Mô tả. Script cuộn khi không tìm thấy feed rơi vào nhánh dự phòng: `window.scrollBy`, `is_end: false`, `height` = chiều cao body. Chiều cao ổn định 3 lần liên tiếp nên `scroll_feed` đặt `reached_end = true` (google_maps.rs:344-349). `place_count` lúc đó là 0, nên `collect_search_listing` trả `SearchListing { reached_end: true, place_urls: [] }`, tức một danh sách rỗng "hợp lệ". Trang không có feed nhưng vẫn được xếp "ready" xảy ra khi `hasPlaceHeading` (bất kỳ `h1`) đúng — xem C10.

Bằng chứng:
- google_maps.rs:777-782 `window.scrollBy(0, 1500); return { height: ..., is_end: false, place_count: document.querySelectorAll('a[href*="/maps/place/"]').length };`
- google_maps.rs:344-347 `if stable_observations >= STABLE_FEED_OBSERVATIONS { reached_end = true;`
- googleMaps.ts:55 `if (snapshot.hasFeed || snapshot.hasPlaceHeading) { return 'ready';`

Tái hiện: mock server (đã có mock-server test ở google_maps.rs:1448+) trả trang `/maps/search/...` chỉ có `<h1>Error</h1>` hoặc layout mới không có `role="feed"`; gọi `collect_search_listing`; kết quả `reached_end == true`, `place_urls.is_empty()`, không lỗi.

Đề xuất sửa: trong `scroll_feed`, nếu script báo `feed_missing: true` thì trả `SelectorChanged` (không retry mù) hoặc retry-có-reload; chỉ chấp nhận danh sách rỗng khi trạng thái `empty_results` được xác nhận. Thêm trường `has_feed` vào `ScrollInfo`.

Test hồi quy: `missing_feed_after_ready_is_selector_changed_not_exhausted` — mock trả `<h1>x</h1>` không feed; assert `Err(SelectorChanged)` (hoặc ít nhất `reached_end == false`).

#### C3 — Một dòng stdout không phải JSON làm hỏng request đang chờ và nuốt response thật
Mức: **vừa** — lỗi `InvalidResponse` không retryable làm hỏng unit đúng lúc, dù sidecar vẫn khỏe.

Mô tả. `response_loop` gặp dòng không parse được JSON thì gọi `fail_all_pending(InvalidResponse, retryable=false)` rồi `continue`. Request đang chờ bị xóa khỏi `pending`; response thật đến sau bị `deliver_response` bỏ qua (không có sender). Sidecar dùng `console.log` ở nhiều nơi: wrapper CloakBrowser/Bun/Playwright có thể in cảnh báo ra stdout; chỉ `launch`/`ensureRuntime` được bọc `diagnosticsToStderr` (runtime.ts:532-540), còn `navigate`/`evaluate` thì không. Sidecar không tự khóa stdout: index.ts:93 ghi response bằng `process.stdout.write`, nhưng bất kỳ `console.log` nào của thư viện cũng đi chung kênh.

Bằng chứng:
- browser.rs:761-770 `Err(error) => { fail_all_pending(&pending, scrape_error(ScrapeErrorKind::InvalidResponse, format!("invalid CloakBrowser response: {error}"), false)).await; }`
- browser.rs:796-797 `let sender = pending.lock().await.remove(&response.id); if let Some(sender) = sender {` (response muộn bị bỏ).

Tái hiện: fake sidecar in `warn: something\n` giữa lúc xử lý một `evaluate`, sau đó in response hợp lệ. Request `evaluate` trả `InvalidResponse`, response đúng bị bỏ.

Đề xuất sửa: Rust bỏ qua (ghi log stderr) dòng không phải JSON hợp lệ, chỉ fail pending khi EOF/IO error; sidecar đầu `index.ts` gán `console.log = console.error` toàn cục và giữ bản `process.stdout.write` riêng cho giao thức.

Test hồi quy: `stray_stdout_line_does_not_fail_inflight_request` (browser_test.rs, fake sidecar): assert request vẫn nhận response đúng.

#### C4 — Fallback website/facebook/zalo lấy link bất kỳ trong trang
Mức: **vừa** — dữ liệu sai đi vào lead (website của bên thứ ba, link chia sẻ, link đặt món).

Mô tả. Khi doanh nghiệp không có nút `authority`, vòng lặp lấy `href` đầu tiên bắt đầu bằng `http` mà không chứa `google.`, `gstatic.com`, `facebook.com`, `zalo.me`. Trang place chứa nhiều link ngoài (đặt hàng, đặt bàn, "Quảng cáo", liên kết tin tức/đánh giá). `facebook`/`zalo` dùng `href.includes('facebook.com')` trên toàn trang nên cũng nhận cả link tới trang Facebook khác (ví dụ link trong đánh giá), và `includes('zalo.me')` khớp cả tên miền chứa chuỗi đó.

Bằng chứng:
- google_maps.rs:818-825 `for (const link of Array.from(document.querySelectorAll('a[href]'))) { ... if (!facebook && href.includes('facebook.com')) facebook = href;`
- google_maps.rs:822-823 `if (!website && href.startsWith('http') && !href.includes('google.') && ...`

Tái hiện: fixture HTML của một place không có `data-item-id="authority"` nhưng có link `https://order.example-delivery.vn/x` → `website` = link đó.

Đề xuất sửa: chỉ tin `[data-item-id="authority"]`; facebook/zalo giới hạn trong `[role="main"]`, parse bằng `new URL(href).hostname` (`endsWith('.facebook.com')`/`facebook.com`), bỏ `sharer`, và bỏ link nằm trong `[data-review-id]`/vùng đánh giá. Gỡ nhánh "lấy link ngoài đầu tiên".

Test hồi quy: `place_without_authority_has_no_website` — fixture có link ngoài; assert `website == None`.

#### C5 — Fallback điện thoại quét mọi `button,a,div,span`
Mức: **vừa** — số điện thoại có thể thuộc địa điểm khác (mục "tìm kiếm liên quan", đánh giá, quảng cáo), và chi phí lớn.

Mô tả. Khi không có `data-item-id^="phone:tel:"` / `a[href^="tel:"]`, script duyệt mọi `button, a, div, span` và lấy phần tử đầu tiên có `textContent` (sau bỏ khoảng trắng/dấu chấm/gạch) khớp `^(?:\+84|0)\d{9,10}$`. Số đầu tiên theo thứ tự DOM không đảm bảo thuộc place hiện tại. Mỗi `div` gọi `textContent` toàn bộ cây con nên độ phức tạp gần O(N·kích thước trang). Regex cũng bỏ sót số tổng đài 1900/1800 (`1900 xxxx`), số quốc tế khác `+84`.

Bằng chứng:
- google_maps.rs:801-807 `const elements = Array.from(document.querySelectorAll('button, a, div, span')); ... if (/^(?:\+84|0)\d{9,10}$/.test(cleaned)) { phone = candidate;`

Tái hiện: fixture place không có số nhưng có một ô "gợi ý" chứa số khác → `phone` = số đó. Đo thời gian trên DOM ~5.000 node.

Đề xuất sửa: giới hạn tìm trong `[role="main"]` và chỉ các nút có `data-item-id`/`aria-label` bắt đầu bằng "Phone"/"Điện thoại"; bỏ quét `div`. Nếu muốn giữ fallback, dùng `aria-label` thay vì `textContent`.

Test hồi quy: `phone_fallback_ignores_numbers_outside_info_panel`.

#### C6 — Địa chỉ/website dùng `textContent` + class bị làm rối `.rogA2c` (chưa chắc)
Mức: **vừa (chưa chắc)** — dữ liệu bẩn, hoặc mất địa chỉ khi Google đổi class.

Mô tả. `.rogA2c` là class bị làm rối (đổi khi Google deploy). Các nút `data-item-id` của Maps thường chứa icon dạng glyph vùng private-use (`aria-hidden`), và `textContent` lấy luôn glyph đó, nên chuỗi có thể bắt đầu bằng ký tự `U+E0xx`. Điều này chỉ xác nhận được trên trang thật nên đánh dấu chưa chắc. `clean_optional` (google_maps.rs:742-747) chỉ `trim()`, không loại ký tự này; `candidate_matches_requested_area` fallback so khớp token địa chỉ nên glyph không phá token (token là chữ+số) nhưng làm bẩn dữ liệu lưu.

Bằng chứng: google_maps.rs:812-813 `text(document.querySelector('[data-item-id="address"]')) || text(document.querySelector('.rogA2c'))`; google_maps.rs:742-747 `clean_optional`.

Đề xuất sửa: ưu tiên `aria-label` của nút ("Address: ..."/"Địa chỉ: ..." rồi bỏ tiền tố cả hai ngôn ngữ), và lọc `/[-]/g` trong `clean_optional`; bỏ `.rogA2c`.

Test hồi quy: `clean_optional_strips_private_use_glyphs` và `address_prefers_aria_label_without_icon_glyph`.

#### C7 — Cửa sổ chờ sẵn sàng 10s rồi `selector_changed` không retry
Mức: **vừa** — mạng/proxy chậm làm unit fail không retry (non-retryable) và người dùng phải bấm "Retry Failed Units".

Mô tả. `ready_attempts: 20`, `ready_delay_ms: 500` → tối đa 10 giây. Trang không rỗng nhưng chưa có feed/h1 (shell "Loading", dialog) lọt vào `SelectorChanged` cuối cùng với `retryable = false`, trong khi trang rỗng thì được retry (TransientNetwork). Đã sửa cho `blank_page` (ledger) nhưng không cho trạng thái "chưa render xong".

Bằng chứng:
- google_maps.rs:100-101 `ready_attempts: 20, ready_delay_ms: 500,` (đọc từ khối `new()` :95-104)
- google_maps.rs:294-299 `_ => scrape_error(ScrapeErrorKind::SelectorChanged, "Google Maps page did not expose a known result or place selector", false)`

Tái hiện: mock server chậm render feed 12 giây → lỗi `SelectorChanged` không retry.

Đề xuất sửa: lần cạn đầu tiên trả lỗi retryable (reload + chờ lâu hơn/gấp đôi); chỉ khi cạn lần hai với DOM không rỗng mới `SelectorChanged`. Cho cấu hình timeout theo `navigation_timeout`.

Test hồi quy: `slow_render_is_retried_before_selector_changed` — assert lần cạn đầu `retryable == true`.

#### C8 — Phát hiện "hết danh sách" sai khi mạng chậm
Mức: **vừa** — `reached_end = true` giả làm `should_subdivide` trả `false` (grid.rs:189 `if reached_end && raw_listing_count < saturation_threshold { return false; }`) nên mất kết quả mà không có dấu hiệu.

Mô tả. Điều kiện kết thúc là `(height, place_count)` không đổi 3 lần liên tiếp, mỗi lần cách 800ms (`scroll_delay_ms`) tức 2,4 giây. Khi Google trả trang kế tiếp chậm hơn (proxy, mạng yếu, CPU bận), vòng lặp kết luận đã hết dù feed còn nhiều. Test hiện có `stable_feed_without_end_sentinel_reports_reached_end` (google_maps.rs:1142) củng cố hành vi này, nên là quyết định có chủ đích để tránh subdivide thừa; điểm yếu nằm ở ngưỡng thời gian cố định.

Bằng chứng: google_maps.rs:80 `const STABLE_FEED_OBSERVATIONS: u32 = 3;`; google_maps.rs:102-103 `max_scroll_attempts: 30, scroll_delay_ms: 800`; google_maps.rs:342-349.

Đề xuất sửa: kiểm tra spinner/`[role="progressbar"]` hoặc nút "đang tải" trong feed (không dùng class); tăng số lần ổn định thích ứng theo thời gian phản hồi (adaptive backoff: nếu lần cuộn trước mất >N ms thì nhân thời gian chờ). Ghi log `reached_end_reason` (sentinel/stable/cap) để dễ gỡ lỗi.

Test hồi quy: `slow_feed_load_is_not_reported_as_end` — mock thêm kết quả sau 3s; assert `reached_end == false` hoặc có đủ kết quả.

#### C9 — Timer `evaluation_timeout` không bị hủy → sidecar không thoát kịp, bị SIGKILL
Mức: **vừa** — gần như mỗi lần tắt graceful sau khi vừa `evaluate` đều thua cuộc đua 3s và dẫn tới SIGKILL (khuếch đại C1).

Mô tả. `Promise.race([page.evaluate(...), timeout(30_000)])` luôn để lại một `setTimeout` 30 giây chưa hủy. Khi stdin đóng, vòng `for await` kết thúc, `await shutdown()` xong, nhưng index.ts không gọi `process.exit`; Node/Bun còn timer nên tiến trình sống tối đa 30s. Rust chỉ chờ 3s rồi SIGKILL. Mỗi lần gọi cũng để lại đóng (closure + promise) cho đến khi timer nổ.

Bằng chứng:
- runtime.ts:378-381 `await Promise.race([ page.evaluate(...), timeout(payload.timeoutMs ?? 30_000, 'evaluation_timeout'), ]);`
- runtime.ts:555-559 `function timeout(...) { return new Promise((_, reject) => { setTimeout(() => reject(...), timeoutMs); }); }`
- index.ts:95 `await shutdown();` (không có `process.exit`).

Tái hiện: unit test sidecar: gọi `evaluate` nhanh rồi đo `process.getActiveResourcesInfo()` (hoặc `await` kết thúc `runSidecar` trong test) — còn `Timeout`.

Đề xuất sửa: giữ handle và `clearTimeout` trong `finally`; hoặc `timer.unref()`; thêm `process.exit(0)` sau shutdown cuối ở `import.meta.main`.

Test hồi quy: `evaluate_clears_timeout_timer` — assert không còn timer sau khi evaluate resolve (dùng fake timers/`jest.getTimerCount`-tương đương của bun).

#### C10 — `h1` bất kỳ được coi là "ready" (chưa chắc)
Mức: **vừa (chưa chắc)** — trang lỗi/shell có `<h1>` vượt qua kiểm tra, trích xuất cho ra "tên" rác hoặc danh sách rỗng giả (xem C2).

Mô tả. `hasPlaceHeading = Boolean(document.querySelector('h1, [role="main"] h1'))`: selector `h1` đã bao gồm vế thứ hai nên thực tế là "có `h1` bất kỳ". Trang lỗi 5xx của Google, trang tham chiếu, hoặc `h1` ẩn (sr-only "Results") đều thỏa. Trạng thái `ready` dựa trên `hasFeed || hasPlaceHeading` (googleMaps.ts:55) nên bỏ qua phần kiểm tra text `rate_limited`/`empty_results` ở dưới. `extract_current_place` lấy `h1` đầu tiên làm tên (google_maps.rs:796). Việc trang Maps hiện tại có `h1` ẩn ở trang tìm kiếm hay không cần kiểm chứng trên trang thật nên đánh dấu chưa chắc.

Bằng chứng: googleMaps.ts:117 `hasPlaceHeading: Boolean(document.querySelector('h1, [role="main"] h1'))`; googleMaps.ts:55.

Đề xuất sửa: với place dùng `[role="main"] h1` cộng điều kiện có ít nhất một `[data-item-id]` hoặc nút hành động; với listing yêu cầu feed. Tách hai chế độ kiểm tra `expect: 'listing' | 'place'` truyền từ Rust.

Test hồi quy: `classify_error_page_with_h1_is_not_ready` (index.test.ts): snapshot `hasPlaceHeading: true` nhưng body "Error Server Error" → không phải `ready`.

#### C11 — UI bỏ event của job không phải active
Mức: **vừa** — danh sách lịch sử hiển thị trạng thái/đếm cũ vĩnh viễn; chọn lại job đó dùng object cũ nên UI báo "running" khi job đã xong, không còn event nào để cập nhật.

Mô tả. `handleJobEvent` thoát ngay nếu `event.job_id !== activeJob.id`. Khi người dùng chọn job B trong lúc job A đang chạy, mọi event của A bị bỏ (kể cả terminal). Khi chọn lại A, `selectJob(job)` lấy object từ `jobs` (cũ), `viewStateFromJob` cho trạng thái cũ, và sẽ không có event mới (job đã xong). Chỉ reload trang/restart mới đồng bộ lại. Ngoài ra `isScraping` suy ra từ `activeJob` nên khi đang xem B (xong) form được mở khóa trong khi A vẫn chạy; tạo job mới sẽ lỗi "already active" ở bước resume.

Bằng chứng: GoogleMapsScraper.svelte:404 `if (!activeJob || event.job_id !== activeJob.id) return;`; :432-435 `selectJob`; :205 `let isScraping = $derived(activeJob !== null && isOpenStatus(activeJob.status));`.

Tái hiện (component test): tạo hai job, chọn B, bắn event `completed` cho A, chọn lại A → UI vẫn "running".

Đề xuất sửa: luôn cập nhật `jobs` theo event (giữ guard sequence/terminal theo từng job), chỉ cập nhật `activeView` khi job đang chọn; `isScraping` nên dựa trên "có job mở bất kỳ" (`jobs.some(open)`).

Test hồi quy: `events for non-selected job update the job list` — assert `jobs[A].status === 'completed'` sau event.

#### C12 — Bấm "Start" nhiều lần tạo job trùng
Mức: **vừa** — tạo nhiều job (profile + units) trong khi chỉ một job chạy được; job thừa kẹt `queued`.

Mô tả. Nút Start chỉ `disabled={!keyword.trim()}`. `isScraping` chỉ bật sau `syncJob(created)`, nên có khoảng thời gian (chuẩn bị runtime có thể kéo dài tới 15 phút; tạo job) trong đó có thể bấm nhiều lần. Bước đầu `startScrape` không có cờ đang gửi; `runtimePreparing` cũng không khóa nút Start. Lần resume thứ hai gặp "already active" nhưng job đã được tạo.

Bằng chứng: GoogleMapsScraper.svelte:775 `<Button size="sm" onclick={startScrape} disabled={!keyword.trim()} ...>`; :316-378 `startScrape` không có guard.

Đề xuất sửa: thêm `let starting = $state(false)` guard đầu hàm và `disabled={!keyword.trim() || starting || runtimePreparing || isScraping}`.

Test hồi quy: `double click start creates one job` — click hai lần, assert `createGoogleMapsScrapeJob` gọi đúng một lần.

#### C13 — Rò listener khi unmount trước khi `subscribe` xong
Mức: **thấp**.

Mô tả. `onMount(async ...)` await `getGoogleMapsRuntimeStatus()` trước, rồi mới `eventCleanup = await subscribeToGoogleMapsJobEvents(...)`. Nếu component bị hủy trước khi promise này resolve thì `onDestroy` gọi `eventCleanup?.()` khi còn `null`; sau đó gán `eventCleanup` nhưng không ai gọi → `listen` vẫn sống, callback tiếp tục chạy trên component đã hủy (gọi `appendLog`, sửa state). Mỗi lần điều hướng nhanh vào/ra tab rò thêm một listener.

Bằng chứng: GoogleMapsScraper.svelte:450-460 và :481-483 `onDestroy(() => { eventCleanup?.(); });`.

Đề xuất sửa: cờ `destroyed`; sau `await subscribe...` nếu `destroyed` thì gọi `unlisten()` ngay; hoặc `$effect` với hàm cleanup trả về, và subscribe trước các `await` khác.

Test hồi quy: `unmount before subscribe resolves still unlistens` — mock `listen` trả promise trễ, unmount, resolve, assert `unlisten` được gọi.

#### C14 — "Resume" khi vẫn bị chặn báo thành công giả; banner luôn ghi "CAPTCHA"
Mức: **thấp** — gây hiểu nhầm, không mất dữ liệu.

Mô tả. `confirm_action` (runner.rs:176-198) trả về job không đổi khi trang vẫn chưa `Ready/EmptyResults`. UI `runJobAction` hiển thị `updated.last_error_message ?? 'Resuming completed.'`, nên khi không có `last_error_message` người dùng thấy "completed" dù job vẫn `paused_requires_action`. Banner (JobPanel.svelte:121-132) cố định chữ "Complete the CAPTCHA…" kể cả khi nguyên nhân là consent hay rate limit (UI có `action_required` nhưng không phân biệt loại).

Bằng chứng: GoogleMapsScraper.svelte:389 `syncJob(updated, updated.last_error_message ?? \`${label} completed.\`);`; GoogleMapsJobPanel.svelte:124 `Complete the CAPTCHA in the opened CloakBrowser window`.

Đề xuất sửa: so sánh `updated.status` với trạng thái trước; nếu vẫn `paused_requires_action` hiển thị "Trang vẫn đang bị chặn, hãy hoàn tất bước kiểm tra"; banner theo `last_error_kind` (captcha/consent/rate_limited).

Test hồi quy: `resume while still blocked shows still-blocked message`.

#### C15 — `logs` tăng không giới hạn, append O(n²)
Mức: **thấp** — job lớn (hàng nghìn unit, mỗi event một dòng) làm UI nặng dần.

Bằng chứng: GoogleMapsScraper.svelte:215-217 `logs = [...logs, \`[${timestamp}] ${msg}\`];` (copy toàn bộ mảng mỗi lần; không cắt).

Đề xuất sửa: giữ tối đa N (ví dụ 500) dòng cuối (`logs = [...logs.slice(-499), line]`); gộp event cùng trạng thái.

Test hồi quy: `log buffer is capped` — bắn 1.000 event, assert `logs.length <= 500`.

#### C16 — `browser_for` bỏ `previous` rồi mới `close()?`; `expect` ở đường production
Mức: **thấp** — vi phạm quy tắc "không `expect` ở production"; lỗi đóng browser cũ chặn job mới.

Mô tả. `active.take()` lấy `previous` ra khỏi `active` trước khi `previous.manager.close().await?`. Nếu `close` lỗi, lỗi lan ra làm job mới fail với lỗi của job cũ, còn `previous` bị drop ngay (kill_on_drop SIGKILL sidecar → có thể mồ côi Chromium, xem C1). Có thêm `expect` cho invariant và `lock().expect(...)`.

Bằng chứng: processor.rs:56-59 `if let Some(previous) = active.take() { previous.manager.close().await?; }`; processor.rs:72-74 `.expect("active browser initialized")`; browser.rs:249 `.lock().expect("stderr task lock")`.

Đề xuất sửa: log lỗi `close` của browser cũ và vẫn tiếp tục; thay `expect` bằng cấu trúc `match`/`get_or_insert_with`; xử lý `PoisonError` bằng `unwrap_or_else(|e| e.into_inner())`.

Test hồi quy: `previous_browser_close_failure_does_not_block_next_job`.

#### C17 — `killProcessTree` khớp theo chuỗi con của cmdline
Mức: **thấp**.

Mô tả. Linux dọn theo `cmdline.includes(userDataDir)`. Với transport pipe (mặc định Linux) đây là cách duy nhất tìm Chromium. Bất kỳ tiến trình nào có đường dẫn profile trong argv (shell, editor, trình xem log) đều bị SIGKILL; ngược lại nếu `userDataDir` được truyền tương đối hoặc có symlink thì không khớp. ID job `gmaps-<ms>-<16hex>` độ dài cố định nên không có trùng tiền tố giữa các job.

Bằng chứng: runtime.ts:482-484 `if (cmdline.includes(userDataDir)) { pidsToKill.add(pid);`

Đề xuất sửa: khớp chính xác `--user-data-dir=<path>` và chỉ chấp nhận tiến trình có exe nằm trong thư mục cache CloakBrowser; chuẩn hóa `realpath`.

Test hồi quy: `kill_process_tree_ignores_unrelated_process_with_path_in_argv`.

#### C18 — `?hl=en` bị cắt khi canonical hóa URL place (chưa chắc)
Mức: **thấp (chưa chắc)** — trang chi tiết place có thể trả ngôn ngữ khác trang tìm kiếm (dữ liệu địa chỉ lẫn tiếng Anh/Việt).

Mô tả. Search URL ép `?hl=en` (google_maps.rs:146, 152) nhưng `canonicalize_place_url` cắt mọi thứ sau `?` (google_maps.rs:605-609), do đó URL place được mở lại không có `hl`; Google chọn ngôn ngữ theo Accept-Language/IP. Phân loại trạng thái đã có từ khóa cả vi/en nên không hỏng, còn trích xuất dùng `data-item-id` (không phụ thuộc ngôn ngữ), nhưng văn bản địa chỉ/tên danh mục có thể lẫn ngôn ngữ giữa các job và giữa các unit.

Đề xuất sửa: khi `navigate` tới place, thêm lại `hl=` theo cấu hình thống nhất (và cân nhắc `gl=vn`).

Test hồi quy: `place_navigation_url_keeps_hl_param`.

#### C19 — Selector consent theo `aria-label` đứng trước kiểm tra "ready" (chưa chắc)
Mức: **thấp (chưa chắc)** — có thể tạm dừng job nhầm.

Mô tả. `hasConsentDialog` khớp `[aria-label*="consent" i]`, `[aria-label*="Đồng ý" i]`, `[aria-label*="Chấp nhận" i]` trên toàn trang; kiểm tra consent đứng trước `hasFeed` (googleMaps.ts:50-52), nên một nút có nhãn chứa các chuỗi này trong trang bình thường sẽ chuyển job sang `paused_requires_action` dù trang đã có feed. Tương tự, `body.includes('recaptcha')` trên 20.000 ký tự body (googleMaps.ts:41, 115) có thể trúng nội dung kết quả. Comment trong code nói rõ captcha có chủ đích đứng trước feed; rủi ro chỉ ở độ rộng của selector/text.

Đề xuất sửa: giới hạn selector vào `form[action*="consent"]`/dialog `[role="dialog"]`; với text thì chỉ xét khi không có feed hoặc dùng cụm đầy đủ.

Test hồi quy: `classify_feed_with_harmless_aria_label_is_ready`.

#### C20 — `normalize_vn_mobile` loại cả chuỗi chứa 2 số / số có hậu tố (chưa chắc)
Mức: **thấp (chưa chắc)** — lead không tạo dù có số hợp lệ.

Mô tả. Hàm lọc mọi chữ số trong chuỗi rồi so độ dài đúng 10. Một chuỗi chứa hai số ("... / ...") hoặc có "ext 12" sẽ có >10 chữ số → `None`. Số dạng `+84 (0) 9...` thành `00...` (11 ký tự) → `None`. Số bàn (02x) luôn bị loại theo thiết kế (`qualifies_for_lead` chỉ chấp nhận di động).

Bằng chứng: importer.rs:25-30 `.filter(char::is_ascii_digit)`; importer.rs:36-42 điều kiện `len() == 10`.

Đề xuất sửa: tách chuỗi theo `/ , ; | -`/khoảng trắng nhiều, thử từng ứng viên; bỏ `(0)`.

Test hồi quy: `normalize_vn_mobile_accepts_first_valid_of_multiple_numbers`.

#### C21 — `BrowserManager::new(_max_sessions)` bỏ qua tham số; `acquire` chờ vô hạn
Mức: **thấp** — hiện chỉ 1 job chạy mỗi lần nên chưa phát sinh, nhưng là bẫy.

Bằng chứng: browser.rs:582 `pub fn new(_max_sessions: usize)`, :590 `Semaphore::new(1)`; :704-708 `.acquire_owned().await` không timeout. Profile mặc định là thư mục tạm cố định `rustsale-cloakbrowser-profile` (browser.rs:585) dùng chung giữa các instance nếu quên `with_profile_dir`.

Đề xuất sửa: dùng `max_sessions` thật, `acquire` có timeout; bắt buộc truyền profile.

Test hồi quy: `acquire_times_out_when_session_leaked`.

---

### Đã kiểm tra, không phải lỗi (để tránh báo trùng)
- Tính toán số kiểu VN ("4,5", "1.234"): scraper hiện không trích rating/reviews/giờ mở cửa/hạng mục nên chưa có parse lỗi. Nếu v3 thêm, cần bộ parse dùng chung (xem mục cải tiến).
- "Đang mở cửa" không được dùng ở đâu trong code Rust/TS (không có trích xuất giờ).
- Đóng stderr: đã có task đọc stderr (browser.rs:242-249) nên không treo pipe; stdout được đọc liên tục.
- Request id: tăng dần theo `request_sequence`, đóng khung bằng `\n`; `request_serial` serial hóa request; `pending` được dọn khi timeout/ghi lỗi. Chỉ vấn đề ở C3.
- Thứ tự captcha > feed > consent > text đã đúng và có comment giải thích; `!3d/!4d` ưu tiên hơn `@lat,lng`; hạn mức 120 kết quả vẫn dẫn tới subdivide qua `raw_listing_count >= saturation_threshold` (grid.rs:189).
- Đường chạy chính không dùng `unwrap/expect` ngoài vị trí nêu ở C16 (các `unwrap` còn lại nằm trong test).

---

### Cải tiến đáng làm (kèm căn cứ)

Tốc độ
1. Trì hoãn cố định 8-15 giây trước mỗi place (`min_delay_ms: 8000, max_delay_ms: 15000`, google_maps.rs:98-99; gọi `self.delay()` ở :242) khiến 1.000 place tốn gần 3,2 giờ chỉ riêng độ trễ. Thay bằng chờ theo sự kiện (đợi `[data-item-id]` ổn định, rồi jitter ngắn 1-3s) và giữ jitter dài chỉ giữa các tìm kiếm. Căn cứ: ledger cho thấy chặn do captcha tập trung ở khối lượng tìm kiếm, không phải thời gian chờ trang place; cần đo trước khi đổi.
2. Lấy sơ bộ từ thẻ trong feed (tên, link, địa chỉ ngắn, SĐT nếu có) để bỏ qua chuyến vào trang place cho những ô đã đủ dữ liệu. Căn cứ: feed đã được cuộn và `COLLECT_PLACE_URLS_SCRIPT` chỉ lấy `href`, bỏ phí dữ liệu thẻ.
3. Mở 2-3 tab song song trong cùng profile cho các place (cần `max_sessions` thật, xem C21) — rủi ro bị phát hiện cao hơn nên đặt làm tùy chọn.
4. Dùng `script` dạng hàm gửi qua `page.evaluate(fn)` thay vì `globalThis.eval(expression)` (runtime.ts:378-379) để không phụ thuộc CSP/Trusted Types; có thể đẩy cả bước quét trong một lần gọi để giảm số vòng stdin/stdout.

Độ bền
5. Dọn tiến trình mồ côi theo profile ngay khi khởi động ứng dụng và trong `launch` (C1); ghi file pid trong thư mục profile.
6. Phân loại lỗi theo hướng "thử lại có reload" trước khi báo `selector_changed` (C7), và lưu `reached_end_reason` vào diagnostics.
7. Lưu snapshot DOM/ảnh chụp khi `selector_changed` (đã có `capture_scrape_diagnostics`) kèm phiên bản Chromium/hl để so khớp khi Google đổi giao diện; thêm một "canary" kiểm tra selector định kỳ trên fixture đã lưu.
8. Chặn mọi `console.log` trong sidecar sang stderr và coi stdout là kênh giao thức duy nhất (C3).
9. Khóa/ủy quyền shutdown: đóng stdin → SIGTERM → chờ → SIGKILL theo tầng, với timeout phù hợp cho việc ghi profile.
10. Phản ứng chặn: hiện chỉ có "dừng và chờ người dùng" (headed relaunch trên cùng profile). Cân nhắc backoff tự động khi `rate_limited` (tăng delay, nghỉ 5-15 phút) trước khi bắt người dùng can thiệp, và đếm số lần chặn trên job để tự giảm tốc.
11. Không có cấu hình proxy/locale/timezone/geolocation trong `LaunchPayload` (protocol.ts:42-46: chỉ `profileDir`, `headless`, `startUrl`). Với lead Việt Nam nên có tùy chọn proxy VN và locale `vi-VN`/múi giờ `Asia/Ho_Chi_Minh` để giảm captcha và để kết quả ổn định.
12. Profile riêng cho mỗi job (commands.rs tạo `profile_root/<job_id>/profile`): job mới luôn bắt đầu "lạnh" (consent/captcha lại, tốn dung lượng). Cân nhắc profile dùng chung đã "ấm" có khóa và dọn cache định kỳ.

Chất lượng dữ liệu
13. Chuẩn hóa tại một chỗ: loại glyph private-use, chuẩn hóa Unicode NFC cho tên/địa chỉ (tiếng Việt có dấu tổ hợp khác nhau gây trùng giả khi so khớp), chuẩn hóa SĐT sang E.164 ngay khi trích xuất.
14. Bổ sung trích xuất mà người dùng thường cần: hạng mục (category), đánh giá/số lượt (viết parser tách `4,5`/`4.5` và `1.234`/`1,234` theo ngữ cảnh), giờ mở cửa, plus code, trạng thái "đóng cửa vĩnh viễn" để loại lead chết. Cần hằng số theo hai ngôn ngữ ("Đang mở cửa", "Đóng cửa vĩnh viễn", "Permanently closed").
15. Dedup chéo job: hiện identity (`place_identity`) chỉ trong phạm vi lập kế hoạch/kết quả của job; xem importer để loại trùng với lead đã có theo SĐT chuẩn hóa (đã có `normalized_phone`).

UI
16. Hiển thị lý do chặn, số lần thử lại, thời gian thử lại tiếp theo, và nút "Mở trình duyệt để kiểm tra" riêng; thông báo toast khi job kết thúc/partial/failed.
17. Chọn lọc nhật ký theo mức, giới hạn kích thước, và nút xuất log (C15).

---

### DANH SÁCH TÍNH NĂNG (để v3 đạt chức năng tương đương)

Tính năng người dùng thao tác được (UI `GoogleMapsScraper.svelte` + service):

1. Runtime CloakBrowser
   - Xem trạng thái runtime: đã cài hay chưa, phiên bản wrapper/Chromium (`get_google_maps_runtime_status`).
   - Cài/chuẩn bị runtime (`prepare_google_maps_runtime`; sidecar `ensure_runtime`, timeout cài tới 15 phút); tự cài khi bấm Start nếu chưa có.
   - Hiển thị lỗi runtime trong panel và nhật ký.
2. Tạo job (`create_google_maps_scrape_job`) với các tham số: `mode` (keyword / grid), `keyword`, `location`, `bbox` [lat_min, lat_max, lng_min, lng_max], `step_km`, `districts[]`, `headless`.
   - Chế độ Keyword: từ khóa + vị trí tự do; "Deep scan" (mặc định bật) quét theo quốc gia (mặc định Vietnam) và danh sách khu vực hành chính được chọn (`canonical_path`).
   - Chế độ Grid: nhập tay bbox (min/max lat/lng) hoặc chọn khu vực hành chính (giải bbox theo `geoname_id`), "Quick select" tỉnh/thành có sẵn (`get_preset_provinces`), geocode theo tên khu vực, thanh trượt bước lưới 0,5-10 km, ước tính số ô, chặn nếu >10.000 ô và gợi ý bước tối thiểu.
   - Validate: keyword không rỗng, tối đa 200 ký tự; ô lưới hợp lệ.
   - Tùy chọn chạy ẩn (headless) hoặc có cửa sổ.
3. Vòng đời job: `start/resume`, `pause`, `cancel`, `retry failed units`, `retry email enrichment`, xóa job (chỉ khi partial/completed/failed/cancelled), tự phục hồi job gián đoạn khi mở app (`recover_startup`, trạng thái `interrupted`), `paused_limit_reached`, `paused_requires_action`.
4. Giám sát: tiến độ % (processed/total units), bộ đếm Discovered / Qualified / Leads / Failed units cập nhật thời gian thực qua sự kiện `google-maps-scrape-job-event` (có `sequence`, bỏ event cũ và sau terminal), lỗi gần nhất (kind + message), URL bị chặn, nhật ký dòng lệnh trong UI, danh sách lịch sử job và chọn job để xem.
5. Xử lý chặn: phát hiện captcha/consent/rate_limited/blank/selector_changed/empty_results; khi cần người dùng thì mở lại trình duyệt có cửa sổ trên cùng profile tại URL bị chặn, người dùng giải xong bấm "I completed browser check — Resume"; hệ thống kiểm tra lại sức khỏe trang rồi chạy tiếp.
6. Hồi phục: retry theo backoff cho lỗi tạm thời, retry thủ công cho unit lỗi (cả job `partial` lẫn `failed`), nhập lead từ kết quả đã sẵn trước khi retry/khởi động lại, chụp chẩn đoán (diagnostics) khi lỗi.
7. Lập kế hoạch quét: các loại unit (keyword / district / grid / place), tự chia ô lưới (`should_subdivide`) khi ô bão hòa, giới hạn độ sâu/số ô, phát hiện `coverage_truncated`, dedup place theo identity (`!1s`, `!3d/!4d`).
8. Trích xuất mỗi place: tên, điện thoại, địa chỉ, website, facebook, zalo, tọa độ, place id, canonical URL; lọc theo vùng yêu cầu (bbox hoặc khớp địa chỉ theo token).
9. Nhập lead: tạo lead vào CRM khi có số di động VN hợp lệ hoặc email hợp lệ (`qualifies_for_lead`, `normalize_vn_mobile`, `normalize_email`), đếm `businesses_qualified`/`leads_created`; làm giàu email (`email.rs`, nút "Retry Email Enrichment").
10. Hạ tầng: profile trình duyệt riêng theo job (lưu bền), nhật ký có cấu trúc (target `google_maps_scraper`, thư mục `logs`), ghi nhận sự kiện job (`log_scrape_event`).
11. Lệnh kế thừa còn đăng ký: `start_google_maps_scrape` / `stop_google_maps_scrape` (src-tauri/src/lib.rs:530-531) — chưa xác minh UI còn dùng không (chưa chắc); v3 nên quyết định bỏ hay giữ.

Không có trong scraper UI (cần hỏi chủ dự án có muốn v3 bổ sung): xuất CSV/Excel ngay trong scraper, nhập danh sách keyword/URL hàng loạt, lên lịch chạy, cấu hình proxy, chỉnh delay/timeout từ UI.
