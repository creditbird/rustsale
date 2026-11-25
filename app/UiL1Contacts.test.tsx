// Headless: the Contacts screen (converted leads) against the real native module.
// The file name sorts it after the screens that prepare the shared demo folder (see screen-port-plan §5).
import { expect, open, test } from "nsh/test";
import { Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l1-contacts-${Date.now()}`;

async function openContacts() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-contacts");
  await app.waitFor("id:contact-search");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openContacts>>, s: string) => app.texts().some((t) => t.includes(s));

test("only converted leads are listed, and the search filters them", async () => {
  const stamp = Date.now();
  const contactName = `Liên hệ ${stamp}`;
  const leadName = `Khách thường ${stamp}`;
  await Host.start(dir);
  const before = ((await Leads.page({ status: "Converted" }, null, 200)) as { total: number }).total;
  await Leads.add({ name: contactName, status: "Converted" });
  await Leads.add({ name: leadName, status: "New" });
  const app = await openContacts();
  await app.waitUntil(() => has(app, contactName));
  expect(has(app, leadName)).toBe(false);
  const total = before + 1;
  await app.waitUntil(() => has(app, `${total} / ${total} liên hệ`));
  await app.type("id:contact-search", contactName);
  await app.waitUntil(() => has(app, "1 / 1 liên hệ"));
  await app.clear("id:contact-search");
  await app.type("id:contact-search", leadName);
  await app.waitUntil(() => has(app, "Không có liên hệ phù hợp"));
});

test("opening a contact shows the shared detail panel", async () => {
  const name = `Liên hệ chi tiết ${Date.now()}`;
  await Host.start(dir);
  const id = (await Leads.add({
    name,
    status: "Converted",
    company: "Công ty Test",
    email: "lienhe@example.com",
    phone: "0901234567",
  })) as number;
  const app = await openContacts();
  await app.waitFor(`id:contact-row-${id}`);
  await app.press(`id:contact-row-${id}`);
  await app.waitFor("id:lead-detail");
  await app.waitUntil(() => has(app, "lienhe@example.com"));
  expect(has(app, name)).toBe(true);
});

test("a long name does not push the Đóng button out of the detail panel", async () => {
  const name = `Phòng Khám Vật Lý Trị Liệu Tân Phú Therapy ${Date.now()} Chi Nhánh Số Một`;
  await Host.start(dir);
  const id = (await Leads.add({ name, status: "Converted" })) as number;
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-contacts");
  await app.waitFor(`id:contact-row-${id}`);
  await app.press(`id:contact-row-${id}`);
  await app.waitFor("id:lead-detail");
  expect(app.shows("id:lead-close")).toBe(true);
  expect(has(app, "Đóng")).toBe(true);
});
