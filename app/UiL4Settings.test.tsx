// Headless: the Settings screen against the real native module on a temp folder.
// The file name sorts it after the screens that prepare the shared demo folder (see screen-port-plan §5).
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { ConversationNetwork, Demo, Host, Settings, Templates, Zalo } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l4-${Date.now()}`;

async function openSettings() {
  await Demo.prepare(dir);
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-settings");
  await app.waitFor("id:settings-tab-general");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openSettings>>, s: string) =>
  app.texts().some((t) => t.includes(s));

async function findSmtp(name: string): Promise<{ id: number; name: string } | undefined> {
  return ((await Templates.smtpAccounts()) as { id: number; name: string }[])
    .find((account) => account.name === name);
}

test("a notification switch is saved through the core and read back", async () => {
  await Host.start(dir);
  const before = (await Settings.notifications()) as { sound_enabled: boolean };
  const app = await openSettings();
  await app.waitFor("id:settings-notif-sound");
  await app.press("id:settings-notif-sound");
  await eventually(async () => {
    const after = (await Settings.notifications()) as { sound_enabled: boolean };
    return after.sound_enabled === !before.sound_enabled;
  });
});

test("the currency form refuses a bad rate and saves a good one", async () => {
  await Host.start(dir);
  const before = (await Settings.currency()) as { base_currency: string; exchange_rate: number };
  const app = await openSettings();
  await app.waitFor("id:settings-rate");
  await app.clear("id:settings-rate");
  await app.type("id:settings-rate", "0");
  await app.press("id:settings-currency-save");
  await app.waitFor("id:settings-currency-error");
  expect(has(app, "lớn hơn 0")).toBe(true);
  expect(((await Settings.currency()) as { exchange_rate: number }).exchange_rate).toBe(before.exchange_rate);

  await app.press("id:settings-currency-USD");
  await app.clear("id:settings-rate");
  await app.type("id:settings-rate", "26000");
  await app.press("id:settings-currency-save");
  await eventually(async () => {
    const after = (await Settings.currency()) as { base_currency: string; exchange_rate: number };
    return after.base_currency === "USD" && after.exchange_rate === 26000;
  });
  await app.waitUntil(() => !has(app, "Tỷ giá phải là số lớn hơn 0."));
});

test("an SMTP account is validated, saved and deleted through the core", async () => {
  const stamp = Date.now();
  const name = `SMTP L4 ${stamp}`;
  await Host.start(dir);
  const app = await openSettings();
  await app.press("id:settings-tab-accounts");
  await app.press("id:settings-tab-email");
  await app.waitFor("id:settings-smtp-add");
  await app.press("id:settings-smtp-add");
  await app.waitFor("id:settings-smtp-name");
  await app.press("id:settings-smtp-save");
  await app.waitFor("id:settings-smtp-error");
  expect(has(app, "Cần nhập tên tài khoản")).toBe(true);
  expect(await findSmtp(name)).toBeUndefined();

  await app.type("id:settings-smtp-name", name);
  await app.type("id:settings-smtp-host", "smtp.example.test");
  await app.type("id:settings-smtp-from-email", `l4-${stamp}@example.test`);
  await app.press("id:settings-smtp-save");
  let account: { id: number; name: string } | undefined;
  await eventually(async () => {
    account = await findSmtp(name);
    return account !== undefined;
  });
  if (!account) throw new Error("the saved SMTP account is not in the core");
  await app.waitUntil(() => has(app, name));

  await app.press(`id:settings-smtp-del-${account.id}`);
  await app.press("id:settings-smtp-confirm-del");
  await eventually(async () => (await findSmtp(name)) === undefined);
  await app.waitUntil(() => !has(app, name));
});

test("each account tab reads its channel list from the core", async () => {
  await Host.start(dir);
  const app = await openSettings();
  await app.press("id:settings-tab-accounts");
  await app.waitFor("id:settings-tab-zalo");
  await app.waitUntil(() => has(app, "Tài khoản Zalo"));
  await app.press("id:settings-tab-email");
  await app.waitUntil(() => has(app, "Tài khoản email"));
  await app.press("id:settings-tab-facebook");
  await app.waitUntil(() => has(app, "Trang Facebook"));
  await app.press("id:settings-tab-whatsapp");
  await app.waitUntil(() => has(app, "Tài khoản WhatsApp"));
});

test("the language choice is saved through the core and read back", async () => {
  await Host.start(dir);
  const app = await openSettings();
  await app.waitFor("id:settings-lang-vi");
  expect(((await Settings.language()) as { language: string }).language).toBe("vi");

  await app.press("id:settings-lang-en");
  await eventually(async () => ((await Settings.language()) as { language: string }).language === "en");
  await app.press("id:settings-lang-vi");
  await eventually(async () => ((await Settings.language()) as { language: string }).language === "vi");
});

test("the autostart switch shows what the native module reports", async () => {
  await Host.start(dir);
  const app = await openSettings();
  const state = (await Settings.autostart()) as { enabled: boolean; supported: boolean };
  if (state.supported) {
    await app.waitFor("id:settings-autostart");
  } else {
    await app.waitFor("id:settings-autostart-unsupported");
  }
});

test("a Zalo account offers reconnect, and a missing account is refused", async () => {
  await Host.start(dir);
  await Demo.seedZaloAccount();
  const app = await openSettings();
  await app.press("id:settings-tab-accounts");
  await app.waitFor("id:settings-zalo-demo-zalo-uid");
  await app.waitFor("id:settings-zalo-reconnect-demo-zalo-uid");
  await expect(Zalo.reconnect("missing-imei")).rejects.toMatchObject({ code: "not_found" });
});

test("the Facebook dialog reads the token from a redirect URL before asking Meta", async () => {
  await Host.start(dir);
  const app = await openSettings();
  await app.press("id:settings-tab-accounts");
  await app.press("id:settings-tab-facebook");
  await app.waitFor("id:settings-fb-add");
  await app.press("id:settings-fb-add");
  await app.waitFor("id:settings-fb-discover-token");
  await app.type("id:settings-fb-discover-token", "https://example.test/cb?state=only");
  await app.press("id:settings-fb-discover");
  await app.waitFor("id:settings-fb-error");
  expect(has(app, "Cần nhập access token")).toBe(true);
});

test("the native module draws a WhatsApp QR code as a PNG image", async () => {
  await Host.start(dir);
  const url = ConversationNetwork.whatsappQrImage("2@qr-test-content") as string;
  expect(url.startsWith("data:image/png;base64,")).toBe(true);
  expect(() => ConversationNetwork.whatsappQrImage("   ")).toThrow();
});
