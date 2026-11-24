# RS v3: perf spike (P0c)

Ngày đo: 2026-11-22. Máy: Linux (CachyOS), X server phần mềm không có compositor, không có GPU thật.
Dữ liệu: bản sao chỉ-đọc `~/.local/share/com.rustsale.crm.v3-dev/rustsale.db` (342 MB; 4.607 khách; hội thoại dài nhất 31.983 tin).
Không mở thư mục dữ liệu thật, không đăng nhập Zalo/WhatsApp.

## 1. Phiên bản NSH: 0.6.1

- 0.6.1 là bản ổn định mới nhất của 0.6.x, trùng tag của skill `nsh`/`nsh-design-guides` đã nạp (`.agents/skills`, thay bằng bản v0.6.1).
- `nsh-core` và `nsh-base` 0.6.1 có trên crates.io; build mixed (TSX + crate Rust `#[native_module]`) chạy thật.
- CLI đang cài cũ hơn (0.4) không có đủ lệnh mixed; đã dùng CLI cập nhật để chạy `check`, `test`, `run`.
- Framework còn ở 0.x và đổi nhanh: khóa phiên bản trong `native/Cargo.toml` và đọc https://nsh.dev/docs trước khi nâng.

## 2. Số đo v3 (đo thật)

| Mục tiêu | Ngưỡng | v3 đo được | Kết luận |
|---|---|---|---|
| Lọc/sắp xếp bảng khách (4.6k dòng) | < 100 ms | Rust: **0,2 đến 5,6 ms** (release và hiển thị trên caption “lọc X ms”); từ lúc bấm tới khi đổi chữ trên màn hình, headless: **~115 ms** (gồm chu kỳ chờ 100 ms của vòng thăm dò test, nên chặn trên) | Đạt phần xử lý. Chưa chứng minh được end-to-end dưới 100 ms trên màn hình thật |
| Cuộn hội thoại 31.983 tin, 60 fps, không dòng trắng | 60 fps | Commit mỗi khung khi cuộn (headless, **bản debug**): **~0,6 ms**, ổn định ~230 node, 31 đến 36 mount item; 0/8 khung trống. Lần nạp thêm tin cũ: 7,8 ms. Mở hội thoại: 194 ms headless | Phần app đạt với biên lớn. **Fps thật chưa đo được** (xem mục 4) |
| Khởi động nguội tới màn đầu dùng được | < 1 s | **3,1 s** (bản debug, gpui, gồm nạp 4.6k khách); tự nạp bất đồng bộ nên khung sườn hiện ngay, bảng hiện skeleton rồi dữ liệu | **Chưa đạt trên bản debug; bản release chưa đo chính xác** |
| Bộ nhớ khi rảnh | < 150 MB | Bản **release** gpui host, bảng khách đã nạp: **167 MB** (ổn định). Bản debug mở hội thoại: ~250 MB | **Trượt 17 MB** trên host này |
| Lời gọi chặn luồng UI | 0 | Nạp khách, mở và cuộn hội thoại đều là `async` + `blocking`; chỉ `query`/`window` trong bộ nhớ chạy đồng bộ, 0,2 đến 5,6 ms | Đạt (theo thiết kế; chưa có công cụ đếm tự động) |

Số Rust (cargo test --release, `native/src/perf.rs`, chạy ignored):

| Việc | Thời gian |
|---|---|
| Nạp 4.607 khách (một lần, ngoài luồng UI, phụ thuộc cache) | 115 đến 830 ms |
| `query` + `window` (lọc/sắp xếp) | 0,2 đến 2,6 ms |
| Đếm tin của hội thoại | ~4 ms |
| Cửa sổ 200 tin (SQL LIMIT/OFFSET) | 2,8 đến 6,5 ms |
| Tìm hội thoại dài nhất (GROUP BY) | 1 đến 2,7 s (chỉ cần cho spike; ngoài luồng UI; core thật sẽ có chỉ mục/sẵn id) |

Quyết định đã làm nhờ số đo: sắp xếp lần đầu 66 đến 98 ms do cấp phát khi so sánh, nay tính sẵn khóa chữ thường: còn 1 đến 2,5 ms. Mở 32k tin đồng bộ mất ~880 ms và chặn UI, nên chuyển thành cửa sổ bất đồng bộ.

