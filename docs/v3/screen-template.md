# Port một màn Svelte sang TSX (NSH 0.6.1)

Mẫu thật để copy: `app/screens/TemplatesScreen.tsx` (danh sách + form + dialog + xác nhận xóa),
`app/screens/LeadsScreen.tsx` (bảng lớn, lọc/sắp xếp ở Rust), `app/screens/ConversationScreen.tsx` (danh sách ảo).
Test mẫu: `app/App.test.tsx`, `app/Templates.test.tsx`.

## 1. Quy tắc đổi từ Svelte 5 sang NSH TSX

| Svelte 5 | NSH TSX |
|---|---|
| `let x = $state(0)` | `const x = signal(0)`; đọc `x()`, ghi `x.set(v)` |
| `$derived(a * 2)` | `computed(() => a() * 2)` |
| `$effect(() => …)` | `effect(() => …)` (chỉ cho tác dụng phụ; ưu tiên computed) |
| `$props()` | tham số `props` của hàm component |
| `{@render children()}` | `props.children` |
| `{#if}` / `{#each}` | `<If when={() => …}>` / `<Each each={…} keyExtractor={…}>`; danh sách dài dùng `List` |
| `class="…"` Tailwind | `class="…"` cùng cú pháp, chỉ dùng token (`bg-card`, `text-muted-foreground`); cấm mã màu thô |
| `onclick` | `onPress` |
| `bind:value` | `value` + `onChange`/`onValue` |
| `fetch`/`invoke` Tauri | gọi module Rust `rustsale/native` (xem mục 4) |

Điểm hay sai:

1. **Component chạy một lần.** Không có re-render. Mọi thứ đổi theo thời gian phải là signal và phải được **đọc ngay trong biểu thức JSX** (`{pick(route())}`), không đọc trong thân component rồi dùng `if`.
2. **Không có `numberOfLines`**; dùng class `truncate`.
3. `nsh/base` (Button, Input, Dialog, Tabs) **không có style**. Luôn dùng bản bọc trong `app/ui` (Button, Input, Badge, Modal, States).
4. Không dùng thư viện React/hook. Không `useState`.
5. Danh sách trên khoảng 100 dòng dùng `List` (tái chế view), không dùng `Each`.

## 2. Design system (app/ui)

`Button` (primary/secondary/ghost/destructive, sm/md), `Input`, `Badge` (neutral/success/warning/danger), `Modal`,
`Skeleton`, `EmptyState`, `ErrorState`. Màu chỉ đến từ `app/theme.json` (token v2 dạng oklch, sáng + tối).
Cần token mới: thêm vào `theme.json` cả `light` và `dark`, rồi dùng class `bg-<token>`.

## 3. Mỗi màn production phải có

- Trạng thái **đang tải** (Skeleton), **rỗng** (EmptyState có gợi ý), **lỗi** (ErrorState có “Thử lại”).
- `testID` cho mọi phần tử mà test cần bấm hoặc đọc.
- Form: kiểm tra đầu vào trước khi lưu, hiện lỗi ngay dưới form (`tpl-error`), Enter gửi form, Hủy đóng mà không lưu.
- Xóa: luôn qua Modal xác nhận, nút `destructive`.
- Chạy được ở cả sáng và tối.

## 4. Gọi dữ liệu

Tác vụ nặng hoặc dữ liệu lớn nằm ở Rust (`native/`), TSX chỉ nhận cửa sổ đang hiển thị:

```ts
const rows = await store.window(0, 200);   // async: không chặn UI
```

Hàm Rust **đồng bộ** chạy trong lời gọi JS và chặn UI; việc nặng phải là `async fn` + `blocking(...)`.
Không `unwrap`/`expect`; lỗi đi qua `thiserror` (`native/src/error.rs`). Lữ Bố **không** có bản DB thật; dùng DB v3-dev do Nakroth cấp hoặc dữ liệu giả trong store.

## 5. Quy trình port một màn

1. Đọc màn Svelte, liệt kê: dữ liệu vào/ra, hành động, trạng thái (tải/rỗng/lỗi), dialog.
2. Tạo `app/stores/<ten>.ts` (signal + hàm thao tác). Dữ liệu thật đi qua module Rust, không gọi DB từ TSX.
3. Tạo `app/screens/<Ten>Screen.tsx` theo `TemplatesScreen.tsx`.
4. Thêm vào `pick()` trong `app/Shell.tsx`.
5. Viết test headless `app/<Ten>.test.tsx` (mở màn, bấm, nhập, kiểm tra chữ trên màn hình).
6. Chạy: `nsh check`, `nsh test`; nếu đụng Rust: `cargo clippy --all-targets -- -D warnings` và `cargo test` trong `native/`.
7. Chụp ảnh thật: `(cd bench && nsh test --target gpui screens.test.tsx)` (xem `bench/screens.test.tsx`).
   Test headless qua chưa có nghĩa là giao diện đẹp; phải nhìn ảnh.

## 6. Giới hạn đã biết trên máy này

- Mục tiêu GTK (`nsh run` mặc định) cần `webkitgtk-6.0`, chưa cài; dùng `--target gpui`.
- Test headless mặc định có viewport nhỏ: gọi `app.setViewport(1280, 800)` đầu mỗi test, nếu không chữ ở vùng nội dung bị cắt và `texts()` thiếu.
- `app.type` không chạy trên `--target gpui` (“no native view”); gõ chữ thì test headless.
- X server của máy này chậm (phần mềm); số fps/cuộn trên gpui ở đây không đại diện.
