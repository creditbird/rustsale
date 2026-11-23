// Headless: the Zalo Outreach screen against the real native module.
// The file name sorts it after the screens that prepare the shared demo folder (see screen-port-plan §5).
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Campaigns, Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l2-outreach-${Date.now()}`;

async function openOutreach() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openOutreach>>, s: string) => app.texts().some((t) => t.includes(s));

test("the lead list filters and pages in the core", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  for (let index = 0; index < 55; index += 1) {
    await Leads.add({ name: `Lead L2A ${stamp} ${index}`, phone: "0901234567", status: "New" });
  }
  const app = await openOutreach();
  await app.type("id:zalo-search", `Lead L2A ${stamp}`);
  await app.waitUntil(() => has(app, "50 / 55 lead"));
  await app.press("id:zalo-more");
  await app.waitUntil(() => has(app, "55 / 55 lead"));
  await app.clear("id:zalo-search");
  await app.type("id:zalo-search", `khong-co-${stamp}`);
  await app.waitUntil(() => has(app, "0 / 0 lead"));
});

test("selecting every match then excluding leads changes the count", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const ids: number[] = [];
  for (let index = 0; index < 3; index += 1) {
    const id = (await Leads.add({
      name: `Lead L2B ${stamp} ${index}`,
      phone: "0901234567",
      status: "New",
    })) as number;
    ids.push(id);
  }
  const app = await openOutreach();
  await app.type("id:zalo-search", `Lead L2B ${stamp}`);
  await app.wait(400);
  await app.waitUntil(() => has(app, "3 / 3 lead"));
  await app.waitUntil(() => has(app, "Đã chọn 3 / 3 lead"));
  await app.press(`id:zalo-row-${ids[0]}`);
  await app.waitUntil(() => has(app, "Đã chọn 2 / 3 lead"));
  await app.press(`id:zalo-row-${ids[1]}`);
  await app.waitUntil(() => has(app, "Đã chọn 1 / 3 lead"));
  await app.press("id:zalo-clear");
  await app.waitUntil(() => has(app, "Đã chọn 0 / 3 lead"));
  await app.press(`id:zalo-row-${ids[2]}`);
  await app.waitUntil(() => has(app, "Đã chọn 1 / 3 lead"));
  await app.press("id:zalo-select-all");
  await app.waitUntil(() => has(app, "Đã chọn 3 / 3 lead"));
});

test("running without a Zalo account shows the error and sends nothing", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const templateId = (await Campaigns.createOutreachTemplate(
    `Mẫu L2 ${stamp}`,
    `Nội dung ${stamp}`,
    null,
  )) as number;
  await Leads.add({ name: `Lead L2C ${stamp}`, phone: "0901234567", status: "New" });
  const app = await openOutreach();
  await app.waitUntil(() => has(app, "Chưa có tài khoản Zalo"));
  await app.type("id:zalo-search", `Lead L2C ${stamp}`);
  await app.waitUntil(() => has(app, "Đã chọn 1 / 1 lead"));
  await eventually(async () => app.shows(`id:zalo-tpl-${templateId}`));
  await app.press(`id:zalo-tpl-${templateId}`);
  await app.press("id:zalo-run");
  await app.waitFor("id:zalo-run-error");
  await app.waitUntil(() => has(app, "Chưa có tài khoản Zalo"));
  expect(has(app, "Đang gửi")).toBe(false);
});

test("a campaign's progress outlives the screen and a failed run frees the button", async () => {
  await Host.start(dir);
  const app = await open("./harness/EventApp.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  await app.press("id:emit-out-progress");
  await app.waitUntil(() => has(app, "Đang gửi 2/5 L2P"));
  await app.press("id:nav-leads");
  await app.waitFor("id:emit-out-log");
  await app.press("id:emit-out-log");
  await app.wait(300);
  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  await app.waitUntil(() => has(app, "Đang gửi 2/5 L2P"));
  await app.waitUntil(() => has(app, "Dòng log L2Q khi rời màn"));
  await app.press("id:emit-out-failed");
  await app.waitUntil(() => has(app, "Session not found L2R"));
});
