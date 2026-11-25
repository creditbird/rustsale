import { Image, Scroll, cleanup, signal, Text, View } from "nsh";
import { Button as Base, Input as BaseInput } from "nsh/base";
import { checkForAppUpdate, openUpdateModal, updaterState } from "../stores/updater";
import { ConversationNetwork, Desktop, Facebook, Host, Settings, Templates, Zalo } from "rustsale/native";
import { Badge, Button, Checkbox, EmptyState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { startCore } from "../stores/core";
import { facebookTokenFromInput } from "../stores/facebookToken";
import { notify, report } from "../stores/toast";

interface NotificationSettings {
  enabled: boolean;
  sound_enabled: boolean;
  privacy_mode: boolean;
  mute_groups: boolean;
}

interface TraySettings {
  close_to_tray: boolean;
  dnd_enabled: boolean;
}

interface CurrencySettings {
  base_currency: string;
  exchange_rate: number;
}

interface ZaloAccount {
  accountKey: string;
  displayName: string | null;
  avatar: string | null;
  authStatus: string;
  authCheckedAt: string | null;
  authError: string | null;
  runtimeStatus: string;
  lastConnectedAt: string | null;
  lastError: string | null;
}

interface SmtpAccount {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  from_email: string;
  from_name: string;
  security: string;
}

interface FacebookAccount {
  channelAccountId: number;
  pageId: string;
  pageName: string | null;
  avatarUrl: string | null;
  tokenStatus: string;
  grantedScopes: string[];
  runtimeStatus: string;
  lastConnectedAt: string | null;
  lastError: string | null;
  updatedAt: string;
}

interface WhatsAppAccount {
  id: number;
  provider_account_key: string;
  display_name: string | null;
  phone_number: string | null;
}

interface DiscoveredFacebookPage {
  id: string;
  name: string;
  category: string | null;
  avatarUrl: string | null;
  accessToken: string;
  permissions: string[];
}

const SECURITY = ["STARTTLS", "TLS", "NONE"] as const;

const LANGUAGES = [
  { id: "vi", label: "Tiếng Việt" },
  { id: "en", label: "English" },
] as const;

function authTone(status: string): BadgeTone {
  if (status === "valid" || status === "connected") return "success";
  if (status === "expired" || status === "error" || status === "invalid" || status === "revoked") return "danger";
  if (status === "checking" || status === "degraded" || status === "connecting") return "warning";
  return "neutral";
}

function authLabel(status: string): string {
  if (status === "valid") return "Hợp lệ";
  if (status === "checking") return "Đang kiểm tra";
  if (status === "expired") return "Hết hạn";
  if (status === "error") return "Lỗi xác thực";
  return "Chưa rõ";
}

function runtimeLabel(status: string): string {
  if (status === "connected") return "Đang chạy";
  if (status === "connecting") return "Đang kết nối";
  if (status === "degraded") return "Chập chờn";
  if (status === "invalid") return "Không hợp lệ";
  return "Đã ngắt";
}

function tokenLabel(status: string): string {
  if (status === "valid") return "Token hợp lệ";
  if (status === "expired") return "Token hết hạn";
  if (status === "invalid") return "Token không hợp lệ";
  if (status === "revoked") return "Token bị thu hồi";
  return "Chưa rõ token";
}

/** Checks the currency form before the core is asked. */
export function validateCurrency(base: string, rate: string): string | null {
  if (base !== "VND" && base !== "USD") return "Chỉ chọn VND hoặc USD.";
  const value = Number(rate.replace(/,/g, "").trim());
  if (!Number.isFinite(value) || value <= 0) return "Tỷ giá phải là số lớn hơn 0.";
  return null;
}

/** Checks the SMTP form before the core is asked. */
export function validateSmtp(v: { name: string; host: string; port: string; fromEmail: string }): string | null {
  if (v.name.trim() === "") return "Cần nhập tên tài khoản.";
  if (v.host.trim() === "") return "Cần nhập SMTP host.";
  const port = Number(v.port);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return "Cổng SMTP phải trong khoảng 1–65535.";
  if (v.fromEmail.trim() === "") return "Cần nhập email người gửi.";
  return null;
}

/** Checks the Facebook form before the core is asked. */
export function validateFacebook(pageId: string, token: string): string | null {
  if (pageId.trim() === "") return "Cần nhập Page ID.";
  if (token.trim() === "") return "Cần nhập access token.";
  return null;
}

/** Settings, part A: notifications, tray, currency and app updates. */
function GeneralSettings() {
  const notifications = signal<NotificationSettings>({
    enabled: true,
    sound_enabled: true,
    privacy_mode: false,
    mute_groups: false,
  });
  const tray = signal<TraySettings>({ close_to_tray: true, dnd_enabled: false });
  const base = signal("VND");
  const rate = signal("25400");
  const ready = signal(false);
  const busy = signal(false);
  const formError = signal<string | null>(null);
  const testing = signal(false);
  const language = signal("vi");
  const savingLanguage = signal(false);
  const autostart = signal(false);
  const autostartSupported = signal(true);
  const autostartBusy = signal(false);

  const load = async () => {
    try {
      await startCore();
      const [n, t, c, lang] = await Promise.all([
        Settings.notifications(),
        Settings.tray(),
        Settings.currency(),
        Settings.language(),
      ]);
      notifications.set(n as NotificationSettings);
      tray.set(t as TraySettings);
      const currency = c as CurrencySettings;
      base.set(currency.base_currency);
      rate.set(String(currency.exchange_rate));
      language.set((lang as { language: string }).language);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const loadAutostart = async () => {
    try {
      const state = (await Settings.autostart()) as { enabled: boolean; supported: boolean };
      autostart.set(state.enabled);
      autostartSupported.set(state.supported);
    } catch (e) {
      autostartSupported.set(false);
      report(e, { retry: () => { void loadAutostart(); } });
    }
  };

  const saveSettings = async (next: NotificationSettings) => {
    if (busy()) return;
    const before = notifications();
    notifications.set(next);
    busy.set(true);
    try {
      await Settings.updateNotifications(next);
    } catch (e) {
      notifications.set(before);
      report(e, { retry: () => { void saveSettings(next); } });
    } finally {
      busy.set(false);
    }
  };

  const setTray = async (next: TraySettings, saved: () => Promise<unknown>) => {
    if (busy()) return;
    const before = tray();
    tray.set(next);
    busy.set(true);
    try {
      await saved();
    } catch (e) {
      tray.set(before);
      report(e, { retry: () => { void setTray(next, saved); } });
    } finally {
      busy.set(false);
    }
  };

  const saveCurrency = async () => {
    if (busy()) return;
    const problem = validateCurrency(base(), rate());
    if (problem) { formError.set(problem); return; }
    formError.set(null);
    busy.set(true);
    try {
      const saved = (await Settings.updateCurrency(base(), Number(rate().replace(/,/g, "").trim()))) as CurrencySettings;
      base.set(saved.base_currency);
      rate.set(String(saved.exchange_rate));
      notify("Đã lưu tiền tệ", "success");
    } catch (e) {
      const f = report(e, { retry: () => { void saveCurrency(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      busy.set(false);
    }
  };

  const testNotification = async () => {
    if (testing()) return;
    testing.set(true);
    try {
      await Desktop.testNotification();
      notify("Đã gửi thông báo thử", "success");
    } catch (e) {
      report(e, { retry: () => { void testNotification(); } });
    } finally {
      testing.set(false);
    }
  };

  const updateBusy = () => {
    const status = updaterState().status;
    return status === "checking" || status === "downloading";
  };

  const updateStatusText = () => {
    const s = updaterState();
    if (s.status === "checking") return "Đang kiểm tra…";
    if (s.status === "available" && s.availableVersion) return `Có bản mới v${s.availableVersion}.`;
    if (s.status === "up-to-date") return "Bạn đang dùng bản mới nhất.";
    if (s.status === "downloading") return `Đang tải bản cập nhật… ${s.progress.percentage}%`;
    if (s.status === "downloaded") return "Đã tải xong bản cập nhật. Khởi động lại để cài đặt.";
    if (s.status === "error") return s.errorMessage ?? "Không kiểm tra được cập nhật.";
    return "Chưa kiểm tra.";
  };

  const checkUpdates = async () => {
    await checkForAppUpdate({ silent: false, openOnAvailable: false });
  };

  const chooseLanguage = async (next: string) => {
    if (savingLanguage() || language() === next) return;
    const before = language();
    language.set(next);
    savingLanguage.set(true);
    try {
      const saved = (await Settings.setLanguage(next)) as { language: string };
      language.set(saved.language);
      notify("Đã lưu ngôn ngữ", "success");
    } catch (e) {
      language.set(before);
      report(e, { retry: () => { void chooseLanguage(next); } });
    } finally {
      savingLanguage.set(false);
    }
  };

  const saveAutostart = async (next: boolean) => {
    if (autostartBusy()) return;
    const before = autostart();
    autostart.set(next);
    autostartBusy.set(true);
    try {
      const saved = (await Settings.setAutostart(next)) as { enabled: boolean };
      autostart.set(saved.enabled);
      notify(saved.enabled ? "Sẽ mở RustSale khi đăng nhập máy" : "Đã tắt mở RustSale khi đăng nhập máy", "success");
    } catch (e) {
      autostart.set(before);
      report(e, { retry: () => { void saveAutostart(next); } });
    } finally {
      autostartBusy.set(false);
    }
  };

  void load();
  void loadAutostart();

  return (
    <View class="gap-5 p-6">
      <View class="gap-3 rounded-(--radius) border border-border bg-card p-4">
        <Text class="text-base font-semibold text-foreground">Thông báo</Text>
        <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
        <If when={ready()}>
          <Checkbox testID="settings-notif-enabled" checked={notifications().enabled}
            label="Thông báo trên máy tính"
            onChange={(v) => { void saveSettings({ ...notifications(), enabled: v }); }} />
          <Checkbox testID="settings-notif-sound" checked={notifications().sound_enabled}
            label="Âm thanh"
            onChange={(v) => { void saveSettings({ ...notifications(), sound_enabled: v }); }} />
          <Checkbox testID="settings-notif-privacy" checked={notifications().privacy_mode}
            label="Chế độ riêng tư (ẩn nội dung tin nhắn)"
            onChange={(v) => { void saveSettings({ ...notifications(), privacy_mode: v }); }} />
          <Checkbox testID="settings-notif-mute" checked={notifications().mute_groups}
            label="Tắt thông báo nhóm"
            onChange={(v) => { void saveSettings({ ...notifications(), mute_groups: v }); }} />
          <View class="my-1 border-t border-border" />
          <Checkbox testID="settings-tray-close" checked={tray().close_to_tray}
            label="Đóng cửa sổ thì thu về khay"
            onChange={(v) => { void setTray({ ...tray(), close_to_tray: v }, () => Settings.setCloseToTray(v)); }} />
          <Checkbox testID="settings-tray-dnd" checked={tray().dnd_enabled}
            label="Không làm phiền"
            onChange={(v) => { void setTray({ ...tray(), dnd_enabled: v }, () => Settings.setDoNotDisturb(v)); }} />
          <View class="flex-row items-center gap-2 pt-1">
            <Button testID="settings-test-notify" label="Gửi thử" disabled={testing() || !notifications().enabled}
              onPress={() => { void testNotification(); }} />
            <If when={tray().dnd_enabled}>
              <Text class="text-sm text-warning">Đang bật Không làm phiền.</Text>
            </If>
          </View>
        </If>
      </View>

      <View class="gap-3 rounded-(--radius) border border-border bg-card p-4">
        <Text class="text-base font-semibold text-foreground">Tiền tệ</Text>
        <View class="flex-row gap-2">
          <Each of={["VND", "USD"]} key={(c) => c}>
            {(c) => (
              <Base testID={`settings-currency-${c()}`} accessibilityLabel={c()}
                onPress={() => base.set(c())}
                class={`rounded-full border px-3 py-1 ${base() === c() ? "border-primary bg-primary" : "border-border bg-card"}`}>
                <Text class={`text-sm ${base() === c() ? "text-primary-foreground" : "text-foreground"}`}>{c()}</Text>
              </Base>
            )}
          </Each>
        </View>
        <Input testID="settings-rate" value={rate()} placeholder="Tỷ giá USD → VND"
          onValue={(v) => rate.set(v)} onSubmit={() => { void saveCurrency(); }} />
        <If when={formError() !== null}>
          <Text testID="settings-currency-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
        </If>
        <View class="flex-row justify-end">
          <Button testID="settings-currency-save" variant="primary" label="Lưu" disabled={busy()}
            onPress={() => { void saveCurrency(); }} />
        </View>
      </View>

      <View class="gap-3 rounded-(--radius) border border-border bg-card p-4">
        <Text class="text-base font-semibold text-foreground">Ngôn ngữ</Text>
        <View class="flex-row gap-2">
          <Each of={LANGUAGES} key={(l) => l.id}>
            {(l) => (
              <Base testID={`settings-lang-${l().id}`} accessibilityLabel={l().label}
                onPress={() => { void chooseLanguage(l().id); }}
                class={`rounded-full border px-3 py-1 ${language() === l().id ? "border-primary bg-primary" : "border-border bg-card"}`}>
                <Text class={`text-sm ${language() === l().id ? "text-primary-foreground" : "text-foreground"}`}>{l().label}</Text>
              </Base>
            )}
          </Each>
        </View>
        <Text class="text-xs text-muted-foreground">Lựa chọn được lưu trên máy này.</Text>
      </View>

      <View class="gap-3 rounded-(--radius) border border-border bg-card p-4">
        <Text class="text-base font-semibold text-foreground">Khởi động cùng máy</Text>
        <If when={autostartSupported()}>
          <Checkbox testID="settings-autostart" checked={autostart()}
            label="Mở RustSale khi đăng nhập máy tính"
            onChange={(v) => { void saveAutostart(v); }} />
        </If>
        <If when={!autostartSupported()}>
          <Text testID="settings-autostart-unsupported" class="text-sm text-muted-foreground">
            Chưa hỗ trợ trên hệ điều hành này.
          </Text>
        </If>
      </View>

      <View class="gap-3 rounded-(--radius) border border-border bg-card p-4">
        <Text class="text-base font-semibold text-foreground">Cập nhật</Text>
        <Text testID="settings-update-status" class="text-sm text-muted-foreground">{updateStatusText()}</Text>
        <If when={updaterState().availableVersion !== null}>
          <Text class="text-sm text-foreground">{`Phiên bản v${updaterState().availableVersion ?? ""}: ${updaterState().releaseNotes ?? ""}`}</Text>
          <View class="flex-row justify-end">
            <Button testID="settings-update-install" variant="primary" label="Tải và cài"
              disabled={updateBusy()} onPress={openUpdateModal} />
          </View>
        </If>
        <View class="flex-row justify-end">
          <Button testID="settings-update-check" label="Kiểm tra cập nhật" disabled={updateBusy()}
            onPress={() => { void checkUpdates(); }} />
        </View>
      </View>
    </View>
  );
}

/** Zalo accounts: list, verify, delete and the QR login dialog. */
function ZaloAccounts() {
  const accounts = signal<ZaloAccount[]>([]);
  const ready = signal(false);
  const busy = signal(false);
  const removing = signal<ZaloAccount | null>(null);
  const qrOpen = signal(false);
  const qrImage = signal<string | null>(null);
  const qrStatus = signal("Đang kết nối Zalo…");
  const qrError = signal<string | null>(null);
  const starting = signal(false);
  const reconnecting = signal<string | null>(null);

  const load = async () => {
    try {
      await startCore();
      accounts.set((await Zalo.accounts()) as ZaloAccount[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const verify = async (account: ZaloAccount) => {
    if (busy()) return;
    busy.set(true);
    try {
      await Zalo.verifySession(account.accountKey);
      notify("Đã kiểm tra phiên Zalo", "success");
    } catch (e) {
      report(e, { refresh: () => { void load(); } });
    } finally {
      busy.set(false);
    }
    await load();
  };

  const reconnect = async (account: ZaloAccount) => {
    if (reconnecting() !== null) return;
    reconnecting.set(account.accountKey);
    try {
      await Zalo.reconnect(account.accountKey);
      notify("Đã kết nối lại Zalo", "success");
    } catch (e) {
      report(e, { refresh: () => { void load(); } });
    } finally {
      reconnecting.set(null);
    }
    await load();
  };

  const confirmRemove = async () => {
    const account = removing();
    removing.set(null);
    if (!account) return;
    try {
      await Zalo.deleteSession(account.accountKey);
      notify("Đã xoá tài khoản Zalo", "success");
    } catch (e) {
      report(e, { retry: () => { removing.set(account); } });
    }
    await load();
  };

  const startLogin = async () => {
    if (starting()) return;
    qrOpen.set(true);
    qrImage.set(null);
    qrError.set(null);
    qrStatus.set("Đang kết nối Zalo…");
    starting.set(true);
    try {
      await Zalo.startQrLogin();
    } catch (e) {
      qrError.set("Không đăng nhập được Zalo. Thử lại.");
      report(e, { retry: () => { void startLogin(); } });
    } finally {
      starting.set(false);
    }
  };

  const closeQr = () => {
    qrOpen.set(false);
    qrImage.set(null);
    qrError.set(null);
  };

  const stop = Host.onEvent((name, payload) => {
    if (name === "zalo-qr-code" && qrOpen()) {
      const p = payload as { image_base64?: string };
      if (p.image_base64) {
        qrImage.set(p.image_base64.startsWith("data:") ? p.image_base64 : `data:image/png;base64,${p.image_base64}`);
        qrStatus.set("Quét mã bằng ứng dụng Zalo trên điện thoại.");
      }
    } else if (name === "zalo-qr-scanned" && qrOpen()) {
      const p = payload as { display_name?: string };
      qrStatus.set(`Đã quét bởi ${p.display_name ?? "tài khoản"}. Xác nhận trên điện thoại.`);
    } else if (name === "zalo-qr-success") {
      closeQr();
      notify("Đã thêm tài khoản Zalo", "success");
      void load();
    } else if (name === "zalo-qr-error" && qrOpen()) {
      qrError.set("Đăng nhập Zalo thất bại. Thử lại.");
      qrStatus.set("");
    } else if (name === "zalo-qr-expired" && qrOpen()) {
      // The core ends the login on expiry; without this the dialog would keep a dead code on screen.
      qrImage.set(null);
      qrError.set("Mã QR đã hết hạn. Đóng hộp thoại rồi thêm lại.");
      qrStatus.set("");
    } else if (name === "zalo-qr-declined" && qrOpen()) {
      qrImage.set(null);
      qrError.set("Bạn đã từ chối đăng nhập trên điện thoại.");
      qrStatus.set("");
    }
  });
  cleanup(stop);

  void load();

  return (
    <View class="gap-3">
      <View class="flex-row items-center justify-between">
        <View class="gap-1">
          <Text class="text-base font-semibold text-foreground">Tài khoản Zalo</Text>
          <Text class="text-xs text-muted-foreground">Đăng nhập một lần, RustSale giữ phiên và tự kết nối lại.</Text>
        </View>
        <View class="flex-row gap-2">
          <Button label="Làm mới" onPress={() => { void load(); }} />
          <Button testID="settings-zalo-add" variant="primary" label="Thêm tài khoản Zalo"
            onPress={() => { void startLogin(); }} />
        </View>
      </View>

      <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && accounts().length === 0}>
        <EmptyState title="Chưa có tài khoản Zalo" hint="Bấm “Thêm tài khoản Zalo” rồi quét mã bằng ứng dụng Zalo." />
      </If>
      <View testID="settings-zalo-list" class="gap-2">
        <Each of={accounts()} key={(a) => a.accountKey}>
          {(a) => (
            <View testID={`settings-zalo-${a().accountKey}`} class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center gap-2">
                <View class="flex-1 gap-1">
                  <Text class="text-sm font-medium text-foreground">{a().displayName?.trim() || a().accountKey}</Text>
                  <Text class="font-mono text-xs text-muted-foreground">{a().accountKey}</Text>
                </View>
                <Badge label={authLabel(a().authStatus)} tone={authTone(a().authStatus)} />
                <Badge label={runtimeLabel(a().runtimeStatus)} tone={authTone(a().runtimeStatus)} />
              </View>
              <If when={a().authError !== null || a().lastError !== null}>
                <Text class="text-xs text-destructive">{a().authError ?? a().lastError ?? ""}</Text>
              </If>
              <View class="flex-row justify-end gap-2">
                <Button testID={`settings-zalo-reconnect-${a().accountKey}`} size="sm" label="Kết nối lại"
                  disabled={reconnecting() !== null} onPress={() => { void reconnect(a()); }} />
                <Button testID={`settings-zalo-verify-${a().accountKey}`} size="sm" label="Kiểm tra"
                  disabled={busy()} onPress={() => { void verify(a()); }} />
                <Button testID={`settings-zalo-del-${a().accountKey}`} size="sm" variant="destructive" label="Xoá"
                  onPress={() => removing.set(a())} />
              </View>
            </View>
          )}
        </Each>
      </View>

      <Modal open={removing() !== null} title="Xoá tài khoản Zalo?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`Phiên “${removing()?.displayName ?? removing()?.accountKey ?? ""}” sẽ bị xoá khỏi máy này.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="settings-zalo-confirm-del" variant="destructive" label="Xoá"
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={qrOpen()} title="Thêm tài khoản Zalo" onClose={closeQr}>
        <View testID="settings-zalo-qr" class="items-center gap-3">
          <If when={qrError() !== null}>
            <Text testID="settings-zalo-qr-error" class="text-sm text-destructive">{qrError() ?? ""}</Text>
          </If>
          <If when={qrImage() !== null}>
            <Image src={qrImage() ?? ""} class="h-56 w-56 rounded-(--radius) border border-border bg-background p-2" />
          </If>
          <Text class="text-sm text-muted-foreground">{qrStatus()}</Text>
          <View class="flex-row justify-end">
            <Button label="Đóng" onPress={closeQr} />
          </View>
        </View>
      </Modal>
    </View>
  );
}

/** SMTP accounts: list, add/edit form with validation, connection test and delete. */
function SmtpAccounts() {
  const accounts = signal<SmtpAccount[]>([]);
  const ready = signal(false);
  const formOpen = signal(false);
  const editingId = signal<number | null>(null);
  const name = signal("");
  const host = signal("");
  const port = signal("587");
  const username = signal("");
  const password = signal("");
  const fromEmail = signal("");
  const fromName = signal("");
  const security = signal<string>("STARTTLS");
  const formError = signal<string | null>(null);
  const saving = signal(false);
  const removing = signal<SmtpAccount | null>(null);
  const testResult = signal<string | null>(null);
  const testing = signal(false);

  const load = async () => {
    try {
      await startCore();
      const rows = (await Templates.smtpAccounts()) as SmtpAccount[];
      accounts.set(rows.filter((row) => typeof row.id === "number"));
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const openForm = (account?: SmtpAccount) => {
    editingId.set(account?.id ?? null);
    name.set(account?.name ?? "");
    host.set(account?.host ?? "");
    port.set(String(account?.port ?? 587));
    username.set(account?.username ?? "");
    password.set("");
    fromEmail.set(account?.from_email ?? "");
    fromName.set(account?.from_name ?? "");
    security.set(account?.security ?? "STARTTLS");
    formError.set(null);
    testResult.set(null);
    formOpen.set(true);
  };

  const submit = async () => {
    if (saving()) return;
    const problem = validateSmtp({ name: name(), host: host(), port: port(), fromEmail: fromEmail() });
    if (problem) { formError.set(problem); return; }
    formError.set(null);
    saving.set(true);
    const server: Record<string, unknown> = {
      name: name().trim(),
      host: host().trim(),
      port: Number(port()),
      username: username().trim(),
      password: password(),
      from_email: fromEmail().trim(),
      from_name: fromName().trim(),
      security: security(),
      created_at: new Date().toISOString(),
    };
    if (editingId() !== null) server.id = editingId();
    try {
      await Templates.saveSmtpServer(server);
      formOpen.set(false);
      notify("Đã lưu tài khoản email", "success");
      await load();
    } catch (e) {
      const f = report(e, { retry: () => { void submit(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      saving.set(false);
    }
  };

  const test = async () => {
    if (testing()) return;
    if (host().trim() === "") { formError.set("Cần nhập SMTP host."); return; }
    const portValue = Number(port());
    if (!Number.isInteger(portValue) || portValue < 1 || portValue > 65535) {
      formError.set("Cổng SMTP phải trong khoảng 1–65535.");
      return;
    }
    formError.set(null);
    testing.set(true);
    testResult.set(null);
    try {
      await Templates.testSmtpConnection(host().trim(), portValue);
      testResult.set(`Kết nối được ${host().trim()}:${portValue}.`);
      notify("Kết nối SMTP thành công", "success");
    } catch (e) {
      testResult.set("Không kết nối được tới SMTP.");
      report(e, { retry: () => { void test(); } });
    } finally {
      testing.set(false);
    }
  };

  const confirmRemove = async () => {
    const account = removing();
    removing.set(null);
    if (!account) return;
    try {
      await Templates.deleteSmtpServer(account.id);
      notify("Đã xoá tài khoản email", "success");
    } catch (e) {
      report(e, { retry: () => { removing.set(account); } });
    }
    await load();
  };

  void load();

  return (
    <View class="gap-3">
      <View class="flex-row items-center justify-between">
        <View class="gap-1">
          <Text class="text-base font-semibold text-foreground">Tài khoản email</Text>
          <Text class="text-xs text-muted-foreground">Mật khẩu đã lưu không hiện lại trên màn này.</Text>
        </View>
        <View class="flex-row gap-2">
          <Button label="Làm mới" onPress={() => { void load(); }} />
          <Button testID="settings-smtp-add" variant="primary" label="Thêm tài khoản email"
            onPress={() => openForm()} />
        </View>
      </View>

      <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && accounts().length === 0}>
        <EmptyState title="Chưa có tài khoản SMTP" hint="Thêm tài khoản SMTP trước khi chạy chiến dịch email." />
      </If>
      <View class="gap-2">
        <Each of={accounts()} key={(a) => String(a.id)}>
          {(a) => (
            <View testID={`settings-smtp-${a().id}`} class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center gap-2">
                <View class="flex-1 gap-1">
                  <Text class="text-sm font-medium text-foreground">{a().name}</Text>
                  <Text class="text-xs text-muted-foreground">{`${a().host}:${a().port} · ${a().from_email}`}</Text>
                </View>
                <Badge label={a().security} tone="neutral" />
              </View>
              <View class="flex-row justify-end gap-2">
                <Button testID={`settings-smtp-edit-${a().id}`} size="sm" label="Sửa"
                  onPress={() => openForm(a())} />
                <Button testID={`settings-smtp-del-${a().id}`} size="sm" variant="destructive" label="Xoá"
                  onPress={() => removing.set(a())} />
              </View>
            </View>
          )}
        </Each>
      </View>

      <Modal open={formOpen()} title={editingId() === null ? "Thêm tài khoản email" : "Sửa tài khoản email"}
        onClose={() => formOpen.set(false)} class="m-auto w-[560px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="gap-3">
          <Input testID="settings-smtp-name" value={name()} placeholder="Tên tài khoản (vd: SMTP chính)"
            onValue={(v) => name.set(v)} />
          <View class="flex-row gap-2">
            <View class="flex-1"><Input testID="settings-smtp-host" value={host()} placeholder="smtp.example.com"
              onValue={(v) => host.set(v)} /></View>
            <View class="w-[120px]"><Input testID="settings-smtp-port" value={port()} placeholder="587"
              onValue={(v) => port.set(v)} /></View>
          </View>
          <View class="flex-row gap-2">
            <View class="flex-1"><Input testID="settings-smtp-username" value={username()} placeholder="Username"
              onValue={(v) => username.set(v)} /></View>
            <View class="flex-1">
              <BaseInput testID="settings-smtp-password" masked value={password()}
                placeholder={editingId() === null ? "Mật khẩu" : "Để trống giữ mật khẩu cũ"}
                onChange={({ value }) => password.set(value)}
                class="h-9 rounded-(--radius) border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring" />
            </View>
          </View>
          <View class="flex-row gap-2">
            <View class="flex-1"><Input testID="settings-smtp-from-name" value={fromName()} placeholder="Tên người gửi"
              onValue={(v) => fromName.set(v)} /></View>
            <View class="flex-1"><Input testID="settings-smtp-from-email" value={fromEmail()} placeholder="Email người gửi"
              onValue={(v) => fromEmail.set(v)} /></View>
          </View>
          <View class="flex-row gap-2">
            <Each of={SECURITY} key={(s) => s}>
              {(s) => (
                <Base testID={`settings-smtp-sec-${s()}`} accessibilityLabel={s()}
                  onPress={() => security.set(s())}
                  class={`rounded-full border px-3 py-1 ${security() === s() ? "border-primary bg-primary" : "border-border bg-card"}`}>
                  <Text class={`text-sm ${security() === s() ? "text-primary-foreground" : "text-foreground"}`}>
                    {s() === "NONE" ? "Không mã hoá" : s()}
                  </Text>
                </Base>
              )}
            </Each>
          </View>
          <If when={formError() !== null}>
            <Text testID="settings-smtp-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <If when={testResult() !== null}>
            <Text testID="settings-smtp-test-result" class="text-sm text-muted-foreground">{testResult() ?? ""}</Text>
          </If>
          <View class="flex-row items-center justify-between gap-2">
            <Button testID="settings-smtp-test" size="sm" label="Kiểm tra kết nối" disabled={testing()}
              onPress={() => { void test(); }} />
            <View class="flex-row gap-2">
              <Button label="Huỷ" onPress={() => formOpen.set(false)} />
              <Button testID="settings-smtp-save" variant="primary" label="Lưu" disabled={saving()}
                onPress={() => { void submit(); }} />
            </View>
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá tài khoản email?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.name ?? ""}” sẽ bị xoá.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="settings-smtp-confirm-del" variant="destructive" label="Xoá"
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}

/** Facebook Pages: list, add with a manual token, and delete. */
function FacebookAccounts() {
  const accounts = signal<FacebookAccount[]>([]);
  const ready = signal(false);
  const formOpen = signal(false);
  const pageId = signal("");
  const pageName = signal("");
  const token = signal("");
  const formError = signal<string | null>(null);
  const saving = signal(false);
  const removing = signal<FacebookAccount | null>(null);
  const discoverToken = signal("");
  const discovering = signal(false);
  const discovered = signal<DiscoveredFacebookPage[]>([]);
  const selectedPages = signal<Set<string>>(new Set());
  const savingPages = signal(false);

  const load = async () => {
    try {
      await startCore();
      accounts.set((await Facebook.accounts()) as FacebookAccount[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const openForm = () => {
    pageId.set("");
    pageName.set("");
    token.set("");
    discoverToken.set("");
    discovered.set([]);
    selectedPages.set(new Set());
    formError.set(null);
    formOpen.set(true);
  };

  const submit = async () => {
    if (saving()) return;
    const problem = validateFacebook(pageId(), token());
    if (problem) { formError.set(problem); return; }
    formError.set(null);
    saving.set(true);
    try {
      await Facebook.saveAccount({
        pageId: pageId().trim(),
        pageName: pageName().trim() || pageId().trim(),
        accessToken: token().trim(),
      });
      formOpen.set(false);
      notify("Đã lưu trang Facebook", "success");
      await load();
    } catch (e) {
      const f = report(e, { retry: () => { void submit(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      saving.set(false);
    }
  };

  const discover = async () => {
    if (discovering()) return;
    const userToken = facebookTokenFromInput(discoverToken());
    if (userToken === null) {
      formError.set("Cần nhập access token hoặc URL chuyển hướng của Facebook.");
      return;
    }
    formError.set(null);
    discovering.set(true);
    try {
      const pages = (await Facebook.discoverPages(userToken, null)) as DiscoveredFacebookPage[];
      discovered.set(pages);
      selectedPages.set(new Set(pages.map((p) => p.id)));
      if (pages.length === 0) formError.set("Token không trả về trang nào.");
    } catch (e) {
      discovered.set([]);
      const f = report(e, { retry: () => { void discover(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      discovering.set(false);
    }
  };

  const togglePage = (id: string) => {
    const next = new Set(selectedPages());
    if (next.has(id)) next.delete(id); else next.add(id);
    selectedPages.set(next);
  };

  const saveDiscovered = async () => {
    if (savingPages()) return;
    const chosen = discovered().filter((p) => selectedPages().has(p.id));
    if (chosen.length === 0) { formError.set("Chọn ít nhất một trang để lưu."); return; }
    formError.set(null);
    savingPages.set(true);
    try {
      for (const page of chosen) {
        await Facebook.saveAccount({
          pageId: page.id,
          pageName: page.name,
          avatarUrl: page.avatarUrl,
          accessToken: page.accessToken,
          grantedScopes: page.permissions,
        });
      }
      formOpen.set(false);
      notify("Đã lưu trang Facebook", "success");
      await load();
    } catch (e) {
      const f = report(e, { retry: () => { void saveDiscovered(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      savingPages.set(false);
    }
  };

  const confirmRemove = async () => {
    const account = removing();
    removing.set(null);
    if (!account) return;
    try {
      await Facebook.deleteAccount(account.pageId);
      notify("Đã xoá trang Facebook", "success");
    } catch (e) {
      report(e, { retry: () => { removing.set(account); } });
    }
    await load();
  };

  void load();

  return (
    <View class="gap-3">
      <View class="flex-row items-center justify-between">
        <View class="gap-1">
          <Text class="text-base font-semibold text-foreground">Trang Facebook</Text>
          <Text class="text-xs text-muted-foreground">Kết nối bằng Page ID và access token; RustSale kiểm tra token với Meta.</Text>
        </View>
        <View class="flex-row gap-2">
          <Button label="Làm mới" onPress={() => { void load(); }} />
          <Button testID="settings-fb-add" variant="primary" label="Thêm trang" onPress={openForm} />
        </View>
      </View>

      <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && accounts().length === 0}>
        <EmptyState title="Chưa có trang Facebook" hint="Bấm “Thêm trang” và dán Page ID cùng access token." />
      </If>
      <View class="gap-2">
        <Each of={accounts()} key={(a) => a.pageId}>
          {(a) => (
            <View testID={`settings-fb-${a().pageId}`} class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center gap-2">
                <View class="flex-1 gap-1">
                  <Text class="text-sm font-medium text-foreground">{a().pageName?.trim() || a().pageId}</Text>
                  <Text class="text-xs text-muted-foreground">{`ID: ${a().pageId}`}</Text>
                </View>
                <Badge label={tokenLabel(a().tokenStatus)} tone={authTone(a().tokenStatus)} />
                <Badge label={runtimeLabel(a().runtimeStatus)} tone={authTone(a().runtimeStatus)} />
              </View>
              <If when={a().lastError !== null}>
                <Text class="text-xs text-destructive">{a().lastError ?? ""}</Text>
              </If>
              <View class="flex-row justify-end gap-2">
                <Button testID={`settings-fb-del-${a().pageId}`} size="sm" variant="destructive" label="Xoá"
                  onPress={() => removing.set(a())} />
              </View>
            </View>
          )}
        </Each>
      </View>

      <Modal open={formOpen()} title="Thêm trang Facebook" onClose={() => formOpen.set(false)}
        class="m-auto w-[620px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="gap-3">
          <View class="gap-2 rounded-(--radius) border border-border p-3">
            <Text class="text-sm font-medium text-foreground">Tìm trang từ token</Text>
            <Text class="text-xs text-muted-foreground">
              Dán access token của người dùng hoặc URL chuyển hướng sau khi đăng nhập Meta.
            </Text>
            <BaseInput testID="settings-fb-discover-token" masked value={discoverToken()}
              placeholder="Access token hoặc URL chuyển hướng"
              onChange={({ value }) => discoverToken.set(value)}
              class="h-9 rounded-(--radius) border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring" />
            <View class="flex-row justify-end">
              <Button testID="settings-fb-discover" size="sm" label="Tìm trang" disabled={discovering()}
                onPress={() => { void discover(); }} />
            </View>
            <If when={discovered().length > 0}>
              <View class="gap-1">
                <Each of={discovered()} key={(p) => p.id}>
                  {(p) => (
                    <Checkbox testID={`settings-fb-page-${p().id}`} checked={selectedPages().has(p().id)}
                      label={`${p().name} · ${p().id}`} onChange={() => togglePage(p().id)} />
                  )}
                </Each>
              </View>
              <View class="flex-row justify-end">
                <Button testID="settings-fb-save-pages" variant="primary" size="sm" label="Lưu trang đã chọn"
                  disabled={savingPages()} onPress={() => { void saveDiscovered(); }} />
              </View>
            </If>
          </View>
          <Text class="text-xs text-muted-foreground">Hoặc thêm thủ công:</Text>
          <Input testID="settings-fb-page" value={pageId()} placeholder="Page ID" onValue={(v) => pageId.set(v)} />
          <Input testID="settings-fb-name" value={pageName()} placeholder="Tên trang (không bắt buộc)"
            onValue={(v) => pageName.set(v)} />
          <BaseInput testID="settings-fb-token" masked value={token()} placeholder="Access token"
            onChange={({ value }) => token.set(value)}
            class="h-9 rounded-(--radius) border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring" />
          <Text class="text-xs text-muted-foreground">Cần mạng để Meta kiểm tra token trước khi lưu.</Text>
          <If when={formError() !== null}>
            <Text testID="settings-fb-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => formOpen.set(false)} />
            <Button testID="settings-fb-save" variant="primary" label="Lưu" disabled={saving()}
              onPress={() => { void submit(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá trang Facebook?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.pageName ?? removing()?.pageId ?? ""}” sẽ bị ngắt khỏi RustSale.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="settings-fb-confirm-del" variant="destructive" label="Xoá"
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}

/** WhatsApp accounts: list, QR pairing dialog and disconnect. */
function WhatsAppAccounts() {
  const accounts = signal<WhatsAppAccount[]>([]);
  const ready = signal(false);
  const qrOpen = signal(false);
  const qrStarting = signal(false);
  const qrRaw = signal<string | null>(null);
  const qrImage = signal<string | null>(null);
  const qrSequence = signal(0);
  const qrSeconds = signal(0);
  const qrError = signal<string | null>(null);
  const qrStatus = signal("Đang kết nối WhatsApp…");
  const attemptId = signal<string | null>(null);
  const removing = signal<WhatsAppAccount | null>(null);

  const load = async () => {
    try {
      await startCore();
      accounts.set((await ConversationNetwork.whatsappAccounts()) as WhatsAppAccount[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const startLogin = async () => {
    if (qrStarting()) return;
    const attempt = `wa-settings-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    attemptId.set(attempt);
    qrOpen.set(true);
    qrRaw.set(null);
    qrImage.set(null);
    qrSequence.set(0);
    qrSeconds.set(0);
    qrError.set(null);
    qrStatus.set("Đang kết nối WhatsApp…");
    qrStarting.set(true);
    try {
      await ConversationNetwork.startWhatsappQr(attempt);
    } catch (e) {
      qrError.set("Không tạo được mã QR WhatsApp. Thử lại.");
      report(e, { retry: () => { void startLogin(); } });
    } finally {
      qrStarting.set(false);
    }
  };

  const closeQr = () => {
    const attempt = attemptId();
    qrOpen.set(false);
    attemptId.set(null);
    qrRaw.set(null);
    qrImage.set(null);
    if (attempt) void ConversationNetwork.cancelWhatsappQr(attempt).catch(() => undefined);
  };

  const confirmRemove = async () => {
    const account = removing();
    removing.set(null);
    if (!account) return;
    try {
      await ConversationNetwork.disconnectWhatsappAccount(account.id);
      notify("Đã ngắt tài khoản WhatsApp", "success");
    } catch (e) {
      report(e, { retry: () => { removing.set(account); } });
    }
    await load();
  };

  const stop = Host.onEvent((name, payload) => {
    const attempt = attemptId();
    if (!attempt || !qrOpen()) return;
    if (name === "whatsapp-qr-code") {
      const p = payload as { attempt_id?: string; qr_raw?: string; expires_in_sec?: number; sequence?: number };
      if (p.attempt_id && p.attempt_id !== attempt) return;
      qrRaw.set(p.qr_raw ?? null);
      if (p.qr_raw) {
        // The core sends the code's content; the native module draws it so TSX carries no QR encoder.
        try {
          qrImage.set(ConversationNetwork.whatsappQrImage(p.qr_raw) as string);
        } catch {
          qrImage.set(null);
        }
      } else {
        qrImage.set(null);
      }
      qrSeconds.set(p.expires_in_sec ?? 0);
      qrSequence.set(p.sequence ?? 0);
      qrStatus.set("Mở WhatsApp → Thiết bị liên kết → Liên kết thiết bị, rồi quét mã.");
    } else if (name === "whatsapp-login-success") {
      const p = payload as { attempt_id?: string };
      if (p.attempt_id && p.attempt_id !== attempt) return;
      closeQr();
      notify("Đã kết nối WhatsApp", "success");
      void load();
    } else if (name === "whatsapp-login-error") {
      const p = payload as { attempt_id?: string };
      if (p.attempt_id && p.attempt_id !== attempt) return;
      qrError.set("Kết nối WhatsApp thất bại. Thử lại.");
      qrStatus.set("");
    }
  });
  cleanup(stop);

  void load();

  return (
    <View class="gap-3">
      <View class="flex-row items-center justify-between">
        <View class="gap-1">
          <Text class="text-base font-semibold text-foreground">Tài khoản WhatsApp</Text>
          <Text class="text-xs text-muted-foreground">Quét mã một lần để nhận và gửi tin qua WhatsApp.</Text>
        </View>
        <View class="flex-row gap-2">
          <Button label="Làm mới" onPress={() => { void load(); }} />
          <Button testID="settings-wa-add" variant="primary" label="Thêm tài khoản WhatsApp"
            onPress={() => { void startLogin(); }} />
        </View>
      </View>

      <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && accounts().length === 0}>
        <EmptyState title="Chưa có tài khoản WhatsApp" hint="Bấm “Thêm tài khoản WhatsApp” rồi quét mã trên điện thoại." />
      </If>
      <View class="gap-2">
        <Each of={accounts()} key={(a) => String(a.id)}>
          {(a) => (
            <View testID={`settings-wa-${a().id}`} class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center gap-2">
                <View class="flex-1 gap-1">
                  <Text class="text-sm font-medium text-foreground">
                    {a().display_name?.trim() || a().phone_number?.trim() || a().provider_account_key}
                  </Text>
                  <Text class="font-mono text-xs text-muted-foreground">{a().phone_number ?? a().provider_account_key}</Text>
                </View>
                <Badge label="Đã kết nối" tone="success" />
              </View>
              <View class="flex-row justify-end">
                <Button testID={`settings-wa-del-${a().id}`} size="sm" variant="destructive" label="Ngắt kết nối"
                  onPress={() => removing.set(a())} />
              </View>
            </View>
          )}
        </Each>
      </View>

      <Modal open={qrOpen()} title="Kết nối WhatsApp" onClose={closeQr}>
        <View testID="settings-wa-qr" class="items-center gap-3">
          <If when={qrError() !== null}>
            <Text testID="settings-wa-qr-error" class="text-sm text-destructive">{qrError() ?? ""}</Text>
          </If>
          <If when={qrImage() !== null}>
            <Image testID="settings-wa-qr-image" src={qrImage() ?? ""}
              class="h-56 w-56 rounded-(--radius) border border-border bg-background p-2" />
          </If>
          <If when={qrImage() === null && qrRaw() !== null}>
            <View class="w-[360px] gap-1 rounded-(--radius) border border-border bg-background p-3">
              <Text class="text-xs font-medium text-foreground">Nội dung mã QR</Text>
              <Text testID="settings-wa-qr-raw" class="text-xs font-mono text-muted-foreground">{qrRaw() ?? ""}</Text>
            </View>
          </If>
          <If when={qrRaw() !== null}>
            <Text class="text-xs text-muted-foreground">{`Mã #${qrSequence()} · còn ${qrSeconds()} giây`}</Text>
          </If>
          <Text class="text-sm text-muted-foreground">{qrStatus()}</Text>
          <View class="flex-row justify-end">
            <Button label="Đóng" onPress={closeQr} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Ngắt tài khoản WhatsApp?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">
            {`“${removing()?.display_name ?? removing()?.phone_number ?? removing()?.provider_account_key ?? ""}” sẽ ngừng nhận và gửi tin.`}
          </Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="settings-wa-confirm-del" variant="destructive" label="Ngắt"
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}

/** Settings, part B: the channel accounts, one tab per channel. */
function AccountsSettings() {
  const channel = signal<"zalo" | "email" | "facebook" | "whatsapp">("zalo");
  const TABS = [
    { id: "zalo", label: "Zalo" },
    { id: "email", label: "Email" },
    { id: "facebook", label: "Facebook" },
    { id: "whatsapp", label: "WhatsApp" },
  ] as const;

  return (
    <View class="gap-4">
      <View class="flex-row gap-2">
        <Each of={TABS} key={(t) => t.id}>
          {(t) => (
            <Base testID={`settings-tab-${t().id}`} accessibilityLabel={t().label}
              onPress={() => channel.set(t().id)}
              class={`rounded-full border px-3 py-1 ${channel() === t().id ? "border-primary bg-primary" : "border-border bg-card"}`}>
              <Text class={`text-sm ${channel() === t().id ? "text-primary-foreground" : "text-foreground"}`}>{t().label}</Text>
            </Base>
          )}
        </Each>
      </View>
      <If when={channel() === "zalo"}><ZaloAccounts /></If>
      <If when={channel() === "email"}><SmtpAccounts /></If>
      <If when={channel() === "facebook"}><FacebookAccounts /></If>
      <If when={channel() === "whatsapp"}><WhatsAppAccounts /></If>
    </View>
  );
}

/** Settings: notifications and currency on one tab, channel accounts on the other. */
export function SettingsScreen() {
  const tab = signal<"general" | "accounts">("general");
  return (
    <View class="flex-1">
      <View class="flex-row items-center justify-between px-6 pt-6">
        <Text class="text-xl font-semibold text-foreground">Cài đặt</Text>
        <View class="flex-row gap-2">
          <Base testID="settings-tab-general" accessibilityLabel="Chung"
            onPress={() => tab.set("general")}
            class={`rounded-full border px-3 py-1 ${tab() === "general" ? "border-primary bg-primary" : "border-border bg-card"}`}>
            <Text class={`text-sm ${tab() === "general" ? "text-primary-foreground" : "text-foreground"}`}>Chung</Text>
          </Base>
          <Base testID="settings-tab-accounts" accessibilityLabel="Tài khoản kênh"
            onPress={() => tab.set("accounts")}
            class={`rounded-full border px-3 py-1 ${tab() === "accounts" ? "border-primary bg-primary" : "border-border bg-card"}`}>
            <Text class={`text-sm ${tab() === "accounts" ? "text-primary-foreground" : "text-foreground"}`}>Tài khoản kênh</Text>
          </Base>
        </View>
      </View>
      <Scroll class="flex-1">
        <If when={tab() === "general"}><GeneralSettings /></If>
        <If when={tab() === "accounts"}><AccountsSettings /></If>
      </Scroll>
    </View>
  );
}