## 3. Số đo v2 (bản release 2535767, identifier `com.rustsale.crm.v2bench`, DB v3-dev đã làm sạch, `env -u` các khóa AI, cổng 8798)

Cổng License/HWID hiện modal "Activation Required". Theo quyết định của Nakroth, không đăng nhập; và **cột cuộn/lọc của v2 bị bỏ**: cửa sổ v2 bị che bởi cửa sổ khác trên màn hình dùng chung, không có devtools trong bản release, và gõ phím/ấn chuột vào đó có nguy cơ chạm cửa sổ của chủ. Chỉ đo thứ không cần thao tác UI:

| Mục v2 | Giá trị |
|---|---|
| Cửa sổ xuất hiện (tiêu đề "RustSale CRM & Outreach Pro") | ~0,26 s sau khi chạy binary (mới là khung vỏ, **chưa phải** bảng có dữ liệu; không có mốc tương đương) |
| RSS rảnh sau ~60 s (có lớp phủ Activation, giao diện Svelte đã nạp) | tiến trình chính 125 MB + WebKitWebProcess 436 MB + WebKitNetworkProcess 66 MB = **~627 MB** (+30 MB swap) |

Thư mục `com.rustsale.crm.v2bench` và worktree `wt-v2-bench` đã xóa.

## 4. Số đo v3 bản release (gpui, `nsh run --target gpui --release --trace`, `env -u` các khóa AI, DB v3-dev, 4.639 khách, hội thoại dài nhất 31.983 tin)

Điều kiện: **máy tải 17 đến 40 trên 8 nhân** do build của các bot khác suốt lúc đo; không drop page cache (cần sudo); font Inter đã đóng gói.

| Mục | Kết quả |
|---|---|
| Cold start: từ lúc host khởi động tới khi bảng khách có dữ liệu, lần chạy đầu | 4.012 ms |
| Cold start, 5 lần chạy ấm kế tiếp (load ~17) | 1.521, 1.219, 1.303, 2.515, 1.374 ms: **median 1.374 ms** (chưa đạt < 1 s) |
| Chia nhỏ một lần chạy (load ~17) | host khởi động tới khi app chạy: 1.350 ms; tải 4.639 khách trong Rust: 173 ms; vẽ dữ liệu: 4 ms. Khung đầu 79 ms (eval 5,9 ms) |
| Thời gian CPU của host tới mốc đó (không phụ thuộc tải), 5 lần | 1.320, 840, 770, 640, 760 ms: **median 770 ms** |
| App TSX trống cùng host (cùng cách đo) | CPU 410 đến 490 ms. Tức phần app của ta ~330 ms CPU, phần host gpui ~440 ms CPU |
| Cuộn bảng khách (150 cú cuộn xdotool, 150 commit) | commit median 0,67 ms, p95 5,7 ms, max 208 ms; 3/150 commit (2%) > 16,7 ms: lần nạp thêm trang 200 dòng (21 ms và 208 ms, 179 ms trong `list js`) và một lần 14 ms. Nghi do GC/nối mảng khi tải trang tiếp, cần xử lý ở P1 |
| Cuộn hội thoại 31.983 tin (120 cú cuộn, 65 commit) | median 0,46 ms, p95 0,69 ms, max 0,82 ms; 0 commit > 16,7 ms |
| RAM đỉnh (VmHWM) | 151 đến 159 MB |
| RAM rảnh 2 phút, màn khách | RSS 7 MB + 49 MB đã bị hệ điều hành đẩy sang swap = **~55 MB** (RSS thô tụt vì máy đang thiếu RAM, swap 20/23 GB, nên dùng RSS+swap) |
| RAM rảnh 2 phút, màn hội thoại | cùng tiến trình sau khi mở hội thoại: RSS 10 MB + swap, tổng ước ~55 đến 80 MB; RSS ngay sau khi vẽ xong: 46 đến 77 MB |

So với v2: RAM rảnh ~55 đến 80 MB (v3) so với ~627 MB (v2), tức nhỏ hơn khoảng 8 lần.

Ảnh chụp lại bằng dữ liệu giả (demo seed) sẽ bổ sung sau; ảnh cũ chụp từ DB thật đã bị gỡ vì chứa tên và tin nhắn Zalo thật.

## 4b. Cột linux (GTK 4), `nsh package --target linux`, đo 22/11 09:00 đến 09:20

