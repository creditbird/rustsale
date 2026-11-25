import { Scroll, Text, View } from "nsh";
import { Badge, Button, EmptyState, ErrorState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import {
  AI_STATUSES, hhmm, useAiApproval, type AiApprovalState, type AiProposal, type LeadEvidence,
} from "../stores/aiApproval";

function platformLabel(platform: string): string {
  const p = platform.toLowerCase();
  if (p === "zalo") return "Zalo";
  if (p === "whatsapp") return "WhatsApp";
  if (p === "email") return "Email";
  if (p === "facebook" || p === "messenger") return "Facebook";
  return "AI";
}

function statusLabel(status: string): string {
  if (status === "APPROVED") return "ĐÃ DUYỆT";
  if (status === "REJECTED") return "ĐÃ TỪ CHỐI";
  if (status === "APPLIED") return "ĐÃ ÁP DỤNG";
  return "CHỜ DUYỆT";
}

function statusTone(status: string): BadgeTone {
  if (status === "APPROVED" || status === "APPLIED") return "success";
  if (status === "REJECTED") return "danger";
  return "warning";
}

function EvidencePanel(props: { proposal: AiProposal; state: AiApprovalState }) {
  const st = props.state;
  const ev = () => st.evidence();
  return (
    <View testID={`ai-evidence-panel-${props.proposal.id ?? 0}`} class="gap-1.5 rounded-(--radius) border border-border bg-muted/30 p-3">
      <View class="flex-row items-center justify-between">
        <Text class="text-[11px] font-semibold text-foreground">Bằng chứng từ CRM</Text>
        <Button size="sm" variant="ghost" label="Đóng" onPress={st.closeEvidence} />
      </View>
      <If when={st.evidenceLoading()}>
        <View class="gap-1"><Skeleton /><Skeleton /></View>
      </If>
      <If when={st.evidenceError() !== null}>
        <Text class="text-xs text-destructive">{st.evidenceError() ?? ""}</Text>
      </If>
      <If when={ev()}>
        {(e: () => LeadEvidence) => (
          <View class="gap-1">
            <Text class="text-xs text-foreground">
              {`Khách: ${e().lead_name}${e().lead_company ? ` · ${e().lead_company}` : ""}`}
            </Text>
            <Text class="text-[11px] text-muted-foreground">
              {`${e().lead_phone || "chưa có SĐT"} · ${e().lead_email || "chưa có email"} · ${e().lead_status}`}
            </Text>
            <If when={e().deal_title !== null || e().deal_stage !== null}>
              <Text class="text-[11px] text-muted-foreground">
                {`Deal: ${e().deal_title ?? "—"}${e().deal_stage ? ` · ${e().deal_stage}` : ""}${e().deal_value !== null ? ` · ${e().deal_value} ${e().lead_currency}` : ""}`}
              </Text>
            </If>
            <If when={e().journey_stage !== null}>
              <Text class="text-[11px] text-muted-foreground">{`Hành trình: ${e().journey_stage}`}</Text>
            </If>
            <If when={e().categories.length > 0}>
              <Text class="text-[11px] text-muted-foreground">{`Danh mục: ${e().categories.join(", ")}`}</Text>
            </If>
            <If when={e().verified_channels.length > 0}>
              <Text class="text-[11px] text-muted-foreground">{`Kênh đã xác minh: ${e().verified_channels.join(", ")}`}</Text>
            </If>
            <If when={e().recent_messages.length > 0}>
              <Text class="mt-1 text-[11px] font-medium text-muted-foreground">Tin gần đây</Text>
              <Each of={e().recent_messages.slice(-3)} key={(m) => String(m.id)}>
                {(m) => (
                  <Text class="text-[11px] text-foreground">
                    {`${m().direction === "inbound" ? "←" : "→"} ${m().text_content}`}
                  </Text>
                )}
              </Each>
            </If>
          </View>
        )}
      </If>
    </View>
  );
}

function ProposalCard(props: { proposal: AiProposal; state: AiApprovalState }) {
  const p = props.proposal;
  const st = props.state;
  const info = () => (p.lead_id !== null ? st.leadInfo()[p.lead_id] : undefined);
  const leadLabel = () => {
    if (p.lead_id === null) return "Lead chưa gắn kết";
    const name = info()?.name ?? "";
    return name !== "" ? name : `Lead #${p.lead_id}`;
  };
  return (
    <View testID={`ai-proposal-${p.id ?? 0}`} class="gap-3 rounded-(--radius) border border-border bg-card p-4">
      <View class="flex-row items-center justify-between gap-2 border-b border-border pb-2">
        <View class="min-w-0 flex-1 gap-0.5">
          <View class="flex-row items-center gap-2">
            <Badge label={platformLabel(p.platform)} tone="neutral" />
            <Text class="truncate text-sm font-semibold text-foreground">{leadLabel()}</Text>
          </View>
          <Text class="text-[11px] text-muted-foreground">
            {`${info()?.phone ? `${info()?.phone} · ` : ""}${platformLabel(p.platform)}`}
          </Text>
        </View>
        <View class="flex-row items-center gap-2">
          <Badge label={statusLabel(p.status)} tone={statusTone(p.status)} />
          <Text class="text-[11px] text-muted-foreground">{hhmm(p.created_at)}</Text>
        </View>
      </View>

      <View class="rounded-(--radius) bg-muted/40 p-3">
        <Text testID={`ai-content-${p.id ?? 0}`} class="text-xs text-foreground">{p.proposed_content}</Text>
      </View>

      <View class="gap-0.5 rounded-(--radius) border border-border bg-muted/20 p-2.5">
        <Text class="text-[11px] font-semibold text-foreground">Lý giải từ AI Agent (Evidence Ledger)</Text>
        <Text class="text-[11px] text-muted-foreground">{p.reasoning}</Text>
      </View>

      <View class="flex-row items-center justify-between gap-2 pt-1">
        <View class="flex-row items-center gap-1">
          <If when={p.lead_id !== null}>
            <Button testID={`ai-evidence-${p.id ?? 0}`} size="sm" variant="ghost" label="Bằng chứng"
              onPress={() => { void st.toggleEvidence(p); }} />
          </If>
        </View>
        <If when={p.status === "PENDING"}>
          <View class="flex-row items-center gap-2">
            <Button testID={`ai-reject-${p.id ?? 0}`} size="sm" variant="ghost" label="Từ chối"
              disabled={st.reviewing()} onPress={() => { void st.reject(p); }} />
            <Button testID={`ai-edit-${p.id ?? 0}`} size="sm" label="Chỉnh sửa"
              disabled={st.reviewing()} onPress={() => st.openEdit(p)} />
            <Button testID={`ai-approve-${p.id ?? 0}`} size="sm" variant="primary" label="Duyệt"
              disabled={st.reviewing()} onPress={() => { void st.approve(p); }} />
          </View>
        </If>
      </View>

      <If when={st.evidenceFor() === p.id}>
        <EvidencePanel proposal={p} state={st} />
      </If>
    </View>
  );
}

function EditModal(props: { state: AiApprovalState }) {
  const st = props.state;
  return (
    <Modal open={st.editOpen()} title="Chỉnh sửa tin nhắn trước khi duyệt" onClose={st.closeEdit}
      class="m-auto w-[560px] rounded-(--radius) border border-border bg-popover p-5">
      <View class="gap-3">
        <Text class="text-xs font-semibold text-muted-foreground">Nội dung tin nhắn</Text>
        <Input testID="ai-edit-text" multiline value={st.editedContent()}
          class="h-32 rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground focus:border-ring"
          onValue={(v) => st.editedContent.set(v)} />
        <Text testID="ai-edit-note" class={st.editProblem() !== null || st.editError() !== null ? "text-[11px] text-destructive" : "text-[11px] text-muted-foreground"}>
          {st.editProblem() ?? st.editError()
            ?? (st.editChanged()
              ? "Nội dung đã sửa sẽ được lưu lại trước khi duyệt."
              : "Giữ nguyên nội dung gốc hoặc sửa trước khi duyệt.")}
        </Text>
        <View class="flex-row items-center justify-between border-t border-border pt-2">
          <Button label="Huỷ" onPress={st.closeEdit} disabled={st.reviewing()} />
          <Button testID="ai-edit-save" variant="primary" label="Lưu & Duyệt"
            disabled={st.reviewing() || st.editProblem() !== null} onPress={() => { void st.saveAndApprove(); }} />
        </View>
      </View>
    </Modal>
  );
}

/** Human-in-the-loop queue: pending AI proposals, their evidence, and approve / reject / edit. */
export function AiApprovalScreen() {
  const st = useAiApproval();
  return (
    <View class="flex-1">
      <Scroll class="flex-1">
        <View class="mx-auto w-full max-w-[1100px] gap-4 p-6">
          <If when={st.error() !== null}>
            <View testID="ai-error" class="flex-row items-center justify-between gap-2 rounded-(--radius) border border-destructive/40 bg-destructive/10 p-3">
              <Text class="flex-1 text-xs text-destructive">{st.error() ?? ""}</Text>
              <Button size="sm" variant="ghost" label="Đóng" onPress={() => st.error.set(null)} />
            </View>
          </If>
          <If when={st.success() !== null}>
            <View testID="ai-success" class="flex-row items-center justify-between gap-2 rounded-(--radius) border border-success/40 bg-success/10 p-3">
              <Text class="flex-1 text-xs text-success">{st.success() ?? ""}</Text>
              <Button size="sm" variant="ghost" label="Đóng" onPress={() => st.success.set(null)} />
            </View>
          </If>

          <View class="flex-row items-center justify-between gap-3 rounded-(--radius) border border-border bg-card p-4">
            <View class="min-w-0 flex-1 gap-1">
              <View class="flex-row items-center gap-2">
                <Text class="text-lg font-semibold text-foreground">Hàng đợi duyệt tin AI (Human-in-the-Loop)</Text>
                <Badge label={`${st.pendingCount()} tin chờ duyệt`} tone="neutral" />
              </View>
              <Text class="text-xs text-muted-foreground">
                Mọi tin nhắn outbound do AI Agent soạn đều dừng ở đây để nhân viên kiểm duyệt. Bản v3 chỉ ghi trạng thái duyệt, chưa tự gửi tin.
              </Text>
            </View>
            <Button testID="ai-refresh" size="sm" label="Làm mới" disabled={st.loading()}
              onPress={() => { void st.load(); }} />
          </View>

          <View class="flex-row flex-wrap items-center justify-between gap-3 rounded-(--radius) border border-border bg-card p-3">
            <View class="w-80">
              <Input testID="ai-search" value={st.search()} placeholder="Tìm theo nội dung, lý do AI, khách hàng…"
                onValue={(v) => st.search.set(v)} />
            </View>
            <View class="flex-row flex-wrap items-center gap-1.5">
              <Each of={AI_STATUSES} key={(s) => s.id}>
                {(s) => (
                  <Button testID={`ai-filter-${s().id}`} size="sm" label={s().label}
                    variant={st.statusFilter() === s().id ? "primary" : "secondary"}
                    onPress={() => st.statusFilter.set(s().id)} />
                )}
              </Each>
            </View>
          </View>

          <If when={!st.ready() && !st.failed()}>
            <View class="gap-2"><Skeleton class="h-32" /><Skeleton class="h-32" /></View>
          </If>
          <If when={st.failed() && !st.ready()}>
            <ErrorState message="Chưa tải được hàng đợi đề xuất AI." onRetry={() => { void st.load(); }} />
          </If>
          <If when={st.ready() && st.loading() && st.proposals().length === 0}>
            <View class="gap-2"><Skeleton class="h-32" /><Skeleton class="h-32" /></View>
          </If>
          <If when={st.ready() && st.filtered().length === 0}>
            <View testID="ai-empty">
              <EmptyState
                title={st.statusFilter() === "PENDING"
                  ? "Không có tin nhắn nào đang chờ duyệt"
                  : "Không có đề xuất nào ở trạng thái này"}
                hint={st.statusFilter() === "PENDING"
                  ? "Tất cả đề xuất của AI đã được xử lý xong."
                  : "Đổi bộ lọc trạng thái để xem các đề xuất khác."} />
            </View>
          </If>
          <View class="gap-3">
            <Each of={st.filtered()} key={(p) => String(p.id)}>
              {(p) => <ProposalCard proposal={p()} state={st} />}
            </Each>
          </View>
        </View>
      </Scroll>
      <EditModal state={st} />
    </View>
  );
}
