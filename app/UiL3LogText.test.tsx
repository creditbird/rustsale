// Headless: the core's English campaign log never reaches the Zalo or Email screen as-is.
import { open, test } from "nsh/test";
import { Host } from "rustsale/native";
import { vietnameseCampaignLog } from "./stores/campaignLog";

const dir = `/tmp/rustsale-v3-ui-l3-log-${Date.now()}`;

const has = (app: Awaited<ReturnType<typeof openEventApp>>, s: string) => app.texts().some((t) => t.includes(s));

async function openEventApp() {
  await Host.start(dir);
  const app = await open("./harness/EventApp.tsx");
  await app.setViewport(1280, 800);
  return app;
}

test("the core's campaign messages map to Vietnamese sentences", () => {
  const cases: [string, string][] = [
    ["Message sent to Nguyễn An (uid-1)", "Đã gửi tin cho Nguyễn An (UID uid-1)."],
    ["Message sent to Nguyễn An (uid-1) (Mock)", "Đã gửi tin cho Nguyễn An (UID uid-1) (bản mô phỏng)."],
    ["Failed to send Zalo message to An (uid-1): ApiError", "Gửi tin Zalo cho An (UID uid-1) thất bại."],
    ["Session not found for IMEI: abc", "Không tìm thấy phiên đăng nhập Zalo cho tài khoản abc."],
    ["No leads selected.", "Chưa chọn lead nào."],
    ["No leads selected; no emails were sent.", "Chưa chọn lead nào nên chưa gửi email nào."],
    ["Processed 1/2 leads", "Đã xử lý 1/2 lead."],
    ["Email outreach completed. 3 emails sent.", "Gửi email xong: 3 email đã gửi."],
    ["Successfully sent email to An (an@example.test)", "Đã gửi email cho An (an@example.test)."],
    ["Failed sending email to An (an@example.test): timeout", "Gửi email cho An (an@example.test) thất bại."],
    ["Skipping lead 'An' (ID 7): No valid email address", "Bỏ qua An (ID 7) vì chưa có email hợp lệ."],
    ["[Burst Limit] Sent 5 messages. Pausing 30 seconds to protect Zalo account...", "Đã gửi 5 tin; tạm nghỉ 30 giây để bảo vệ tài khoản Zalo…"],
    ["Skipped Zalo lookup for An (0901): Previously marked as BLOCKED_STRANGER", "Bỏ qua tra cứu Zalo cho An (0901) vì trước đó đã đánh dấu người lạ đã chặn."],
  ];
  for (const [input, expected] of cases) {
    const actual = vietnameseCampaignLog(input);
    if (actual !== expected) throw new Error(`${input}\n  expected: ${expected}\n  actual:   ${actual}`);
  }
  // Text the UI itself wrote is already Vietnamese and must pass through unchanged.
  const uiLines = ["Đang gửi 2/5 L2P", "Bắt đầu gửi tới 3 lead.", "Dòng log L2Q khi rời màn"];
  for (const line of uiLines) {
    if (vietnameseCampaignLog(line) !== line) throw new Error(`UI line changed: ${line}`);
  }
});

test("the Zalo and Email screens show the translated log", async () => {
  const app = await openEventApp();

  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  await app.press("id:emit-out-failed");
  await app.waitUntil(() => has(app, "Không tìm thấy phiên đăng nhập Zalo."));
  if (has(app, "Session not found")) throw new Error("the Zalo log still shows the core's English");

  await app.press("id:nav-email_outreach");
  await app.waitFor("id:email-search");
  await app.press("id:emit-email-log");
  await app.waitUntil(() => has(app, "Đã gửi email cho An (an@example.test)."));
  await app.press("id:emit-email-progress");
  await app.waitUntil(() => has(app, "Đã xử lý 1/2 lead."));
  await app.press("id:emit-email-complete");
  await app.waitUntil(() => has(app, "Gửi email xong: 2 email đã gửi."));
  for (const raw of ["Successfully sent email", "Processed 1/2 leads", "Email outreach completed"]) {
    if (has(app, raw)) throw new Error(`the email log still shows the core's English: ${raw}`);
  }
});
