import { Scroll, signal, Text, View } from "nsh";
import { Campaigns, Leads } from "rustsale/native";
import { Badge, Button, Checkbox, EmptyState, ErrorState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { startCore, type Page } from "../stores/core";
import { notify, report } from "../stores/toast";
import {
  CHANNELS, CONDITIONS, DELAYS, PLACEHOLDERS,
  buildPayload, delayLabel, moveStep, newStep, percent, renumber, statusLabel, validateCampaign,
  type CampaignAnalytics, type StepDraft, type UnifiedCampaign,
} from "../stores/campaigns";

interface PickLead { id: number; name: string; phone: string; company: string }

function tone(status: string): BadgeTone {
  if (status === "ACTIVE") return "success";
  if (status === "PAUSED") return "warning";
  return "neutral";
}

function Metric(props: { testID: string; label: string; value: string }) {
  return (
    <View class="min-w-[140px] flex-1 gap-1 rounded-(--radius) border border-border bg-card p-3">
      <Text class="text-[11px] font-medium uppercase text-muted-foreground">{props.label}</Text>
      <Text testID={props.testID} class="text-2xl font-semibold text-foreground">{props.value}</Text>
    </View>
  );
}

/** Chiến dịch (UI-32/33): the unified multi-step campaigns, their builder, enrolment and analytics. */
export function CampaignCenterScreen() {
  const campaigns = signal<UnifiedCampaign[]>([]);
  const loaded = signal(false);
  const failed = signal(false);
  const busy = signal<number | null>(null);

  const loadList = async () => {
    try {
      await startCore();
      campaigns.set((await Campaigns.unifiedCampaigns()) as UnifiedCampaign[]);
      failed.set(false);
    } catch (e) {
      failed.set(true);
      report(e, { retry: () => { void loadList(); } });
    } finally {
      loaded.set(true);
    }
  };

  // ---- builder
  const building = signal(false);
  const name = signal("");
  const description = signal("");
  const steps = signal<StepDraft[]>([]);
  const buildError = signal<string | null>(null);
  const saving = signal(false);

  const openBuilder = () => {
    name.set("");
    description.set("");
    steps.set([newStep(1)]);
    buildError.set(null);
    building.set(true);
  };
  const patchStep = (i: number, patch: Partial<StepDraft>) =>
    steps.set(steps().map((s, k) => (k === i ? { ...s, ...patch } : s)));
  const addStep = () => steps.set([...steps(), newStep(steps().length + 1, steps()[steps().length - 1])]);
  const removeStep = (i: number) => steps.set(renumber(steps().filter((_, k) => k !== i)));

  const save = async () => {
    if (saving()) return;
    const problem = validateCampaign(name(), steps());
    if (problem !== null) { buildError.set(problem); return; }
    saving.set(true);
    buildError.set(null);
    try {
      const body = buildPayload(name(), description(), steps());
      await Campaigns.saveUnifiedCampaign(body.campaign, body.steps);
      building.set(false);
      notify("Đã lưu chiến dịch", "success");
      await loadList();
    } catch (e) {
      const f = report(e, { retry: () => { void save(); } });
      if (f.action === "fix") buildError.set(f.message);
    } finally {
      saving.set(false);
    }
  };

  // ---- start / pause
  const toggle = async (c: UnifiedCampaign) => {
    if (busy() !== null) return;
    busy.set(c.id);
    try {
      if (c.status === "ACTIVE") await Campaigns.pauseUnifiedCampaign(c.id);
      else await Campaigns.startUnifiedCampaign(c.id);
      notify(c.status === "ACTIVE" ? `Đã tạm dừng "${c.name}"` : `Đã kích hoạt "${c.name}"`, "success");
      await loadList();
    } catch (e) {
      report(e, { refresh: () => { void loadList(); }, retry: () => { void toggle(c); } });
    } finally {
      busy.set(null);
    }
  };

  // ---- delete
  const deleting = signal<UnifiedCampaign | null>(null);
  const confirmDelete = async () => {
    const c = deleting();
    if (c === null) return;
    try {
      await Campaigns.deleteUnifiedCampaign(c.id);
      deleting.set(null);
      notify(`Đã xoá "${c.name}"`, "success");
      await loadList();
    } catch (e) {
      deleting.set(null);
      report(e, { refresh: () => { void loadList(); } });
    }
  };

  // ---- analytics
  const analyticsFor = signal<UnifiedCampaign | null>(null);
  const analytics = signal<CampaignAnalytics | null>(null);
  const openAnalytics = async (c: UnifiedCampaign) => {
    analyticsFor.set(c);
    analytics.set(null);
    try {
      const a = (await Campaigns.unifiedCampaignAnalytics(c.id)) as CampaignAnalytics;
      if (analyticsFor()?.id === c.id) analytics.set(a);
    } catch (e) {
      analyticsFor.set(null);
      report(e, { refresh: () => { void loadList(); }, retry: () => { void openAnalytics(c); } });
    }
  };

  // ---- enrol
  const enrolFor = signal<UnifiedCampaign | null>(null);
  const leads = signal<PickLead[]>([]);
  const leadsLoading = signal(false);
  const search = signal("");
  const categoryId = signal<number | null>(null);
  const categories = signal<{ id: number; name: string }[]>([]);
  const chosen = signal<number[]>([]);
  const enrolling = signal(false);
  let leadGen = 0;

  const loadLeads = async () => {
    leadGen += 1;
    const mine = leadGen;
    leadsLoading.set(true);
    try {
      const page = (await Leads.page(
        { search: search().trim() || undefined, categoryId: categoryId() ?? undefined, sort: "name" },
        null,
        100,
      )) as Page<PickLead>;
      if (mine === leadGen) leads.set(page.items);
    } catch (e) {
      if (mine === leadGen) report(e, { retry: () => { void loadLeads(); } });
    } finally {
      if (mine === leadGen) leadsLoading.set(false);
    }
  };
  const openEnrol = async (c: UnifiedCampaign) => {
    enrolFor.set(c);
    chosen.set([]);
    search.set("");
    categoryId.set(null);
    void loadLeads();
    try {
      categories.set((await Leads.categories()) as { id: number; name: string }[]);
    } catch (e) {
      report(e, {});
    }
  };
  const toggleLead = (id: number, on: boolean) =>
    chosen.set(on ? [...chosen().filter((x) => x !== id), id] : chosen().filter((x) => x !== id));
  const confirmEnrol = async () => {
    const c = enrolFor();
    if (c === null || chosen().length === 0 || enrolling()) return;
    enrolling.set(true);
    try {
      const count = Number(await Campaigns.enrollLeadsIntoCampaign(c.id, chosen()));
      enrolFor.set(null);
      notify(`Đã ghi danh ${count} khách vào "${c.name}"`, "success");
    } catch (e) {
      report(e, { refresh: () => { enrolFor.set(null); void loadList(); } });
    } finally {
      enrolling.set(false);
    }
  };

  void loadList();

  return (
    <View class="flex-1">
      <Scroll class="flex-1">
        <View class="gap-4 p-6">
          <View class="flex-row items-center justify-between">
            <View class="gap-1">
              <Text class="text-xl font-semibold text-foreground">Chiến dịch</Text>
              <Text testID="cc-caption" class="text-sm text-muted-foreground">
                {loaded()
                  ? `${campaigns().length} chiến dịch · ${campaigns().filter((c) => c.status === "ACTIVE").length} đang chạy`
                  : "Đang mở dữ liệu…"}
              </Text>
            </View>
            <View class="flex-row gap-2">
              <Button testID="cc-refresh" label="Làm mới" size="sm" onPress={() => { void loadList(); }} />
              <Button testID="cc-new" label="Tạo kịch bản mới" variant="primary" size="sm" onPress={openBuilder} />
            </View>
          </View>

          <If when={!loaded()}>
            <View class="gap-2"><Skeleton /><Skeleton /><Skeleton /></View>
          </If>
          <If when={loaded() && failed() && campaigns().length === 0}>
            <ErrorState message="Chưa tải được chiến dịch." onRetry={() => { void loadList(); }} />
          </If>
          <If when={loaded() && !failed() && campaigns().length === 0}>
            <View class="rounded-(--radius) border border-border bg-card">
              <EmptyState title="Chưa có chiến dịch nào" hint="Tạo chuỗi tiếp cận nhiều bước qua Zalo, WhatsApp, Email, Facebook." />
            </View>
          </If>

          <Each of={campaigns()} key={(c) => String(c.id)}>
            {(c) => (
              <View testID={`cc-campaign-${c().id}`} class="gap-3 rounded-(--radius) border border-border bg-card p-4">
                <View class="flex-row items-start justify-between gap-2">
                  <View class="min-w-0 flex-1 gap-0.5">
                    <Text class="truncate text-sm font-semibold text-foreground">{c().name}</Text>
                    <Text class="truncate text-xs text-muted-foreground">{c().description ?? "Không có mô tả"}</Text>
                  </View>
                  <Badge label={statusLabel(c().status)} tone={tone(c().status)} />
                </View>
                <View class="flex-row flex-wrap gap-2">
                  <Button testID={`cc-toggle-${c().id}`} size="sm" disabled={busy() !== null}
                    label={c().status === "ACTIVE" ? "Dừng" : "Chạy"}
                    variant={c().status === "ACTIVE" ? "secondary" : "primary"}
                    onPress={() => { void toggle(c()); }} />
                  <Button testID={`cc-enrol-${c().id}`} label="Ghi danh" size="sm" onPress={() => { void openEnrol(c()); }} />
                  <Button testID={`cc-analytics-${c().id}`} label="Báo cáo" size="sm" onPress={() => { void openAnalytics(c()); }} />
                  <Button testID={`cc-delete-${c().id}`} label="Xoá" size="sm" variant="ghost" onPress={() => deleting.set(c())} />
                </View>
              </View>
            )}
          </Each>
        </View>
      </Scroll>

      <Modal open={building()} title="Kịch bản chiến dịch mới" onClose={() => building.set(false)}
        class="m-auto h-[640px] w-[720px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="flex-1 gap-3">
          <Input testID="cc-name" value={name()} placeholder="Tên chiến dịch" onValue={(v) => name.set(v)} />
          <Input testID="cc-desc" value={description()} placeholder="Mô tả (không bắt buộc)" onValue={(v) => description.set(v)} />
          <Text class="text-xs text-muted-foreground">{`Biến dùng được: ${PLACEHOLDERS.join(" ")} · spintax: {Chào|Xin chào}`}</Text>
          <Scroll class="h-[360px]">
            <View class="gap-3">
              <Each of={steps()} key={(s) => String(s.step_order)}>
                {(s, i) => (
                  <View testID={`cc-step-${s().step_order}`} class="gap-2 rounded-(--radius) border border-border p-3">
                    <View class="flex-row items-center justify-between">
                      <Text class="text-sm font-semibold text-foreground">{`Bước ${s().step_order} · sau ${delayLabel(s().delay_minutes)}`}</Text>
                      <View class="flex-row gap-1">
                        <Button testID={`cc-step-up-${s().step_order}`} label="↑" size="sm" variant="ghost" disabled={i() === 0}
                          onPress={() => steps.set(moveStep(steps(), i(), -1))} />
                        <Button testID={`cc-step-down-${s().step_order}`} label="↓" size="sm" variant="ghost" disabled={i() === steps().length - 1}
                          onPress={() => steps.set(moveStep(steps(), i(), 1))} />
                        <Button testID={`cc-step-remove-${s().step_order}`} label="Xoá bước" size="sm" variant="ghost" onPress={() => removeStep(i())} />
                      </View>
                    </View>
                    <View class="flex-row flex-wrap gap-1">
                      <Each of={CHANNELS} key={(c) => c.id}>
                        {(c) => (
                          <Button testID={`cc-step-${s().step_order}-ch-${c().id}`} label={c().label} size="sm"
                            variant={s().channel === c().id ? "primary" : "secondary"}
                            onPress={() => patchStep(i(), { channel: c().id })} />
                        )}
                      </Each>
                    </View>
                    <View class="flex-row flex-wrap gap-1">
                      <Each of={DELAYS} key={(d) => String(d.minutes)}>
                        {(d) => (
                          <Button testID={`cc-step-${s().step_order}-delay-${d().minutes}`} label={d().label} size="sm"
                            variant={s().delay_minutes === d().minutes ? "primary" : "ghost"}
                            onPress={() => patchStep(i(), { delay_minutes: d().minutes })} />
                        )}
                      </Each>
                    </View>
                    <View class="flex-row flex-wrap gap-1">
                      <Each of={CONDITIONS} key={(c) => c.id}>
                        {(c) => (
                          <Button testID={`cc-step-${s().step_order}-cond-${c().id}`} label={c().label} size="sm"
                            variant={s().condition_type === c().id ? "primary" : "ghost"}
                            onPress={() => patchStep(i(), { condition_type: c().id })} />
                        )}
                      </Each>
                    </View>
                    <Input testID={`cc-step-${s().step_order}-content`} value={s().template_content} multiline
                      placeholder="Nội dung tin nhắn" onValue={(v) => patchStep(i(), { template_content: v })} />
                    <Input testID={`cc-step-${s().step_order}-ai`} value={s().ai_prompt_override ?? ""}
                      placeholder="Gợi ý cho AI (không bắt buộc)"
                      onValue={(v) => patchStep(i(), { ai_prompt_override: v.trim() === "" ? null : v })} />
                  </View>
                )}
              </Each>
            </View>
          </Scroll>
          <Button testID="cc-add-step" label="+ Thêm bước" size="sm" onPress={addStep} />
          <If when={buildError()}>
            {(m) => <Text testID="cc-build-error" class="text-sm text-destructive">{m()}</Text>}
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-build-cancel" label="Huỷ" disabled={saving()} onPress={() => building.set(false)} />
            <Button testID="cc-save" variant="primary" label={saving() ? "Đang lưu…" : "Lưu chiến dịch"}
              disabled={saving()} onPress={() => { void save(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={deleting() !== null} title="Xoá chiến dịch?" onClose={() => deleting.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-foreground">{`Xoá "${deleting()?.name ?? ""}" và mọi dữ liệu phân tích của nó.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-delete-cancel" label="Huỷ" onPress={() => deleting.set(null)} />
            <Button testID="cc-delete-confirm" label="Xoá" variant="primary" onPress={() => { void confirmDelete(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={analyticsFor() !== null} title={`Báo cáo: ${analyticsFor()?.name ?? ""}`}
        onClose={() => analyticsFor.set(null)} class="m-auto w-[640px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="gap-3">
          <If when={analytics() === null}><Skeleton class="h-24" /></If>
          <If when={analytics()}>
            {(a) => (
              <View class="gap-3">
                <View class="flex-row flex-wrap gap-2">
                  <Metric testID="cc-an-enrolled" label="Ghi danh" value={String(a().total_enrolled)} />
                  <Metric testID="cc-an-sent" label="Đã gửi" value={String(a().sent_count + a().delivered_count)} />
                  <Metric testID="cc-an-replied" label="Phản hồi" value={`${a().replied_count} · ${percent(a().response_rate)}`} />
                  <Metric testID="cc-an-converted" label="Chốt deal" value={`${a().converted_count} · ${percent(a().conversion_rate)}`} />
                </View>
                <If when={a().step_breakdown.length === 0}>
                  <Text class="text-sm text-muted-foreground">Chưa có bước nào để thống kê.</Text>
                </If>
                <Each of={a().step_breakdown} key={(s) => String(s.step_id)}>
                  {(s) => (
                    <Text testID={`cc-an-step-${s().step_order}`} class="text-sm text-foreground">
                      {`Bước ${s().step_order} (${s().channel}): chờ ${s().pending} · gửi ${s().sent} · tới ${s().delivered} · phản hồi ${s().replied} · lỗi ${s().failed} · bỏ ${s().skipped}`}
                    </Text>
                  )}
                </Each>
              </View>
            )}
          </If>
          <View class="flex-row justify-end"><Button testID="cc-an-close" label="Đóng" onPress={() => analyticsFor.set(null)} /></View>
        </View>
      </Modal>

      <Modal open={enrolFor() !== null} title={`Ghi danh vào: ${enrolFor()?.name ?? ""}`}
        onClose={() => enrolFor.set(null)} class="m-auto h-[560px] w-[620px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="flex-1 gap-3">
          <Input testID="cc-enrol-search" value={search()} placeholder="Tìm theo tên, SĐT, công ty"
            onValue={(v) => { search.set(v); void loadLeads(); }} />
          <View class="flex-row flex-wrap gap-1">
            <Button testID="cc-enrol-cat-all" label="Tất cả" size="sm" variant={categoryId() === null ? "primary" : "secondary"}
              onPress={() => { categoryId.set(null); void loadLeads(); }} />
            <Each of={categories()} key={(c) => String(c.id)}>
              {(c) => (
                <Button testID={`cc-enrol-cat-${c().id}`} label={c().name} size="sm"
                  variant={categoryId() === c().id ? "primary" : "secondary"}
                  onPress={() => { categoryId.set(c().id); void loadLeads(); }} />
              )}
            </Each>
          </View>
          <View class="flex-row items-center gap-2">
            <Button testID="cc-enrol-all" label={`Chọn tất cả (${leads().length})`} size="sm"
              onPress={() => chosen.set([...new Set([...chosen(), ...leads().map((l) => l.id)])])} />
            <Button testID="cc-enrol-none" label="Bỏ chọn" size="sm" variant="ghost" onPress={() => chosen.set([])} />
          </View>
          <Scroll class="h-[300px]">
            <View class="gap-1">
              <If when={!leadsLoading() && leads().length === 0}>
                <Text class="text-sm text-muted-foreground">Không có khách phù hợp.</Text>
              </If>
              <Each of={leads()} key={(l) => String(l.id)}>
                {(l) => (
                  <Checkbox testID={`cc-enrol-lead-${l().id}`} checked={chosen().includes(l().id)}
                    label={`${l().name || "Không tên"} · ${l().phone || "không SĐT"} · ${l().company || "Cá nhân"}`}
                    onChange={(on) => toggleLead(l().id, on)} />
                )}
              </Each>
            </View>
          </Scroll>
          <View class="flex-row items-center justify-between">
            <Text testID="cc-enrol-count" class="text-sm text-muted-foreground">{`Đã chọn ${chosen().length} khách`}</Text>
            <View class="flex-row gap-2">
              <Button testID="cc-enrol-cancel" label="Huỷ" onPress={() => enrolFor.set(null)} />
              <Button testID="cc-enrol-confirm" variant="primary" label={enrolling() ? "Đang xử lý…" : `Ghi danh (${chosen().length})`}
                disabled={chosen().length === 0 || enrolling()} onPress={() => { void confirmEnrol(); }} />
            </View>
          </View>
        </View>
      </Modal>
    </View>
  );
}
