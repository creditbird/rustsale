import { signal, Text, View } from "nsh";
import { Select } from "nsh/base";
import { Campaigns, LeadsMore, Templates, Zalo } from "rustsale/native";
import { Badge, Button, Input, Modal } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { notify, report } from "../stores/toast";
import { hasValidZaloUid, isVnMobile, parseZaloOutreachResult } from "../stores/zaloTarget";

export interface Lead {
  id: number;
  name: string;
  company: string | null;
  status: string;
  owner: string | null;
  value: number | null;
  currency: string | null;
  email: string | null;
  phone: string | null;
  about: string | null;
  address: string | null;
  last_activity: string | null;
  has_zalo?: number | null;
  zalo_status?: string | null;
  zalo_uid?: string | null;
}

export interface Activity {
  id: number;
  activity_type: string;
  content: string;
  created_at: string;
}

/** The whole lead as the core stores it: `LeadsMore.update` replaces every field. */
export interface LeadEditDraft {
  name: string;
  company: string;
  status: string;
  value: number;
  owner: string;
  email: string;
  phone: string;
  about: string | null;
  address: string | null;
  currency: string | null;
}

export interface Deal {
  id: number;
  lead_id: number;
  title: string;
  value: number;
  stage: string;
  currency: string | null;
  created_at: string;
}

export const STATUSES = ["New", "Contacted", "Qualified", "Proposal", "Won", "Lost", "Converted"];

/** A deal value as people say it: millions as "15 tr", smaller as the plain number. */
export function money(value: number | null, currency: string | null): string {
  if (!value) return "—";
  const unit = currency && currency !== "VND" ? ` ${currency}` : "";
  return value >= 1_000_000 ? `${Math.round(value / 100_000) / 10} tr${unit}` : `${value}${unit}`;
}

export function tone(status: string): BadgeTone {
  if (status === "Won" || status === "Converted" || status === "Qualified") return "success";
  if (status === "Lost") return "danger";
  return "neutral";
}

/** Checks an edit before the core is asked; returns the sentence to show, or null when it is fine. */
export function validateLeadEdit(draft: LeadEditDraft): string | null {
  if (draft.name.trim() === "") return "Cần nhập tên khách.";
  if (!Number.isFinite(draft.value) || draft.value < 0) return "Giá trị phải là số không âm.";
  return null;
}

/** The value typed in the field: blank is 0, thousands separators are allowed, text is NaN. */
export function amountOf(text: string): number {
  const trimmed = text.trim().replace(/,/g, "");
  return trimmed === "" ? 0 : Number(trimmed);
}

export interface LeadDetailProps {
  lead: () => Lead;
  activities: () => Activity[];
  /** The lead's deals; screens that do not load them leave the section empty. */
  deals?: () => Deal[];
  onClose: () => void;
  onStatus: (status: string) => void;
  /** When present, the panel offers the edit form; resolves true once the core saved the lead. */
  onSave?: (draft: LeadEditDraft) => Promise<boolean>;
  /** True while a save is in flight: the form's buttons wait for it. */
  saving?: () => boolean;
  /** The message of a failed save (invalid_request), shown under the fields. */
  saveError?: () => string | null;
  /** When present, the panel offers a one-lead Zalo and email send, and reloads after one. */
  onSent?: () => void;
}

interface ZaloAccount {
  account_key: string;
  display_name: string | null;
  auth_status: string;
}

interface ZaloTemplate {
  id: number;
  name: string;
}

interface SmtpAccount {
  id: number;
  name: string;
  from_email: string;
}

interface EmailTemplate {
  id: number;
  name: string;
  subject: string;
}

