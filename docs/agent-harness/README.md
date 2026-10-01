# Kiểm chứng cho AGY CLI trong RustSale_v2

## Cách dùng

### 1. Kiểm chứng ngay trong phiên AGY đang làm việc

Mở `agy` trong repo và giao task như bình thường. Rule
`.agents/rules/command-evidence.md` hướng dẫn agent chuẩn bị `.agents/task.json`,
sau khi sửa xong gọi lệnh Bash ngay trong phiên hiện tại:

```bash
bun --no-env-file run agent:verify <sessionId>
```

Đặt working directory của công cụ Bash là `/home/arch/Project/test/RustSale_v2`.
Nếu không chỉ định được thư mục cho công cụ, chuyển thư mục ngay trong lệnh:

```bash
cd /home/arch/Project/test/RustSale_v2 && bun --no-env-file run agent:verify <sessionId>
```

`<sessionId>` phải trùng trường `sessionId` trong task contract. Khi không có
driver cung cấp ID, dùng ID conversation nếu biết; nếu không, tạo ID riêng
cho task. ID tự tạo không được coi là ID conversation do AGY xác nhận.
Agent phải cập nhật tiêu chí cho task mới; contract của đợt cài harness không
thay thế tiêu chí nghiệm thu tính năng đang làm.

`agent:verify` chạy các lệnh kiểm tra và ghi evidence, không gọi model khác.
Nó nhận ID và đọc tiêu chí trong file JSON, không nhận đề bài ngôn ngữ tự nhiên.
Agent chờ exit code và kết quả rồi sửa lỗi, chạy lại và báo cáo đúng trạng thái.
Rule được khai báo Always On (`trigger: always_on`); với phiên đã mở trước khi thêm rule,
yêu cầu agent đọc `.agents/rules/command-evidence.md` hoặc mở phiên mới.

**Đây là quy trình do agent tuân thủ.** Rule không cưỡng chế agent gọi lệnh và
không chặn được mọi báo cáo sai. Việc thực thi verifier độc lập và tự động ghi log receipt.

---

### 2. Bộ điều phối kiểm chứng độc lập

Chạy từ terminal khi muốn chương trình điều phối cả task và kiểm chứng:

```bash
bun --no-env-file run agent:run "Mô tả yêu cầu sửa hoặc thêm tính năng" [model-id]
```

Driver dùng `gemini-3.8-flash-high` mặc định; có thể truyền model ID ở đối số thứ hai.
Driver gọi AGY ở chế độ headless, cung cấp session ID, tự chạy verify sau mỗi lượt,
và gửi lỗi trở lại cùng conversation (tối đa 3 lượt). Kết quả cuối được chương
trình tạo: `PASS` hoặc `BLOCKED`, cùng đường dẫn log tại `~/.local/state/rustsale-agent/`.
Phản hồi model được giữ trong log, không hiển thị thành tuyên bố hoàn thành chưa kiểm chứng.
Đây là lệnh dành cho **tác vụ lập trình từ terminal ngoài**, không dùng nó làm lệnh kiểm chứng
bên trong phiên AGY hiện tại (trong phiên hiện tại dùng `agent:verify`).

Với tác vụ chạy qua driver, agent phải:

1. Đọc yêu cầu, tạo `.agents/task.json` trước khi sửa code, dùng đúng `sessionId`.
2. Liệt kê đầy đủ yêu cầu và ánh xạ từng yêu cầu tới kiểm tra hành vi có thể chạy.
3. Sửa code; driver tự chạy `bun --no-env-file run agent:verify <sessionId>` sau phản hồi.
4. Đọc kết quả và log. Sửa lỗi trong phạm vi công việc rồi chạy lại nếu cần.
5. Chỉ báo PASS cho các kiểm tra được chứng minh; nêu rõ giới hạn kiểm chứng.

---

### 3. Cấu trúc Task Contract (`.agents/task.json`)

```json
{
  "sessionId": "ID_CUA_TASK_DANG_LAM",
  "title": "Tên tính năng hoặc bugfix",
  "requirements": [
    {
      "id": "R1",
      "description": "Mô tả yêu cầu cụ thể",
      "checks": ["ten-check-1"]
    }
  ],
  "checks": [
    {
      "id": "ten-check-1",
      "command": ["bun", "test", "path/to/test.ts"],
      "timeoutMs": 120000,
      "testFormat": "bun"
    },
    {
      "id": "rust-custom-test",
      "command": ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "ten_test"],
      "timeoutMs": 300000,
      "testFormat": "cargo"
    }
  ]
}
```

* `command`: mảng tham số bắt đầu bằng `bun`, `bunx` hoặc `cargo`.
* `testFormat`: `"bun"` hoặc `"cargo"`. Verifier sẽ phân tích stdout:
  - Yêu cầu số lượng test pass > 0.
  - Yêu cầu không có test bị skip/ignored (đối với cargo base check cho phép tối đa 1 test bỏ qua đã biết do yêu cầu credential live Zalo).

---

### 4. BASE_CHECKS mặc định

Mọi phiên verification đều tự động chạy toàn bộ các cổng chất lượng nền tảng:
1. `rust-clippy`: `cargo clippy --manifest-path src-tauri/Cargo.toml`
2. `rust-tests`: `cargo test --manifest-path src-tauri/Cargo.toml`
3. `frontend-check`: `bun run check`
4. `frontend-build`: `bun run build`
5. `harness-types`: `bun run agent:typecheck`
6. `harness-evals`: `bun test scripts/agent`
