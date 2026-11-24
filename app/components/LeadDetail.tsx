import { signal, Text, View } from "nsh";
import { Button, Input } from "../ui";
import type { BadgeTone } from "../ui/Badge";

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
  onClose: () => void;
  onStatus: (status: string) => void;
  /** When present, the panel offers the edit form; resolves true once the core saved the lead. */
  onSave?: (draft: LeadEditDraft) => Promise<boolean>;
  /** True while a save is in flight: the form's buttons wait for it. */
  saving?: () => boolean;
  /** The message of a failed save (invalid_request), shown under the fields. */
  saveError?: () => string | null;
}

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
          <Text class="mt-2 text-xs font-medium text-muted-foreground">Hoạt động gần đây</Text>
          <If when={props.activities().length === 0}><Text class="text-sm text-muted-foreground">Chưa có hoạt động.</Text></If>
          <Each of={props.activities().slice(0, 6)} key={(a) => String(a.id)}>
            {(a) => <Text class="text-sm text-foreground">{`${a().activity_type}: ${a().content}`}</Text>}
          </Each>
        </View>
      </If>
    </View>
  );
}