const EMAIL_SHAPE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/** The lead detail panel the Leads, Contacts and Categories screens share. */
export function LeadDetail(props: LeadDetailProps) {
  const editing = signal(false);
  const name = signal("");
  const company = signal("");
  const email = signal("");
  const phone = signal("");
  const owner = signal("");
  const value = signal("");
  const about = signal("");
  const address = signal("");
  const localError = signal<string | null>(null);
  const zaloOpen = signal(false);
  const zaloAccounts = signal<ZaloAccount[]>([]);
  const zaloTemplates = signal<ZaloTemplate[]>([]);
  const zaloAccount = signal("");
  const zaloTemplate = signal<number | null>(null);
  const zaloBusy = signal(false);
  const zaloError = signal<string | null>(null);
  const emailOpen = signal(false);
  const smtpAccounts = signal<SmtpAccount[]>([]);
  const emailTemplates = signal<EmailTemplate[]>([]);
  const smtpId = signal<number | null>(null);
  const emailTemplateId = signal<number | null>(null);
  const emailBusy = signal(false);
  const emailError = signal<string | null>(null);

  const zaloLabel = (account: ZaloAccount) => account.display_name || account.account_key;
  const zaloTemplateName = () =>
    zaloTemplates().find((template) => template.id === zaloTemplate())?.name ?? "mẫu Zalo";
  const emailTemplateName = () =>
    emailTemplates().find((template) => template.id === emailTemplateId())?.name ?? "mẫu email";

  const openZalo = async () => {
    const lead = props.lead();
    // Match core `is_valid_zalo_lead_target`: require a valid Zalo UID or a Vietnamese mobile number.
    if (!hasValidZaloUid(lead.zalo_uid)) {
      const rawPhone = (lead.phone ?? "").trim();
      if (rawPhone === "") {
        notify("Khách chưa có số điện thoại hoặc Zalo.", "warning");
        return;
      }
      if (!isVnMobile(rawPhone)) {
        notify("SĐT không hợp lệ, không gửi.", "danger");
        return;
      }
    }
    zaloOpen.set(true);
    zaloError.set(null);
    zaloAccounts.set([]);
    zaloTemplates.set([]);
    try {
      const [accounts, templates] = await Promise.all([
        Zalo.accounts(),
        Campaigns.outreachTemplates(),
      ]);
      const accountRows = accounts as ZaloAccount[];
      const templateRows = templates as ZaloTemplate[];
      zaloAccounts.set(accountRows);
      zaloTemplates.set(templateRows);
      const valid = accountRows.find((account) => account.auth_status === "valid") ?? accountRows[0];
      zaloAccount.set(valid?.account_key ?? "");
      zaloTemplate.set(templateRows[0]?.id ?? null);
    } catch (e) {
      report(e, { retry: () => { void openZalo(); } });
    }
  };
  const closeZalo = () => {
    if (!zaloBusy()) zaloOpen.set(false);
  };
  const sendZalo = async () => {
    if (zaloBusy()) return;
    if (zaloAccount() === "") {
      zaloError.set("Chưa có tài khoản Zalo đã đăng nhập.");
      return;
    }
    if (zaloTemplate() === null) {
      zaloError.set("Chưa có mẫu tin Zalo.");
      return;
    }
    const lead = props.lead();
    zaloBusy.set(true);
    zaloError.set(null);
    try {
      const result = parseZaloOutreachResult(
        await Campaigns.startZaloOutreach([zaloAccount()], zaloTemplate(), [lead.id]),
      );
      if (result.queued <= 0) {
        const msg = "SĐT không hợp lệ, không gửi.";
        zaloError.set(msg);
        notify(msg, "danger");
        return;
      }
      // The send is queued from here on: a failed log must never offer a retry that sends it twice.
      try {
        await LeadsMore.addActivity({
          leadId: lead.id,
          activityType: "Zalo",
          content: `Đã gửi Zalo: ${zaloTemplateName()}`,
          performedBy: "Self",
        });
      } catch {
        notify("Tin Zalo đã vào hàng gửi nhưng chưa ghi được hoạt động.", "warning");
      }
      zaloOpen.set(false);
      if (result.skipped > 0) {
        notify(`Đã xếp ${result.queued}, bỏ ${result.skipped}`, "warning");
      } else {
        notify(`Đã xếp hàng gửi Zalo cho ${lead.name}`, "success");
      }
      props.onSent?.();
    } catch (e) {
      const f = report(e, { retry: () => { void sendZalo(); } });
      if (f.action === "fix") zaloError.set(f.message);
    } finally {
      zaloBusy.set(false);
    }
  };

  const openEmail = async () => {
    const lead = props.lead();
    if (lead.email === null || !EMAIL_SHAPE.test(lead.email.trim())) {
      notify("Khách chưa có email hợp lệ.", "warning");
      return;
    }
    emailOpen.set(true);
    emailError.set(null);
    smtpAccounts.set([]);
    emailTemplates.set([]);
    try {
      const [accounts, templates] = await Promise.all([
        Templates.smtpAccounts(),
        Templates.emailTemplates(),
      ]);
      const smtpRows = accounts as SmtpAccount[];
      const templateRows = templates as EmailTemplate[];
      smtpAccounts.set(smtpRows);
      emailTemplates.set(templateRows);
      smtpId.set(smtpRows[0]?.id ?? null);
      emailTemplateId.set(templateRows[0]?.id ?? null);
    } catch (e) {
      report(e, { retry: () => { void openEmail(); } });
    }
  };
  const closeEmail = () => {
    if (!emailBusy()) emailOpen.set(false);
  };
  const sendEmail = async () => {
    if (emailBusy()) return;
    if (smtpId() === null) {
      emailError.set("Chưa có tài khoản SMTP.");
      return;
    }
    if (emailTemplateId() === null) {
      emailError.set("Chưa có mẫu email.");
      return;
    }
    const lead = props.lead();
    emailBusy.set(true);
    emailError.set(null);
    try {
      await Campaigns.sendEmailOutreach(smtpId()!, emailTemplateId()!, [lead.id]);
      // The core only queues the email (it answers before SMTP runs); a failed log must not offer a second send.
      try {
        await LeadsMore.addActivity({
          leadId: lead.id,
          activityType: "Email",
          content: `Đã gửi email: ${emailTemplateName()}`,
          performedBy: "Self",
        });
      } catch {
        notify("Email đã vào hàng gửi nhưng chưa ghi được hoạt động.", "warning");
      }
      emailOpen.set(false);
      notify(`Đã xếp hàng gửi email cho ${lead.name}`, "success");
      props.onSent?.();
    } catch (e) {
      const f = report(e, { retry: () => { void sendEmail(); } });
      if (f.action === "fix") emailError.set(f.message);
    } finally {
      emailBusy.set(false);
    }
  };

  const startEdit = () => {
    const lead = props.lead();
    name.set(lead.name);
    company.set(lead.company ?? "");
    email.set(lead.email ?? "");
    phone.set(lead.phone ?? "");
    owner.set(lead.owner ?? "");
    value.set(lead.value === null ? "" : String(lead.value));
    about.set(lead.about ?? "");
    address.set(lead.address ?? "");
    localError.set(null);
    editing.set(true);
  };

  const stopEdit = () => {
    editing.set(false);
    localError.set(null);
  };

  const save = async () => {
    if (props.saving?.() === true) return;
    const lead = props.lead();
    const draft: LeadEditDraft = {
      name: name().trim(),
      company: company().trim(),
      status: lead.status,
      value: amountOf(value()),
      owner: owner().trim(),
      email: email().trim(),
      phone: phone().trim(),
      about: about().trim() === "" ? null : about().trim(),
      address: address().trim() === "" ? null : address().trim(),
      currency: lead.currency,
    };
    const problem = validateLeadEdit(draft);
    if (problem !== null) {
      localError.set(problem);
      return;
    }
    localError.set(null);
    if ((await props.onSave?.(draft)) === true) editing.set(false);
  };

  return (
    <View testID="lead-detail" class="w-[360px] gap-3 border-l border-border bg-card p-5">
      <View class="flex-row items-center gap-2">
        <Text testID="lead-detail-name" class="min-w-0 flex-1 truncate text-lg font-semibold text-foreground">{props.lead().name}</Text>
        <If when={!editing() && props.onSave !== undefined}>
          <View class="shrink-0"><Button testID="lead-edit" label="Sửa" variant="secondary" size="sm" onPress={startEdit} /></View>
        </If>
        <View class="shrink-0"><Button testID="lead-close" label="Đóng" variant="ghost" size="sm" onPress={() => props.onClose()} /></View>
      </View>
      <If when={editing()}>
        <View class="gap-2">
          <Input testID="lead-edit-name" value={name()} placeholder="Tên khách" onValue={(v) => name.set(v)} />
          <Input testID="lead-edit-company" value={company()} placeholder="Công ty" onValue={(v) => company.set(v)} />
          <View class="flex-row gap-2">
            <View class="flex-1"><Input testID="lead-edit-email" value={email()} placeholder="Email" onValue={(v) => email.set(v)} /></View>
            <View class="flex-1"><Input testID="lead-edit-phone" value={phone()} placeholder="Điện thoại" onValue={(v) => phone.set(v)} /></View>
          </View>
          <Input testID="lead-edit-owner" value={owner()} placeholder="Phụ trách" onValue={(v) => owner.set(v)} />
          <Input testID="lead-edit-value" value={value()} placeholder="Giá trị" onValue={(v) => value.set(v)} />
          <Input testID="lead-edit-about" value={about()} placeholder="Ghi chú" onValue={(v) => about.set(v)} multiline />
          <Input testID="lead-edit-address" value={address()} placeholder="Địa chỉ" onValue={(v) => address.set(v)} />
          <If when={localError() ?? props.saveError?.() ?? null}>
            {(message) => <Text testID="lead-edit-error" class="text-sm text-destructive">{message()}</Text>}
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="lead-edit-cancel" label="Huỷ" disabled={props.saving?.() ?? false} onPress={stopEdit} />
            <Button testID="lead-edit-save" variant="primary" label={(props.saving?.() ?? false) ? "Đang lưu…" : "Lưu"}
              disabled={props.saving?.() ?? false} onPress={() => { void save(); }} />
          </View>
        </View>
      </If>

      <If when={!editing()}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{props.lead().company ?? "—"}</Text>
          <Text class="text-sm text-foreground">{`${props.lead().email ?? "—"} · ${props.lead().phone ?? "—"}`}</Text>
          <Text class="text-sm text-muted-foreground">{props.lead().about ?? ""}</Text>
          <View class="flex-row flex-wrap gap-2">
            <Each of={STATUSES} key={(s) => s}>
              {(s) => (
                <Button testID={`set-status-${s()}`} label={s()} size="sm"
                  variant={props.lead().status === s() ? "primary" : "secondary"}
                  onPress={() => props.onStatus(s())} />
              )}
            </Each>
          </View>
          <If when={props.onSent !== undefined}>
            <View class="flex-row gap-2">
              <Button testID="lead-send-zalo" label="Gửi Zalo" size="sm" variant="secondary"
                onPress={() => { void openZalo(); }} />
              <Button testID="lead-send-email" label="Gửi email" size="sm" variant="secondary"
                onPress={() => { void openEmail(); }} />
            </View>
          </If>
          <Text class="mt-2 text-xs font-medium text-muted-foreground">Hoạt động gần đây</Text>
          <If when={props.activities().length === 0}><Text class="text-sm text-muted-foreground">Chưa có hoạt động.</Text></If>
          <Each of={props.activities().slice(0, 6)} key={(a) => String(a.id)}>
            {(a) => <Text class="text-sm text-foreground">{`${a().activity_type}: ${a().content}`}</Text>}
          </Each>
          <Text class="mt-2 text-xs font-medium text-muted-foreground">Deal</Text>
          <If when={(props.deals?.() ?? []).length === 0}>
            <Text testID="lead-deals-empty" class="text-sm text-muted-foreground">Chưa có deal.</Text>
          </If>
          <Each of={props.deals?.() ?? []} key={(d) => String(d.id)}>
            {(d) => (
              <View testID={`lead-deal-${d().id}`} class="flex-row items-center gap-2">
                <Text class="min-w-0 flex-1 truncate text-sm text-foreground">{d().title}</Text>
                <Badge label={d().stage} tone="neutral" />
                <Text class="shrink-0 text-sm text-muted-foreground">{money(d().value, d().currency)}</Text>
              </View>
            )}
          </Each>
        </View>
      </If>

      <Modal open={zaloOpen()} title="Gửi Zalo" onClose={closeZalo}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`Gửi một tin Zalo cho ${props.lead().name}.`}</Text>
          <View class="w-full">
            <Select testID="send-zalo-account"
              items={zaloAccounts().map((account) => ({ value: account.account_key, title: zaloLabel(account) }))}
              value={zaloAccount()}
              placeholder="Chọn tài khoản Zalo"
              onChange={({ value }) => zaloAccount.set(value ?? "")}
              trigger={
                <View testID="send-zalo-account-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">
                    {zaloAccounts().find((account) => account.account_key === zaloAccount())?.display_name
                      ?? zaloAccounts().find((account) => account.account_key === zaloAccount())?.account_key
                      ?? "Chọn tài khoản Zalo"}
                  </Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={`send-zalo-account-item-${entry.value}`}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
          <View class="w-full">
            <Select testID="send-zalo-template"
              items={zaloTemplates().map((template) => ({ value: String(template.id), title: template.name }))}
              value={zaloTemplate() === null ? null : String(zaloTemplate())}
              placeholder="Chọn mẫu tin"
              onChange={({ value }) => zaloTemplate.set(value === null ? null : Number(value))}
              trigger={
                <View testID="send-zalo-template-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">{zaloTemplateName()}</Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={`send-zalo-template-item-${entry.value}`}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
          <If when={zaloError() !== null}>
            <Text testID="send-zalo-error" class="text-sm text-destructive">{zaloError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" disabled={zaloBusy()} onPress={closeZalo} />
            <Button testID="send-zalo-confirm" variant="primary"
              label={zaloBusy() ? "Đang gửi…" : "Gửi"} disabled={zaloBusy()}
              onPress={() => { void sendZalo(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={emailOpen()} title="Gửi email" onClose={closeEmail}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`Gửi email tới ${props.lead().email ?? ""}.`}</Text>
          <View class="w-full">
            <Select testID="send-email-smtp"
              items={smtpAccounts().map((account) => ({ value: String(account.id), title: `${account.name} · ${account.from_email}` }))}
              value={smtpId() === null ? null : String(smtpId())}
              placeholder="Chọn tài khoản SMTP"
              onChange={({ value }) => smtpId.set(value === null ? null : Number(value))}
              trigger={
                <View testID="send-email-smtp-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">
                    {smtpAccounts().find((account) => account.id === smtpId())?.name ?? "Chọn tài khoản SMTP"}
                  </Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={`send-email-smtp-item-${entry.value}`}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
          <View class="w-full">
            <Select testID="send-email-template"
              items={emailTemplates().map((template) => ({ value: String(template.id), title: template.name }))}
              value={emailTemplateId() === null ? null : String(emailTemplateId())}
              placeholder="Chọn mẫu email"
              onChange={({ value }) => emailTemplateId.set(value === null ? null : Number(value))}
              trigger={
                <View testID="send-email-template-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">{emailTemplateName()}</Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={`send-email-template-item-${entry.value}`}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
          <If when={emailError() !== null}>
            <Text testID="send-email-error" class="text-sm text-destructive">{emailError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" disabled={emailBusy()} onPress={closeEmail} />
            <Button testID="send-email-confirm" variant="primary"
              label={emailBusy() ? "Đang gửi…" : "Gửi"} disabled={emailBusy()}
              onPress={() => { void sendEmail(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
