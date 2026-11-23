import { cleanup, signal, Text, View, Scroll } from "nsh";
import { Button as Base, Progress } from "nsh/base";
import { Campaigns, Host, Zalo } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { notify, report } from "../stores/toast";
import { startCore } from "../stores/core";
import { useOutreachLeads } from "../stores/outreach";
import { OutreachLeads } from "./OutreachLeads";

interface ZaloAccount {
  account_key: string;
  display_name: string | null;
  auth_status: string;
  runtime_status: string;
  last_error: string | null;
}

interface OutreachTemplate {
  id: number;
  name: string;
  content: string;
  image_url: string | null;
  created_at: string;
}

interface LogLine {
  id: number;
  text: string;
}

interface ProgressEvent { progress?: number; status?: string; message?: string }
interface LogEvent { message?: string }
interface CompleteEvent { total_sent?: number; message?: string }

const MAX_LOGS = 50;

function accountLabel(account: ZaloAccount): string {
  const name = account.display_name?.trim();
  return name && name !== "" ? name : account.account_key;
}

function runtimeLabel(status: string): string {
  if (status === "connected") return "Đã kết nối";
  if (status === "disconnected") return "Chưa kết nối";
  return status;
}

/** Zalo outreach: accounts, templates, lead selection, delay and a campaign that reports through events. */
export function OutreachScreen() {
  const accounts = signal<ZaloAccount[]>([]);
  const accountsReady = signal(false);
  const chosenAccounts = signal<string[]>([]);
  const templates = signal<OutreachTemplate[]>([]);
  const templatesReady = signal(false);
  const chosenTemplate = signal<number | null>(null);
  const editing = signal<number | "new" | null>(null);
  const formName = signal("");
  const formBody = signal("");
  const formImage = signal("");
  const formError = signal<string | null>(null);
  const saving = signal(false);
  const removing = signal<OutreachTemplate | null>(null);
  const deleting = signal(false);

  const starting = signal(false);
  const runError = signal<string | null>(null);
  const progress = signal(0);
  const status = signal("");
  const logs = signal<LogLine[]>([]);
  let logSeq = 0;

  const interDelay = signal("3");
  const burstEnabled = signal(true);
  const burstMax = signal("5");
  const burstDelay = signal("30");

  const pick = useOutreachLeads("zalo");

  const appendLog = (text: string) => {
    logSeq += 1;
    const next = logs().concat([{ id: logSeq, text }]);
    logs.set(next.length > MAX_LOGS ? next.slice(next.length - MAX_LOGS) : next);
  };

  const loadAccounts = async () => {
    try {
      await startCore();
      const rows = (await Zalo.accounts()) as ZaloAccount[];
      accounts.set(rows);
      accountsReady.set(true);
      if (rows.length > 0 && chosenAccounts().length === 0) {
        chosenAccounts.set([rows[0].account_key]);
      }
    } catch (e) {
      report(e, { retry: () => { void loadAccounts(); } });
    }
  };
  const loadTemplates = async () => {
    try {
      await startCore();
      const rows = ((await Campaigns.outreachTemplates()) as OutreachTemplate[])
        .filter((row) => typeof row.id === "number");
      templates.set(rows);
      templatesReady.set(true);
      const current = chosenTemplate();
      if (rows.length > 0 && (current === null || !rows.some((row) => row.id === current))) {
        chosenTemplate.set(rows[0].id);
      }
      if (rows.length === 0) chosenTemplate.set(null);
    } catch (e) {
      report(e, { retry: () => { void loadTemplates(); } });
    }
  };

  const toggleAccount = (key: string) => {
    const current = chosenAccounts();
    chosenAccounts.set(current.includes(key) ? current.filter((x) => x !== key) : current.concat([key]));
  };

  const openCreate = () => {
    formName.set("");
    formBody.set("");
    formImage.set("");
    formError.set(null);
    editing.set("new");
  };
  const openEdit = (template: OutreachTemplate) => {
    formName.set(template.name);
    formBody.set(template.content);
    formImage.set(template.image_url ?? "");
    formError.set(null);
    editing.set(template.id);
  };
  const submitTemplate = async () => {
    if (saving()) return;
    if (formName().trim() === "") { formError.set("Cần nhập tên mẫu."); return; }
    if (formBody().trim() === "") { formError.set("Cần nhập nội dung mẫu."); return; }
    const id = editing();
    const image = formImage().trim() || null;
    saving.set(true);
    try {
      if (typeof id === "number") {
        await Campaigns.updateOutreachTemplate(id, formName().trim(), formBody().trim(), image);
      } else {
        await Campaigns.createOutreachTemplate(formName().trim(), formBody().trim(), image);
      }
      editing.set(null);
      notify("Đã lưu mẫu", "success");
      await loadTemplates();
    } catch (e) {
      const f = report(e, {
        refresh: () => { editing.set(null); void loadTemplates(); },
        retry: () => { void submitTemplate(); },
      });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      saving.set(false);
    }
  };
  const confirmRemove = async () => {
    const template = removing();
    removing.set(null);
    if (!template || deleting()) return;
    deleting.set(true);
    try {
      await Campaigns.deleteOutreachTemplate(template.id);
      notify("Đã xoá mẫu", "success");
      await loadTemplates();
    } catch (e) {
      report(e, {
        refresh: () => { void loadTemplates(); },
        retry: () => { removing.set(template); },
      });
    } finally {
      deleting.set(false);
    }
  };

  interface Delays { inter: number; burstMax: number; burstDelay: number }
  const parseDelays = (): Delays | string => {
    const whole = (value: string) => /^\d+$/.test(value.trim());
    if (!whole(interDelay())) return "Delay giữa các tin phải là số nguyên không âm.";
    const delays: Delays = {
      inter: Number(interDelay().trim()),
      burstMax: 0,
      burstDelay: 0,
    };
    if (burstEnabled()) {
      if (!whole(burstMax())) return "Số tin mỗi đợt phải là số nguyên không âm.";
      if (!whole(burstDelay())) return "Nghỉ giữa các đợt phải là số nguyên không âm.";
      delays.burstMax = Number(burstMax().trim());
      delays.burstDelay = Number(burstDelay().trim());
    }
    return delays;
  };

  const run = async () => {
    if (starting()) return;
    runError.set(null);
    if (accounts().length === 0) {
      runError.set("Chưa có tài khoản Zalo. Vào Cài đặt để kết nối tài khoản trước khi chạy.");
      return;
    }
    if (chosenAccounts().length === 0) {
      runError.set("Chọn ít nhất một tài khoản Zalo để chạy.");
      return;
    }
    const template = chosenTemplate();
    if (template === null) {
      runError.set("Chọn một mẫu tin để chạy chiến dịch.");
      return;
    }
    if (pick.chosenCount() === 0) {
      runError.set("Chọn ít nhất một lead để chạy chiến dịch.");
      return;
    }
    const delays = parseDelays();
    if (typeof delays === "string") {
      runError.set(delays);
      return;
    }
    starting.set(true);
    progress.set(0);
    status.set("Đang chuẩn bị…");
    logs.set([]);
    try {
      const ids = await pick.resolve();
      if (ids.length === 0) {
        runError.set("Không có lead nào phù hợp để gửi.");
        starting.set(false);
        status.set("");
        return;
      }
      await Campaigns.startZaloOutreach(
        chosenAccounts(), template, ids,
        burstEnabled(), delays.burstMax, delays.burstDelay, delays.inter,
      );
      status.set(`Đang gửi tới ${ids.length} lead…`);
      appendLog(`Bắt đầu gửi tới ${ids.length} lead.`);
      notify(`Đã bắt đầu gửi tới ${ids.length} lead`, "success");
    } catch (e) {
      const f = report(e, { retry: () => { void run(); } });
      if (f.action === "fix") runError.set(f.message);
      starting.set(false);
      status.set("");
    }
  };

  // The core reports the campaign through events; re-read the leads when it ends.
  const stop = Host.onEvent((name, payload) => {
    if (name === "outreach-progress") {
      const event = payload as ProgressEvent;
      if (typeof event.progress === "number") progress.set(event.progress);
      if (typeof event.message === "string" && event.message !== "") appendLog(event.message);
    } else if (name === "outreach-log") {
      const event = payload as LogEvent;
      if (typeof event.message === "string" && event.message !== "") appendLog(event.message);
    } else if (name === "outreach-complete") {
      const event = payload as CompleteEvent;
      const sent = typeof event.total_sent === "number" ? event.total_sent : 0;
      progress.set(100);
      starting.set(false);
      status.set(`Đã gửi ${sent} tin.`);
      appendLog(typeof event.message === "string" ? event.message : `Hoàn tất: ${sent} tin.`);
      notify(`Đã gửi ${sent} tin`, "success");
      void pick.list.reload();
    }
  });
  cleanup(stop);

  void loadAccounts();
  void loadTemplates();

  return (
    <View class="flex-1 flex-row">
      <View class="w-[400px] gap-3 border-r border-border p-4">
        <View class="gap-0.5">
          <Text class="text-lg font-semibold text-foreground">Zalo Outreach</Text>
          <Text class="text-xs text-muted-foreground">Chọn tài khoản, mẫu tin và lead rồi chạy chiến dịch.</Text>
        </View>

        <Scroll class="flex-1">
          <View class="gap-3">
            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <Text class="text-xs font-medium text-muted-foreground">Tài khoản Zalo</Text>
              <If when={!accountsReady()}><Skeleton /></If>
              <If when={accountsReady() && accounts().length === 0}>
                <Text testID="zalo-accounts-empty" class="text-sm text-muted-foreground">
                  Chưa có tài khoản Zalo. Vào Cài đặt để kết nối.
                </Text>
              </If>
              <Each of={accounts()} key={(account) => account.account_key}>
                {(account) => (
                  <Base testID={`zalo-account-${account().account_key}`} onPress={() => toggleAccount(account().account_key)}
                    class={`flex-row items-center gap-2 rounded-(--radius) border px-2 py-1.5 ${chosenAccounts().includes(account().account_key) ? "border-primary bg-primary/10" : "border-border bg-background"}`}>
                    <Text class="truncate flex-1 text-sm text-foreground">{accountLabel(account())}</Text>
                    <Badge label={runtimeLabel(account().runtime_status)}
                      tone={account().runtime_status === "connected" ? "success" : "neutral"} />
                  </Base>
                )}
              </Each>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center justify-between">
                <Text class="text-xs font-medium text-muted-foreground">Mẫu tin</Text>
                <Button testID="zalo-tpl-add" label="Thêm mẫu" size="sm" variant="primary" onPress={openCreate} />
              </View>
              <If when={!templatesReady()}><Skeleton /></If>
              <If when={templatesReady() && templates().length === 0}>
                <EmptyState title="Chưa có mẫu tin" hint="Bấm “Thêm mẫu” để tạo mẫu đầu tiên." />
              </If>
              <Each of={templates()} key={(template) => String(template.id)}>
                {(template) => (
                  <View class="flex-row items-center gap-1">
                    <Button testID={`zalo-tpl-${template().id}`} label={template().name} size="sm"
                      variant={chosenTemplate() === template().id ? "primary" : "secondary"}
                      onPress={() => chosenTemplate.set(template().id)} />
                    <Button testID={`zalo-tpl-edit-${template().id}`} label="Sửa" size="sm" variant="ghost"
                      onPress={() => openEdit(template())} />
                    <Button testID={`zalo-tpl-del-${template().id}`} label="Xoá" size="sm" variant="ghost"
                      onPress={() => removing.set(template())} />
                  </View>
                )}
              </Each>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <Text class="text-xs font-medium text-muted-foreground">Nhịp gửi</Text>
              <View class="flex-row items-center gap-2">
                <Text class="text-sm text-foreground">Delay giữa tin (giây)</Text>
                <View class="w-20"><Input testID="zalo-delay" value={interDelay()} onValue={(v) => interDelay.set(v)} /></View>
              </View>
              <View class="flex-row items-center gap-2">
                <Button testID="zalo-burst-toggle" label={burstEnabled() ? "Giới hạn đợt: bật" : "Giới hạn đợt: tắt"}
                  size="sm" variant={burstEnabled() ? "primary" : "secondary"}
                  onPress={() => burstEnabled.set(!burstEnabled())} />
              </View>
              <If when={burstEnabled()}>
                <View class="flex-row items-center gap-2">
                  <Text class="text-sm text-foreground">Tin mỗi đợt</Text>
                  <View class="w-16"><Input testID="zalo-burst-max" value={burstMax()} onValue={(v) => burstMax.set(v)} /></View>
                  <Text class="text-sm text-foreground">Nghỉ (giây)</Text>
                  <View class="w-16"><Input testID="zalo-burst-delay" value={burstDelay()} onValue={(v) => burstDelay.set(v)} /></View>
                </View>
              </If>
            </View>
          </View>
        </Scroll>

        <View class="gap-2">
          <If when={runError() !== null}>
            <Text testID="zalo-run-error" class="text-sm text-destructive">{runError() ?? ""}</Text>
          </If>
          <Button testID="zalo-run" label={starting() ? "Đang chạy…" : "Chạy chiến dịch"} variant="primary"
            disabled={starting()} onPress={() => { void run(); }} />
          <If when={starting() || progress() > 0}>
            <Progress testID="zalo-progress" value={progress()} accessibilityLabel="Tiến độ gửi Zalo"
              trackClass="h-2 rounded-full bg-muted" indicatorClass="h-2 rounded-full bg-primary" />
          </If>
          <Text testID="zalo-status" class="text-xs text-muted-foreground">{status() || "Chưa chạy"}</Text>
          <View testID="zalo-log" class="gap-0.5">
            <Each of={logs().slice(-4)} key={(line) => String(line.id)}>
              {(line) => <Text class="truncate text-xs text-muted-foreground">{line().text}</Text>}
            </Each>
          </View>
        </View>
      </View>

      <View class="flex-1 gap-3 p-5">
        <Text class="text-xl font-semibold text-foreground">Chọn lead nhận tin</Text>
        <OutreachLeads pick={pick} prefix="zalo" zalo={true} />
      </View>

      <Modal open={editing() !== null} title={editing() === "new" ? "Thêm mẫu tin" : "Sửa mẫu tin"}
        onClose={() => editing.set(null)}>
        <View class="gap-3">
          <Input testID="zalo-tpl-name" value={formName()} placeholder="Tên mẫu" onValue={(v) => formName.set(v)} />
          <Input testID="zalo-tpl-body" value={formBody()} placeholder="Nội dung tin" onValue={(v) => formBody.set(v)} />
          <Input testID="zalo-tpl-image" value={formImage()} placeholder="URL ảnh (không bắt buộc)" onValue={(v) => formImage.set(v)} />
          <If when={formError() !== null}>
            <Text testID="zalo-tpl-form-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => editing.set(null)} />
            <Button testID="zalo-tpl-save" label="Lưu" variant="primary" disabled={saving()}
              onPress={() => { void submitTemplate(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá mẫu tin?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.name ?? ""}” sẽ bị xoá.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="zalo-confirm-del" label="Xoá" variant="destructive" disabled={deleting()}
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