Build được sau khi cài `webkitgtk-6.0` 2.54.3 (chủ đồng ý 22/11). Lý do cần gói này: nsh-cli luôn build host Linux của app mixed (Rust + TSX) với feature `webview` của `nsh-platform-linux` (`HOST_ONLY_FEATURES` trong `rust_app.rs`), nên thiếu `javascriptcoregtk-6.0.pc` là lỗi build dù app không dùng WebView. Build release lần đầu 17 phút 44 giây; `.nsh/out/RustSale` 67 MB (tar.gz 22 MB). Cùng DB v3-dev 4.643 khách, `env -u` khóa AI, máy tải ~20 trên 8 nhân.

| Mục | gpui (mục 4) | linux GTK4 | Tỉ lệ |
|---|---|---|---|
| Cold start wall tới khi bảng có dữ liệu, 5 lần ấm (lần đầu bỏ) | median 1.374 ms | 1.935, 2.079, 2.266, 2.544, 2.553: **median 2.266 ms** (lần đầu 1.657) | 1,65x |
| CPU host tới mốc đó, median | 770 ms | 710 đến 780 ms, **median 750 ms** | ~1x |
| Cuộn bảng khách, commit (150 cú cuộn thật) | median 0,67 ms, p95 5,7 ms, max 208 ms | median 1,0 ms, **p95 16,2 ms**, max 136 ms; 4/71 commit > 16,7 ms | p95 2,8x |
| RAM rảnh sau 2 phút, màn khách | ~55 MB (RSS+swap, lúc máy thiếu RAM) | RSS 154 MB + swap 16 MB = **~170 MB**; Pss 140 MB (file-backed 106 MB: thư viện GTK/WebKit dùng chung) | ~3x |
| RAM đỉnh VmHWM | 151 đến 159 MB | 169 MB | ~1,1x |

Ảnh GTK4 chụp lại bằng dữ liệu giả sẽ bổ sung sau (ảnh cũ đã gỡ vì chứa dữ liệu thật).

Đọc số:
- Cold start theo CPU không khác gpui; chênh wall (1,65x) đến từ tải máy, chưa vượt ngưỡng 2x.
- **Rủi ro cần lưu ý (vượt 2x):** p95 commit khi cuộn là 16,2 ms so với 5,7 ms của gpui (2,8x). Số commit chỉ là phần JS/layout; GTK vẽ bằng CPU nên chi phí vẽ cao hơn. Số `--fps` trong lúc cuộn: paint 15 đến 52 ms trung bình, frame p95 ~70 ms, fps 2 đến 8 (máy tải 20, nên chưa đủ để kết luận mượt hay không trên máy yên tĩnh). gpui không có `--fps` nên không so trực tiếp được.
- RAM rảnh GTK4 cao hơn gpui, nhưng 106 MB là thư viện hệ thống dùng chung; Pss_Anon chỉ 34 MB. So với v2 (~627 MB) vẫn nhỏ hơn ~3,7 lần.
- Việc nên làm: đo lại cuộn và fps trên máy yên tĩnh trước khi chốt GTK4 là bản phát hành Linux; nếu vẫn chậm thì Nakroth quyết có viết host GPUI hay không.

**Runtime có cần webkitgtk-6.0 không: CÓ (kiểm 22/11, bản `nsh package --target linux`, 68 MB).**
- `readelf -d .nsh/out/RustSale/RustSale | grep NEEDED` có `libwebkitgtk-6.0.so.4`, `libjavascriptcoregtk-6.0.so.1`, `libgtk-4.so.1`. `ldd` giải được cả ba về `/usr/lib/`. Đây là phụ thuộc liên kết động cứng (DT_NEEDED), không phải nạp lười: thiếu thư viện thì trình nạp từ chối chạy binary, dù app không mở WebView nào.
- Metadata gói không khai phụ thuộc: `nsh/app.json` chỉ có name/id/version/schemes, `.desktop` không có trường nào về gói; `RustSale.tar.gz` chỉ chứa binary + bundle, không kèm thư viện. Người dùng cuối phải tự cài `webkitgtk-6.0` (Arch), `libwebkitgtk-6.0-4` (Debian/Ubuntu 24.04+), `webkitgtk6.0` (Fedora), kèm GTK 4.14+. Đó là vài trăm MB phụ thuộc cho một app không dùng WebView.
- Nguyên nhân: nsh-cli luôn bật feature `webview` của `nsh-platform-linux` khi build host app mixed (`HOST_ONLY_FEATURES`, mục trên).
- Đề xuất (chưa làm, không vá nsh-cli): (1) gói `.deb`/`.rpm`/PKGBUILD tự ta viết cho bản phát hành, khai `Depends: libwebkitgtk-6.0-4, libgtk-4-1`; (2) hỏi upstream nsh (github.com/longbridge/nsh) cho tắt feature `webview` với app không dùng WebView; khi có, binary không còn kéo WebKit. Trong lúc chờ: chọn (1). Nakroth quyết có mở issue upstream không.

