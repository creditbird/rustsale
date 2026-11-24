import { Text, View } from "nsh";
import { Button } from "../ui";
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

export interface LeadDetailProps {
  lead: () => Lead;
  activities: () => Activity[];
  onClose: () => void;
  onStatus: (status: string) => void;
}

/** The lead detail panel the Leads, Contacts and Categories screens share. */
export function LeadDetail(props: LeadDetailProps) {
  return (
    <View testID="lead-detail" class="w-[340px] gap-3 border-l border-border bg-card p-5">
      <View class="flex-row items-center gap-2">
        <Text testID="lead-detail-name" class="min-w-0 flex-1 truncate text-lg font-semibold text-foreground">{props.lead().name}</Text>
        <View class="shrink-0"><Button testID="lead-close" label="Đóng" variant="ghost" size="sm" onPress={() => props.onClose()} /></View>
      </View>
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
  );
}
