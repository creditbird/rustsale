import { cleanup, signal, Text, View, Scroll } from "nsh";
import { Button as Base, Progress } from "nsh/base";
import { Campaigns, Templates } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { notify, report } from "../stores/toast";
import { startCore } from "../stores/core";
import { useOutreachLeads } from "../stores/outreach";
import { emailCampaign } from "../stores/campaign";
import { OutreachLeads } from "./OutreachLeads";

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

interface EmailTemplate {
  id: number;
  name: string;
  subject: string;
  content: string;
  created_at: string;
}

/** Email outreach: SMTP accounts, email templates, lead selection, and a campaign with progress and logs. */
export function EmailOutreachScreen() {
  const smtp = signal<SmtpAccount[]>([]);
  const smtpReady = signal(false);
  const chosenSmtp = signal<number | null>(null);

  const templates = signal<EmailTemplate[]>([]);
  const templatesReady = signal(false);
  const chosenTemplate = signal<number | null>(null);
  const editing = signal<number | "new" | null>(null);
  const formName = signal("");
  const formSubject = signal("");
  const formBody = signal("");
  const formError = signal<string | null>(null);
  const saving = signal(false);
  const removing = signal<EmailTemplate | null>(null);
  const deleting = signal(false);

  const campaign = emailCampaign;
  const { starting, progress, status, logs } = campaign;
  const runError = signal<string | null>(null);

  const pick = useOutreachLeads("email");

  const loadSmtp = async () => {
    try {
      await startCore();
      const rows = ((await Templates.smtpAccounts()) as SmtpAccount[])
        .filter((row) => typeof row.id === "number");
      smtp.set(rows);
      smtpReady.set(true);
      const current = chosenSmtp();
      if (rows.length > 0 && (current === null || !rows.some((row) => row.id === current))) {
        chosenSmtp.set(rows[0].id);
      }
      if (rows.length === 0) chosenSmtp.set(null);
    } catch (e) {
      report(e, { retry: () => { void loadSmtp(); } });
    }
  };
  const loadTemplates = async () => {
    try {
      await startCore();
      const rows = ((await Templates.emailTemplates()) as EmailTemplate[])
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

  const openCreate = () => {
    formName.set("");
    formSubject.set("");
    formBody.set("");
    formError.set(null);
    editing.set("new");
  };
  const openEdit = (template: EmailTemplate) => {
    formName.set(template.name);
    formSubject.set(template.subject);
    formBody.set(template.content);
    formError.set(null);
    editing.set(template.id);
  };
  const submitTemplate = async () => {
    if (saving()) return;
    if (formName().trim() === "") { formError.set("Cần nhập tên mẫu."); return; }
    if (formSubject().trim() === "") { formError.set("Cần nhập tiêu đề email."); return; }
    if (formBody().trim() === "") { formError.set("Cần nhập nội dung email."); return; }
    const id = editing();
    saving.set(true);
    try {
      if (typeof id === "number") {
        await Templates.updateEmailTemplate(id, formName().trim(), formSubject().trim(), formBody().trim());
      } else {
        await Templates.createEmailTemplate(formName().trim(), formSubject().trim(), formBody().trim());
      }
      editing.set(null);
      notify("Đã lưu mẫu email", "success");
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
      await Templates.deleteEmailTemplate(template.id);
      notify("Đã xoá mẫu email", "success");
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

  const run = async () => {
    if (starting()) return;
    runError.set(null);
    if (smtp().length === 0) {
      runError.set("Chưa có tài khoản SMTP. Vào Cài đặt để thêm tài khoản gửi trước khi chạy.");
      return;
    }
    const smtpId = chosenSmtp();
    if (smtpId === null) { runError.set("Chọn một tài khoản SMTP để gửi."); return; }
    const template = chosenTemplate();
    if (template === null) { runError.set("Chọn một mẫu email để gửi."); return; }
    if (pick.chosenCount() === 0) { runError.set("Chọn ít nhất một lead để gửi."); return; }
    campaign.begin();
    try {
      const ids = await pick.resolve();
      if (ids.length === 0) {
        runError.set("Không có lead nào phù hợp để gửi.");
        campaign.abort();
        return;
      }
      await Campaigns.sendEmailOutreach(smtpId, template, ids);
      status.set(`Đang gửi tới ${ids.length} lead…`);
      campaign.appendLog(`Bắt đầu gửi email tới ${ids.length} lead.`);
      notify(`Đã bắt đầu gửi tới ${ids.length} lead`, "success");
    } catch (e) {
      const f = report(e, { retry: () => { void run(); } });
      if (f.action === "fix") runError.set(f.message);
      campaign.abort();
    }
  };

  // The campaign state outlives the screen; only the lead list needs a re-read when a run ends.
  cleanup(campaign.onDone(() => { void pick.list.reload(); }));

  void loadSmtp();
  void loadTemplates();

  return (
    <View class="flex-1 flex-row">
      <View class="w-[400px] gap-3 border-r border-border p-4">
        <View class="gap-0.5">
          <Text class="text-lg font-semibold text-foreground">Email Outreach</Text>
          <Text class="text-xs text-muted-foreground">Chọn tài khoản gửi, mẫu email và lead rồi chạy chiến dịch.</Text>
        </View>

        <Scroll class="flex-1">
          <View class="gap-3">
            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <Text class="text-xs font-medium text-muted-foreground">Tài khoản SMTP</Text>
              <If when={!smtpReady()}><Skeleton /></If>
              <If when={smtpReady() && smtp().length === 0}>
                <Text testID="email-smtp-empty" class="text-sm text-muted-foreground">
                  Chưa có tài khoản SMTP. Vào Cài đặt để thêm tài khoản gửi.
                </Text>
              </If>
              <Each of={smtp()} key={(account) => String(account.id)}>
                {(account) => (
                  <Base testID={`email-smtp-${account().id}`} onPress={() => chosenSmtp.set(account().id)}
                    class={`flex-row items-center gap-2 rounded-(--radius) border px-2 py-1.5 ${chosenSmtp() === account().id ? "border-primary bg-primary/10" : "border-border bg-background"}`}>
                    <View class="flex-1 gap-0.5">
                      <Text class="truncate text-sm text-foreground">{account().name}</Text>
                      <Text class="truncate text-xs text-muted-foreground">
                        {`${account().host}:${account().port} · ${account().from_email || "—"}`}
                      </Text>
                    </View>
                    <Badge label={account().security || "SMTP"} tone="neutral" />
                  </Base>
                )}
              </Each>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <View class="flex-row items-center justify-between">
                <Text class="text-xs font-medium text-muted-foreground">Mẫu email</Text>
                <Button testID="email-tpl-add" label="Thêm mẫu" size="sm" variant="primary" onPress={openCreate} />
              </View>
              <If when={!templatesReady()}><Skeleton /></If>
              <If when={templatesReady() && templates().length === 0}>
                <EmptyState title="Chưa có mẫu email" hint="Bấm “Thêm mẫu” để tạo mẫu đầu tiên." />
              </If>
              <Each of={templates()} key={(template) => String(template.id)}>
                {(template) => (
                  <View class="flex-row items-center gap-1">
                    <Button testID={`email-tpl-${template().id}`} label={template().name} size="sm"
                      variant={chosenTemplate() === template().id ? "primary" : "secondary"}
                      onPress={() => chosenTemplate.set(template().id)} />
                    <Button testID={`email-tpl-edit-${template().id}`} label="Sửa" size="sm" variant="ghost"
                      onPress={() => openEdit(template())} />
                    <Button testID={`email-tpl-del-${template().id}`} label="Xoá" size="sm" variant="ghost"
                      onPress={() => removing.set(template())} />
                  </View>
                )}
              </Each>
            </View>
          </View>
        </Scroll>

        <View class="gap-2">
          <If when={runError() !== null}>
            <Text testID="email-run-error" class="text-sm text-destructive">{runError() ?? ""}</Text>
          </If>
          <Button testID="email-run" label={starting() ? "Đang gửi…" : "Chạy chiến dịch"} variant="primary"
            disabled={starting()} onPress={() => { void run(); }} />
          <If when={starting() || progress() > 0}>
            <Progress testID="email-progress" value={progress()} accessibilityLabel="Tiến độ gửi email"
              trackClass="h-2 rounded-full bg-muted" indicatorClass="h-2 rounded-full bg-primary" />
          </If>
          <Text testID="email-status" class="text-xs text-muted-foreground">{status() || "Chưa chạy"}</Text>
          <View testID="email-log" class="gap-0.5">
            <Each of={logs().slice(-4)} key={(line) => String(line.id)}>
              {(line) => <Text class="truncate text-xs text-muted-foreground">{line().text}</Text>}
            </Each>
          </View>
        </View>
      </View>

      <View class="flex-1 gap-3 p-5">
        <Text class="text-xl font-semibold text-foreground">Chọn lead nhận email</Text>
        <OutreachLeads pick={pick} prefix="email" zalo={false} templates={templates()} />
      </View>

      <Modal open={editing() !== null} title={editing() === "new" ? "Thêm mẫu email" : "Sửa mẫu email"}
        onClose={() => editing.set(null)}>
        <View class="gap-3">
          <Input testID="email-tpl-name" value={formName()} placeholder="Tên mẫu" onValue={(v) => formName.set(v)} />
          <Input testID="email-tpl-subject" value={formSubject()} placeholder="Tiêu đề email" onValue={(v) => formSubject.set(v)} />
          <Input testID="email-tpl-body" value={formBody()} placeholder="Nội dung email" onValue={(v) => formBody.set(v)} />
          <If when={formError() !== null}>
            <Text testID="email-tpl-form-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => editing.set(null)} />
            <Button testID="email-tpl-save" label="Lưu" variant="primary" disabled={saving()}
              onPress={() => { void submitTemplate(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá mẫu email?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.name ?? ""}” sẽ bị xoá.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="email-tpl-confirm-del" label="Xoá" variant="destructive" disabled={deleting()}
              onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