## 4c. Sửa khựng khi cuộn bảng khách (22/11 10:00 đến 10:13)

**Nguyên nhân:** bảng nạp trang 200 dòng bằng `rows.set([...rows(), ...trang])`; mỗi lần nạp thêm là một commit 14 đến 60 ms (đỉnh 208 ms lúc máy tải), lặp mỗi ~170 commit, đúng ranh giới trang (đo lại trên gpui, 1.500 cú cuộn: 9 commit > 16,7 ms, max 60 ms). Đổi `keyExtractor` sang so sánh theo identity không giúp.

**Sửa (`app/screens/LeadsScreen.tsx`):** `List` chỉ giữ mảng số thứ tự (token `thế hệ * 1.000.000 + chỉ số`, tạo một lần mỗi lần lọc/sắp xếp), không còn nối mảng và không còn `onLoadMore`. Nội dung dòng đọc từ Rust theo trang 64 dòng, khi dòng đó được gắn vào ô lần đầu, rồi lưu cache theo trang. Dữ liệu đã nằm trong bộ nhớ Rust (nạp DB chạy nền bằng `blocking()` từ trước), nên đọc trang là phép đọc bộ nhớ, giao diện không chờ DB. Token đổi mỗi thế hệ để lọc/sắp xếp mới buộc mọi dòng gắn lại.

| Commit khi cuộn 1.500 cú, tải máy 12 đến 20 | Trước (gpui) | Sau (gpui) | Sau (GTK4, cửa sổ 1200x800, tải 5) |
|---|---|---|---|
| median | 519 µs | 440 đến 500 µs | 612 µs |
| p95 | 2,5 ms | 0,95 đến 1,4 ms | 1,27 ms |
| p99 | ~8 ms | 2,6 đến 9,3 ms | 3,8 ms |
| commit > 16,7 ms | 9 | 1 (lặp lại đúng một chỗ, 140 đến 151 ms, `list js` 140 ms) | 7 (max 264 ms) |

Cuộn bảng p95 đạt mục tiêu (< 16,7 ms) trên cả hai host, và p95 GTK4 so với gpui nay ~1 đến 1,3x (trước đó 2,8x, nhưng số cũ chỉ có 71 commit, lúc tải 20; chưa so cùng điều kiện).

**Còn mở:**
- Còn commit lẻ rất chậm: gpui đúng một lần 140 đến 151 ms tại cùng vị trí cuộn trong cả hai lần chạy (`list js` 140 ms, 5 mount items; chưa tìm ra nguyên nhân, nghi GC của QuickJS hoặc một dòng cụ thể); GTK4 có 6 commit 75 đến 167 ms với `list js` ~0,1 ms (tức phần dựng của host, nghi do tải máy) và một commit 264 ms (`list js` 155 ms). p95 không bị ảnh hưởng nhưng người dùng sẽ thấy khựng một nhịp.
- Máy vẫn tải 5 đến 25 trong lúc đo, chưa phải điều kiện "load < 4" mà Nakroth yêu cầu để đo lại cột GTK4 chính thức.
- Ảnh GTK4 đã cuộn: sẽ chụp lại bằng dữ liệu giả (ảnh cũ đã gỡ vì chứa dữ liệu thật).

## 5. Giới hạn của phép đo (nói thẳng)

