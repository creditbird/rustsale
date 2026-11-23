import { List, Scroll, Text, View } from "nsh";
import { Progress, Select } from "nsh/base";
import { Badge, Button, Checkbox, EmptyState, ErrorState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { OutreachLeads } from "./OutreachLeads";
import { route } from "../stores/nav";
import {
  CAMPAIGN_STATUSES, campaignProgress, dayLabel, useWhatsAppOutreach,
  type CampaignRecipient, type WhatsAppAccount, type WhatsAppCampaign, type WhatsAppState,
} from "../stores/whatsapp";

const TAGS = ["name", "phone", "company", "email", "notes"];
const MAX_PREVIEW_PHONES = 10;

function campaignTone(status: string): BadgeTone {
  const s = status.toLowerCase();
  if (s === "running" || s === "completed") return "success";
  if (s === "paused" || s === "queued") return "warning";
  if (s === "failed" || s === "cancelled") return "danger";
  return "neutral";
}

function recipientTone(status: string): BadgeTone {
  const s = status.toLowerCase();
  if (s === "delivered" || s === "read" || s === "sent") return "success";
  if (s === "running") return "warning";
  if (s === "failed" || s === "blocked") return "danger";
  return "neutral";
}

function accountLabel(account: WhatsAppAccount): string {
  const name = account.display_name?.trim() ?? "";
  if (name !== "") return name;
  const phone = account.phone_number?.trim() ?? "";
  if (phone !== "") return phone;
  return account.provider_account_key.slice(0, 10);
}

function Metric(props: { testID: string; label: string; value: string; hint?: string }) {
  return (
    <View class="min-w-[160px] flex-1 gap-1 rounded-(--radius) border border-border bg-card p-3">
      <Text class="text-[11px] font-medium uppercase text-muted-foreground">{props.label}</Text>
      <Text testID={props.testID} class="text-2xl font-semibold text-foreground">{props.value}</Text>
      {props.hint ? <Text class="text-[10px] text-muted-foreground">{props.hint}</Text> : null}
    </View>
  );
}

function StatusPills(props: { wa: WhatsAppState }) {
  const wa = props.wa;
  return (
    <View class="flex-row flex-wrap items-center gap-1.5">
      <Each of={CAMPAIGN_STATUSES} key={(s) => s.id}>
        {(s) => (
          <Button testID={`wa-filter-${s().id}`} size="sm"
            label={s().id === "all" ? s().label : `${s().label} (${wa.statusCount(s().id)})`}
            variant={wa.statusFilter() === s().id ? "primary" : "ghost"}
            onPress={() => wa.statusFilter.set(s().id)} />
        )}
      </Each>
    </View>
  );
}

function CampaignCard(props: { wa: WhatsAppState; campaign: WhatsAppCampaign }) {
  const c = () => props.campaign;
  const wa = props.wa;
  const status = () => c().status.toLowerCase();
  return (
    <View testID={`wa-campaign-${c().id}`}
      class="min-w-[280px] max-w-[440px] flex-1 gap-3 rounded-(--radius) border border-border bg-card p-4">
      <View class="flex-row items-start justify-between gap-2">
        <View class="min-w-0 flex-1 gap-0.5">
          <Text class="truncate text-sm font-semibold text-foreground">{c().name}</Text>
          <Text class="text-[10px] text-muted-foreground">{`#${c().id} · ${dayLabel(c().created_at)}`}</Text>
        </View>
        <Badge label={c().status} tone={campaignTone(c().status)} />
      </View>

      <View class="gap-1.5">
        <View class="flex-row items-center justify-between">
          <Text testID={`wa-progress-${c().id}`} class="text-xs text-foreground">
            {`Tiến độ: ${c().sent_count} / ${c().total_recipients}`}
          </Text>
          <Text class="text-xs text-muted-foreground">{`${campaignProgress(c())}%`}</Text>
        </View>
        <Progress value={campaignProgress(c())} trackClass="h-2 rounded-full bg-muted"
          indicatorClass="h-2 rounded-full bg-primary" />
      </View>

      <View class="flex-row gap-1">
        <View class="flex-1 items-center rounded bg-muted py-1">
          <Text class="text-[10px] text-muted-foreground">Đã gửi</Text>
          <Text class="text-xs font-semibold text-foreground">{String(c().sent_count)}</Text>
        </View>
        <View class="flex-1 items-center rounded bg-muted py-1">
          <Text class="text-[10px] text-muted-foreground">Đến</Text>
          <Text class="text-xs font-semibold text-foreground">{String(c().delivered_count)}</Text>
        </View>
        <View class="flex-1 items-center rounded bg-muted py-1">
          <Text class="text-[10px] text-muted-foreground">Đọc</Text>
          <Text class="text-xs font-semibold text-foreground">{String(c().read_count)}</Text>
        </View>
        <View class="flex-1 items-center rounded bg-muted py-1">
          <Text class="text-[10px] text-muted-foreground">Lỗi</Text>
          <Text class="text-xs font-semibold text-foreground">{String(c().failed_count)}</Text>
        </View>
      </View>

      <View class="gap-0.5 border-t border-border pt-2">
        <Text class="text-[11px] text-muted-foreground">
          {`Nhịp gửi: ${c().rate_limit_min_delay_secs}s–${c().rate_limit_max_delay_secs}s · lô ${c().burst_limit_per_batch}/${c().burst_pause_secs}s`}
        </Text>
        <If when={c().quiet_hours_enabled}>
          <Text class="text-[11px] text-muted-foreground">
            {`Giờ yên tĩnh: ${c().quiet_hours_start}–${c().quiet_hours_end} (${c().timezone})`}
          </Text>
        </If>
      </View>

      <View class="flex-row items-center justify-between border-t border-border pt-2">
        <Button testID={`wa-detail-${c().id}`} size="sm" variant="ghost" label="Xem chi tiết"
          onPress={() => wa.openDetails(c())} />
        <View class="flex-row items-center gap-1">
          <If when={status() === "draft" || status() === "queued"}>
            <Button testID={`wa-start-${c().id}`} size="sm" label="Bắt đầu"
              onPress={() => { void wa.control(c(), "start"); }} />
          </If>
          <If when={status() === "running"}>
            <Button testID={`wa-pause-${c().id}`} size="sm" label="Tạm dừng"
              onPress={() => { void wa.control(c(), "pause"); }} />
            <Button testID={`wa-cancel-${c().id}`} size="sm" variant="ghost" label="Huỷ"
              onPress={() => { void wa.control(c(), "cancel"); }} />
          </If>
          <If when={status() === "paused"}>
            <Button testID={`wa-resume-${c().id}`} size="sm" label="Tiếp tục"
              onPress={() => { void wa.control(c(), "resume"); }} />
            <Button testID={`wa-cancel-${c().id}`} size="sm" variant="ghost" label="Huỷ"
              onPress={() => { void wa.control(c(), "cancel"); }} />
          </If>
        </View>
      </View>
    </View>
  );
}

function RecipientRow(props: { row: CampaignRecipient }) {
  const r = props.row;
  return (
    <View testID={`wa-recipient-${r.id}`} class="flex-row items-center border-b border-border px-2 py-2">
      <Text class="flex-[2] truncate text-xs text-foreground">{r.recipient_phone_e164 || r.recipient_phone_raw}</Text>
      <View class="flex-1"><Badge label={r.status} tone={recipientTone(r.status)} /></View>
      <Text class="flex-[2] truncate pr-2 text-[11px] text-muted-foreground">
        {r.skip_reason ?? r.last_error ?? "—"}
      </Text>
      <Text class="flex-[3] truncate pr-2 text-[11px] text-muted-foreground">{r.rendered_text ?? "—"}</Text>
      <Text class="w-10 text-center text-xs text-foreground">{String(r.attempt_count)}</Text>
    </View>
  );
}

function CreateCampaignModal(props: { wa: WhatsAppState }) {
  const wa = props.wa;
  const categoryItems = () => [{ value: "", title: "Tất cả danh mục" },
    ...wa.categories().map((c) => ({ value: String(c.id), title: c.name }))];
  return (
    <Modal open={wa.createOpen()} title="Tạo chiến dịch WhatsApp" onClose={wa.closeCreate}
      class="m-auto w-[960px] rounded-(--radius) border border-border bg-popover p-5">
      <View class="gap-3">
        <If when={wa.createError() !== null}>
          <View testID="wa-create-error" class="rounded-(--radius) border border-destructive/40 bg-destructive/10 p-2">
            <Text class="text-xs text-destructive">{wa.createError() ?? ""}</Text>
          </View>
        </If>
        <Scroll class="h-[560px]">
          <View class="gap-4 pr-2">
            <View class="gap-1">
              <Text class="text-xs font-semibold text-foreground">Tên chiến dịch *</Text>
              <Input testID="wa-name" value={wa.name()} placeholder="Ví dụ: Chăm sóc khách hàng tháng 11"
                onValue={(v) => wa.name.set(v)} />
            </View>

            <View class="gap-1.5">
              <View class="flex-row items-center justify-between">
                <Text class="text-xs font-semibold text-foreground">Tài khoản gửi *</Text>
                <Text class="text-[10px] text-muted-foreground">{`${wa.chosenAccounts().length} đã chọn`}</Text>
              </View>
              <If when={wa.accounts().length === 0}>
                <View class="flex-row items-center justify-between rounded-(--radius) border border-border bg-muted/30 p-3">
                  <Text class="text-xs text-muted-foreground">Chưa có tài khoản WhatsApp nào.</Text>
                  <Button testID="wa-open-settings" size="sm" variant="ghost" label="Mở Cài đặt"
                    onPress={() => { wa.closeCreate(); route.set("settings"); }} />
                </View>
              </If>
              <View class="max-h-36 flex-row flex-wrap gap-2 overflow-hidden rounded-(--radius) border border-border p-2">
                <Each of={wa.accounts()} key={(a) => String(a.id)}>
                  {(a) => (
                    <View class="min-w-[240px] flex-1">
                      <Checkbox testID={`wa-account-${a().id}`} checked={wa.chosenAccounts().includes(a().id)}
                        onChange={() => wa.toggleAccount(a().id)}
                        label={`${accountLabel(a())}${a().phone_number ? ` · ${a().phone_number}` : ""}`} />
                    </View>
                  )}
                </Each>
              </View>
            </View>

            <View class="gap-1.5">
              <Text class="text-xs font-semibold text-foreground">Nội dung tin nhắn *</Text>
              <Input testID="wa-template" multiline value={wa.template()}
                class="h-24 rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
                placeholder="Nhập nội dung, dùng {A|B} cho spintax và [name], [company]…"
                onValue={(v) => wa.template.set(v)} />
              <View class="flex-row flex-wrap items-center gap-1.5">
                <Text class="text-[11px] text-muted-foreground">Chèn biến:</Text>
                <Each of={TAGS} key={(t) => t}>
                  {(t) => (
                    <Button testID={`wa-tag-${t()}`} size="sm" variant="ghost" label={`+ [${t()}]`}
                      onPress={() => wa.insertTag(t())} />
                  )}
                </Each>
              </View>
              <View class="gap-1 rounded-(--radius) border border-border bg-muted/30 p-2.5">
                <View class="flex-row items-center justify-between">
                  <Text class="text-[11px] font-semibold text-foreground">Xem trước spintax</Text>
                  <Button testID="wa-reroll" size="sm" variant="ghost" label="Đổi mẫu"
                    onPress={wa.shufflePreview} />
                </View>
                <Text testID="wa-preview" class="text-xs italic text-foreground">{wa.renderedPreview()}</Text>
              </View>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border p-3">
              <Text class="text-xs font-semibold text-foreground">Chống chặn: nhịp gửi và giờ yên tĩnh</Text>
              <View class="flex-row flex-wrap gap-2">
                <View class="min-w-[120px] flex-1 gap-1">
                  <Text class="text-[11px] text-muted-foreground">Trễ tối thiểu (giây)</Text>
                  <Input testID="wa-min-delay" value={wa.minDelay()} onValue={(v) => wa.minDelay.set(v)} />
                </View>
                <View class="min-w-[120px] flex-1 gap-1">
                  <Text class="text-[11px] text-muted-foreground">Trễ tối đa (giây)</Text>
                  <Input testID="wa-max-delay" value={wa.maxDelay()} onValue={(v) => wa.maxDelay.set(v)} />
                </View>
                <View class="min-w-[120px] flex-1 gap-1">
                  <Text class="text-[11px] text-muted-foreground">Số tin mỗi lô</Text>
                  <Input testID="wa-burst-limit" value={wa.burstLimit()} onValue={(v) => wa.burstLimit.set(v)} />
                </View>
                <View class="min-w-[120px] flex-1 gap-1">
                  <Text class="text-[11px] text-muted-foreground">Nghỉ giữa lô (giây)</Text>
                  <Input testID="wa-burst-pause" value={wa.burstPause()} onValue={(v) => wa.burstPause.set(v)} />
                </View>
              </View>
              <Checkbox testID="wa-quiet" checked={wa.quietEnabled()} onChange={(v) => wa.quietEnabled.set(v)}
                label="Bật giờ yên tĩnh" />
              <If when={wa.quietEnabled()}>
                <View class="flex-row flex-wrap gap-2 pl-6">
                  <View class="min-w-[120px] flex-1 gap-1">
                    <Text class="text-[11px] text-muted-foreground">Bắt đầu</Text>
                    <Input testID="wa-quiet-start" value={wa.quietStart()} onValue={(v) => wa.quietStart.set(v)} />
                  </View>
                  <View class="min-w-[120px] flex-1 gap-1">
                    <Text class="text-[11px] text-muted-foreground">Kết thúc</Text>
                    <Input testID="wa-quiet-end" value={wa.quietEnd()} onValue={(v) => wa.quietEnd.set(v)} />
                  </View>
                  <View class="min-w-[160px] flex-1 gap-1">
                    <Text class="text-[11px] text-muted-foreground">Múi giờ</Text>
                    <Input testID="wa-timezone" value={wa.timezone()} onValue={(v) => wa.timezone.set(v)} />
                  </View>
                </View>
              </If>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border p-3">
              <View class="flex-row items-center justify-between">
                <Text class="text-xs font-semibold text-foreground">Người nhận</Text>
                <Badge label={`${wa.recipientCount()} người nhận`} tone="neutral" />
              </View>
              <View class="flex-row items-center gap-2">
                <Button testID="wa-mode-crm" size="sm" label="Từ CRM"
                  variant={wa.recipientMode() === "crm" ? "primary" : "secondary"}
                  onPress={() => wa.recipientMode.set("crm")} />
                <Button testID="wa-mode-manual" size="sm" label="Số thủ công"
                  variant={wa.recipientMode() === "manual" ? "primary" : "secondary"}
                  onPress={() => wa.recipientMode.set("manual")} />
              </View>

              <If when={wa.recipientMode() === "crm"}>
                <View class="gap-2">
                  <View class="flex-row items-center gap-2">
                    <Text class="text-[11px] text-muted-foreground">Danh mục:</Text>
                    <View class="flex-1">
                      <Select testID="wa-category" items={categoryItems()}
                        value={wa.pick.categoryId() === null ? "" : String(wa.pick.categoryId())}
                        placeholder={wa.categoryLabel()}
                        onChange={({ value }) => wa.pick.setCategory(value === null || value === "" ? null : Number(value))}
                        trigger={
                          <View class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                            <Text class="truncate text-sm text-foreground">{wa.categoryLabel()}</Text>
                            <Text class="text-xs text-muted-foreground">▾</Text>
                          </View>
                        }
                        itemClass="px-3 py-2 selected:bg-accent" />
                    </View>
                  </View>
                  <Checkbox testID="wa-target-all" checked={wa.pick.allMatch()}
                    onChange={(v) => { if (v) wa.pick.selectAll(); else wa.pick.clearChoice(); }}
                    label="Gửi tất cả lead khớp bộ lọc (bỏ chọn để tự chọn từng lead)" />
                  <View class="h-[300px]">
                    <OutreachLeadsTable wa={wa} />
                  </View>
                </View>
              </If>

              <If when={wa.recipientMode() === "manual"}>
                <View class="gap-2">
                  <Input testID="wa-manual" multiline value={wa.manualPhones()}
                    class="h-24 rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
                    placeholder="Mỗi dòng một số, hoặc cách nhau bằng dấu phẩy/chấm phẩy"
                    onValue={(v) => wa.manualPhones.set(v)} />
                  <If when={wa.parsedManual().length > 0}>
                    <View class="gap-1 rounded-(--radius) border border-border bg-muted/30 p-2">
                      <Text testID="wa-manual-count" class="text-[11px] font-semibold text-foreground">
                        {`Số hợp lệ sau chuẩn hoá (${wa.parsedManual().length}):`}
                      </Text>
                      <View class="flex-row flex-wrap gap-1">
                        <Each of={wa.parsedManual().slice(0, MAX_PREVIEW_PHONES)} key={(p) => p.e164}>
                          {(p) => <Badge label={p().e164} tone="neutral" />}
                        </Each>
                      </View>
                    </View>
                  </If>
                </View>
              </If>
            </View>
          </View>
        </Scroll>
        <View class="flex-row items-center justify-end gap-2 border-t border-border pt-3">
          <Button label="Huỷ" onPress={wa.closeCreate} disabled={wa.creating()} />
          <Button testID="wa-create-submit" variant="primary" disabled={wa.creating()}
            label={wa.creating() ? "Đang tạo…" : "Tạo chiến dịch"}
            onPress={() => { void wa.create(); }} />
        </View>
      </View>
    </Modal>
  );
}

/** The L2 lead picker, scoped to the WhatsApp dialog (phone-capable leads, no Zalo history filters). */
function OutreachLeadsTable(props: { wa: WhatsAppState }) {
  return <OutreachLeads pick={props.wa.pick} prefix="wap" zalo={false} showSegmentation={false} />;
}

function DetailsModal(props: { wa: WhatsAppState }) {
  const wa = props.wa;
  return (
    <Modal open={wa.detailsOpen()} title="Chi tiết chiến dịch" onClose={wa.closeDetails}
      class="m-auto w-[900px] rounded-(--radius) border border-border bg-popover p-5">
      <View class="gap-3">
        <If when={wa.selected()}>
          {(c) => (
            <View class="flex-row items-center justify-between gap-2">
              <View class="min-w-0 flex-1">
                <Text class="truncate text-sm font-semibold text-foreground">{c().name}</Text>
                <View class="flex-row items-center gap-2">
                  <Badge label={c().status} tone={campaignTone(c().status)} />
                  <Text testID="wa-details-caption" class="text-xs text-muted-foreground">
                    {`${wa.recipients().length} / ${c().total_recipients} người nhận`}
                  </Text>
                </View>
              </View>
              <Button testID="wa-details-refresh" size="sm" label="Làm mới"
                disabled={wa.recipientsLoading()} onPress={() => { void wa.openDetails(c()); }} />
            </View>
          )}
        </If>

        <View class="flex-row items-center border-b border-border bg-muted px-2 py-2">
          <Text class="flex-[2] text-[11px] font-medium text-muted-foreground">Số điện thoại</Text>
          <Text class="flex-1 text-[11px] font-medium text-muted-foreground">Trạng thái</Text>
          <Text class="flex-[2] text-[11px] font-medium text-muted-foreground">Lý do / lỗi</Text>
          <Text class="flex-[3] text-[11px] font-medium text-muted-foreground">Tin đã render</Text>
          <Text class="w-10 text-center text-[11px] font-medium text-muted-foreground">Thử</Text>
        </View>

        <If when={wa.recipientsLoading() && wa.recipients().length === 0}>
          <View class="gap-2 p-2"><Skeleton /><Skeleton /><Skeleton /></View>
        </If>
        <If when={!wa.recipientsLoading() && wa.recipients().length === 0}>
          <EmptyState title="Chưa có người nhận" hint="Chiến dịch này chưa được nạp danh sách gửi." />
        </If>
        <View class="h-[340px] overflow-hidden rounded-(--radius) border border-border">
          <List testID="wa-recipients-list" each={wa.recipients()} keyExtractor={(r) => String(r.id)}
            estimatedItemSize={40} onLoadMore={wa.moreRecipients} class="flex-1">
            {(r) => <RecipientRow row={r()} />}
          </List>
        </View>
        <If when={!wa.recipientsDone() && wa.recipients().length > 0}>
          <View class="flex-row justify-center">
            <Button testID="wa-recipients-more" size="sm" variant="ghost" label="Xem thêm"
              onPress={wa.moreRecipients} disabled={wa.recipientsLoading()} />
          </View>
        </If>
        <View class="flex-row justify-end border-t border-border pt-2">
          <Button testID="wa-details-close" label="Đóng" onPress={wa.closeDetails} />
        </View>
      </View>
    </Modal>
  );
}

/** WhatsApp campaigns: metrics, filters, campaign cards with controls, create form and recipient ledger. */
export function WhatsAppOutreachScreen() {
  const wa = useWhatsAppOutreach();
  return (
    <View class="flex-1">
      <Scroll class="flex-1">
        <View class="gap-4 p-6">
          <View class="flex-row items-center justify-between gap-3">
            <View class="min-w-0 flex-1">
              <Text class="text-xl font-semibold text-foreground">WhatsApp Outreach</Text>
              <Text class="text-xs text-muted-foreground">
                Chiến dịch gửi hàng loạt nhiều tài khoản, có nhịp gửi chống chặn và lọc người nhận.
              </Text>
            </View>
            <View class="flex-row items-center gap-2">
              <Button testID="wa-refresh" size="sm" label="Làm mới" disabled={wa.loading()}
                onPress={() => { void wa.loadCampaigns(); }} />
              <Button size="sm" variant="ghost" label="Tài khoản" onPress={() => route.set("settings")} />
              <Button testID="wa-new" size="sm" variant="primary" label="Chiến dịch mới"
                onPress={wa.openCreate} />
            </View>
          </View>

          <View class="flex-row flex-wrap gap-3">
            <Metric testID="wa-metric-total" label="Tổng chiến dịch" value={String(wa.totalCampaigns())} />
            <Metric testID="wa-metric-running" label="Đang chạy / chờ" value={String(wa.runningCampaigns())} />
            <Metric testID="wa-metric-sent" label="Tổng đã gửi" value={String(wa.totalSent())} />
            <Metric testID="wa-metric-rate" label="Tỉ lệ đến + đọc" value={`${wa.deliveryReadRate()}%`}
              hint={`${wa.totalDelivered()} đến / ${wa.totalRead()} đọc`} />
            <Metric testID="wa-metric-skipped" label="Bị bỏ qua" value={String(wa.totalSkipped())} />
          </View>

          <View class="flex-row flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
            <StatusPills wa={wa} />
            <View class="w-64">
              <Input testID="wa-search" value={wa.searchQuery()} placeholder="Tìm tên chiến dịch…"
                onValue={(v) => wa.searchQuery.set(v)} />
            </View>
          </View>

          <If when={!wa.ready() && !wa.loadFailed()}>
            <View class="flex-row flex-wrap gap-3"><Skeleton class="h-40 flex-1" /><Skeleton class="h-40 flex-1" /></View>
          </If>
          <If when={wa.loadFailed() && !wa.ready()}>
            <ErrorState message="Chưa tải được danh sách chiến dịch." onRetry={() => { void wa.loadCampaigns(); }} />
          </If>
          <If when={wa.ready() && wa.filtered().length === 0}>
            <EmptyState title="Chưa có chiến dịch nào"
              hint="Tạo chiến dịch mới để gửi WhatsApp hàng loạt có kiểm soát." />
          </If>
          <View class="flex-row flex-wrap gap-3">
            <Each of={wa.filtered()} key={(c) => String(c.id)}>
              {(c) => <CampaignCard wa={wa} campaign={c()} />}
            </Each>
          </View>
        </View>
      </Scroll>
      <CreateCampaignModal wa={wa} />
      <DetailsModal wa={wa} />
    </View>
  );
}
