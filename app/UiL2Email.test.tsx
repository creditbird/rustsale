// Headless: the Email Outreach screen against the real native module.
// The file name sorts it after the screens that prepare the shared demo folder (see screen-port-plan §5).
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Host, Leads, Templates } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l2-email-${Date.now()}`;

async function openEmail() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-email_outreach");
  await app.waitFor("id:email-search");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openEmail>>, s: string) => app.texts().some((t) => t.includes(s));

async function findTemplate(name: string): Promise<{ id: number; name: string; subject: string } | undefined> {
  return ((await Templates.emailTemplates()) as { id: number; name: string; subject: string }[])
    .find((template) => template.name === name);
}

test("an email template is created, edited and deleted through the core", async () => {
  const stamp = Date.now();
  const name = `Mẫu email ${stamp}`;
  await Host.start(dir);
  const app = await openEmail();
  await app.press("id:email-tpl-add");
  await app.press("id:email-tpl-save");
  await app.waitFor("id:email-tpl-form-error");
  await app.type("id:email-tpl-name", name);
  await app.type("id:email-tpl-subject", `Tiêu đề ${stamp}`);
  await app.type("id:email-tpl-body", `Nội dung ${stamp}`);
  await app.press("id:email-tpl-save");
  await app.waitUntil(() => has(app, name));
  let template: { id: number; name: string; subject: string } | undefined;
  await eventually(async () => {
    template = await findTemplate(name);
    return template !== undefined;
  });
  if (!template) throw new Error("the created email template is not in the core");
  await app.press(`id:email-tpl-edit-${template.id}`);
  await app.clear("id:email-tpl-subject");
  await app.type("id:email-tpl-subject", `Tiêu đề mới ${stamp}`);
  await app.press("id:email-tpl-save");
  await eventually(async () => (await findTemplate(name))?.subject === `Tiêu đề mới ${stamp}`);
  await app.press(`id:email-tpl-del-${template.id}`);
  await app.press("id:email-tpl-confirm-del");
  await eventually(async () => (await findTemplate(name)) === undefined);
  await app.waitUntil(() => !has(app, name));
});

test("sending with no SMTP account shows the error and sends nothing", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const templateId = (await Templates.createEmailTemplate(
    `Mẫu gửi ${stamp}`,
    `Tiêu đề gửi ${stamp}`,
    `Nội dung gửi ${stamp}`,
  )) as number;
  await Leads.add({
    name: `Lead L2E ${stamp}`,
    email: `l2e-${stamp}@example.test`,
    status: "New",
  });
  const app = await openEmail();
  await app.waitUntil(() => has(app, "Chưa có tài khoản SMTP"));
  await app.type("id:email-search", `Lead L2E ${stamp}`);
  await app.wait(400);
  await app.waitUntil(() => has(app, "Đã chọn 1 / 1 lead"));
  await eventually(async () => app.shows(`id:email-tpl-${templateId}`));
  await app.press(`id:email-tpl-${templateId}`);
  await app.press("id:email-run");
  await app.waitFor("id:email-run-error");
  await app.waitUntil(() => has(app, "Chưa có tài khoản SMTP"));
  expect(has(app, "Đang gửi")).toBe(false);
});