- Chỉ có chi phí commit phía app, không có fps thật (gpui không có `--fps`, GTK cần `webkitgtk-6.0`). xdotool ~130 ms mỗi cú cuộn, chậm hơn người dùng thật.
- Cold start đo qua dev host của `nsh run` (có kết nối websocket tới dev server). `nsh package` **không có target gpui** (chỉ linux GTK, macos AppKit, windows Win32, web), nên chưa có binary đóng gói gpui để đo. Số thật cần đo lại trên máy yên tĩnh và trên binary đóng gói.
- Wall time bị tải 17 đến 40 làm nhiễu rất mạnh (cùng app trống: 1,5 đến 9,6 s tùy lúc), nên đưa thêm thời gian CPU.
- Cuộn bảng khách v2 và cold start v2 đến bảng có dữ liệu: không đo (xem mục 3).
- Mốc `rs-marker` (console.log) trong `App.tsx` và `LeadsScreen.tsx` chỉ để đo, chỉ hiện khi chạy `--trace`.

## 6. Kết luận cho Nakroth

- Đạt: lọc/sắp xếp trong Rust 0,2 đến 5,6 ms; cuộn hội thoại và bảng không khung nào > 16,7 ms ở trạng thái ổn định; RAM rảnh ~55 đến 80 MB (đỉnh 151 đến 159 MB, sát ngưỡng 150).
- Chưa đạt/chưa chứng minh: cold start wall 1,37 s (load 17) so với mục tiêu 1 s, nhưng CPU 0,77 s và host trống đã tốn 0,44 s CPU; việc của app ~0,33 s. Cần máy yên tĩnh để kết luận. Còn vài giật khi nạp trang 200 dòng kế tiếp của bảng khách.
- Kiến trúc (Nakroth 22/11): phát hành bằng target native (`nsh package`: linux GTK4, macos, windows); gpui chỉ để chạy dev và so sánh. Chặn đo GTK4: thiếu webkitgtk-6.0 (mục 4b).

## 7. Chạy lại

```
cd bench
CARGO_TARGET_DIR=/home/arch/Project/test/.target-rustsale-v3 nsh test perf.test.tsx          # headless: sort, mở và cuộn hội thoại
CARGO_TARGET_DIR=... nsh test --target gpui screens.test.tsx                                # chụp ảnh (docs/v3/screens)
cd ../native && cargo test --release -- --include-ignored --nocapture                       # số Rust
```

## 4d. P1 screens on both hosts (22/11)

| Màn | gpui (dev, `nsh run --release`) | GTK4 (`nsh package --target linux`) | Test headless |
|---|---|---|---|
| Khách | mở, cuộn, p95 commit ~1 ms | mở, cuộn, p95 1,27 ms (4c) | App.test.tsx 4/4 |
| Hội thoại | mở (App.test) | mở, nạp 31.983 tin (bản sao v3-dev), cuộn được | điều hướng 1/1 |
| Google Maps Scraper | mở, job giả chạy và dừng | mở, job giả chạy tới "Hoàn tất" | Scraper.test.tsx 2/2 |

Ghi chú: Scraper dùng store giả (`app/stores/scraper.ts`) cho tới khi nối core của Toro. Ảnh: `docs/v3/screens/scraper-gpui-form.png`, `scraper-gtk4-release.png`. Ảnh Hội thoại có tin thật từ bản sao nên không đưa vào repo. Máy tải 25 khi chụp, chưa đo cold start lại.

## 4e. GPUI host build: gpui-fast pinned to 0.1.17 (#55, 24/11)

`nsh run --target gpui` stopped building: `nsh-platform-gpui` 0.6.1 asks for `gpui-fast ^0.1.16`, a fresh resolution
picks 0.1.19, and 0.1.19 defines its own `FillOptions` without lyon's `FillOptions::even_odd()` that
`nsh-platform-gpui` calls at `render.rs:466` (E0599). 0.1.17 still re-exports lyon's `FillOptions`.

Fix: `native/Cargo.toml` requires `gpui-fast` and `gpui-fast-platform` as `=0.1.17` under `[target.'cfg(any())'.dependencies]`.
nsh writes the GPUI host's own workspace and lockfile under `native/.nsh/rust/gpui` and resolves it from the app's
requirements (not from `native/Cargo.lock`); `cfg(any())` is false on every target, so the two crates are resolved but
never compiled or linked into the app. Checked: the host builds (9 min cold), connects and draws its first frame.

To drop the pin: delete that table once an `nsh-platform-gpui` release fixes the call for gpui-fast 0.1.19, then delete
`native/.nsh/rust/gpui/Cargo.lock` and run `nsh run --target gpui` again.
