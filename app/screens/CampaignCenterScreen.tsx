import { Scroll, signal, Text, View } from "nsh";
import { Campaigns, Leads, Templates } from "rustsale/native";
import { Badge, Button, Checkbox, EmptyState, ErrorState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { startCore, type Page } from "../stores/core";
import { notify, report } from "../stores/toast";
import {
  CHANNELS, CONDITIONS, DELAYS, PLACEHOLDERS, TEMPLATE_CHANNELS, TEMPLATE_STAGES,
  buildPayload, delayLabel, describeRemoved, filterTemplates, moveStep, newStep, parseTags, percent, previewSpintax, renumber,
  statusLabel, stepsToDrafts, stringifyTags, validateCampaign, validateTemplateDraft,
  type CampaignAnalytics, type RemovedStepImpact, type StepDraft, type StepRecord, type TemplateFolder, type TemplateRecord, type UnifiedCampaign,
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

  // editing: the campaign being edited (null = a new one) and where its stored steps are in loading
  const editing = signal<UnifiedCampaign | null>(null);
  const stepsState = signal<"ready" | "loading" | "error">("ready");
  let editGen = 0;

  const openBuilder = () => {
    editGen += 1;
    editing.set(null);
    stepsState.set("ready");
    name.set("");
    description.set("");
    steps.set([newStep(1)]);
    buildError.set(null);
    building.set(true);
  };
  const loadEditSteps = async (c: UnifiedCampaign) => {
    editGen += 1;
    const mine = editGen;
    stepsState.set("loading");
    buildError.set(null);
    try {
      await startCore();
      const rows = (await Campaigns.unifiedCampaignSteps(c.id)) as StepRecord[];
      if (mine !== editGen) return;
      steps.set(stepsToDrafts(rows));
      stepsState.set("ready");
    } catch (e) {
      if (mine !== editGen) return;
      stepsState.set("error");
      report(e, { retry: () => { void loadEditSteps(c); } });
    }
  };
  const openEditor = (c: UnifiedCampaign) => {
    editing.set(c);
    name.set(c.name);
    description.set(c.description ?? "");
    steps.set([]);
    building.set(true);
    void loadEditSteps(c);
  };
  const closeBuilder = () => {
    editGen += 1;
    building.set(false);
  };
  const patchStep = (i: number, patch: Partial<StepDraft>) =>
    steps.set(steps().map((s, k) => (k === i ? { ...s, ...patch } : s)));
  const addStep = () => steps.set([...steps(), newStep(steps().length + 1, steps()[steps().length - 1])]);
  const removeStep = (i: number) => steps.set(renumber(steps().filter((_, k) => k !== i)));

  // Saving an edit that removes a step with customers still waiting asks first; the body checked is the body saved.
  const removal = signal<{ body: ReturnType<typeof buildPayload>; impact: RemovedStepImpact[] } | null>(null);

  const commit = async (body: ReturnType<typeof buildPayload>) => {
    try {
      await Campaigns.saveUnifiedCampaign(body.campaign, body.steps);
      removal.set(null);
      closeBuilder();
      notify("Đã lưu chiến dịch", "success");
      await loadList();
    } catch (e) {
      removal.set(null);
      const f = report(e, { retry: () => { void save(); } });
      if (f.action === "fix") buildError.set(f.message);
    }
  };

  const save = async () => {
    if (saving()) return;
    if (stepsState() !== "ready") return;
    const problem = validateCampaign(name(), steps());
    if (problem !== null) { buildError.set(problem); return; }
    saving.set(true);
    buildError.set(null);
    try {
      const target = editing();
      const body = buildPayload(name(), description(), steps(), target ?? undefined);
      if (target !== null) {
        const impact = (await Campaigns.unifiedCampaignSaveImpact(target.id, body.steps)) as RemovedStepImpact[];
        if (impact.length > 0) {
          removal.set({ body, impact });
          return;
        }
      }
      await commit(body);
    } catch (e) {
      const f = report(e, { retry: () => { void save(); } });
      if (f.action === "fix") buildError.set(f.message);
    } finally {
      saving.set(false);
    }
  };
  const confirmRemoval = async () => {
    const pending = removal();
    if (pending === null || saving()) return;
    saving.set(true);
    try {
      await commit(pending.body);
    } finally {
      saving.set(false);
    }
  };

  // ---- templates (UI-35): the builder dialog is hidden while one of these is open, its draft stays in the signals
  const pickFor = signal<number | null>(null);
  const tplList = signal<TemplateRecord[]>([]);
  const tplFolders = signal<TemplateFolder[]>([]);
  const tplLoading = signal(false);
  const tplQuery = signal("");
  const tplChannel = signal("all");
  const tplFolder = signal<number | null>(null);
  const tplSelected = signal<number | null>(null);
  const tplPreview = signal("");
  const filtered = () => filterTemplates(tplList(), tplChannel(), tplFolder(), tplQuery());
  const selected = () => tplList().find((t) => t.id === tplSelected()) ?? null;

  const selectTpl = (t: TemplateRecord) => {
    tplSelected.set(t.id);
    tplPreview.set(previewSpintax(t.content));
  };
  const loadTemplates = async () => {
    tplLoading.set(true);
    try {
      const [list, folders] = await Promise.all([Templates.list(null), Templates.folders()]);
      tplList.set(list as TemplateRecord[]);
      tplFolders.set(folders as TemplateFolder[]);
    } catch (e) {
      report(e, { retry: () => { void loadTemplates(); } });
    } finally {
      tplLoading.set(false);
    }
  };
  const openPicker = (i: number) => {
    pickFor.set(i);
    tplQuery.set("");
    tplFolder.set(null);
    tplChannel.set(steps()[i]?.channel ?? "all");
    tplSelected.set(null);
    tplPreview.set("");
    void loadTemplates();
  };
  const chooseTpl = () => {
    const i = pickFor();
    const t = selected();
    if (i === null || t === null) return;
    patchStep(i, {
      template_content: t.content,
      template_id: t.id,
      ...(t.ai_prompt ? { ai_prompt_override: t.ai_prompt } : {}),
      ...(t.channel !== "omnichannel" ? { channel: t.channel } : {}),
    });
    pickFor.set(null);
    void Templates.incrementUsage(t.id).catch(() => undefined);
  };

  const saveTplFor = signal<number | null>(null);
  const tName = signal("");
  const tChannel = signal("zalo");
  const tFolder = signal<number | null>(null);
  const tStage = signal("cold");
  const tSubject = signal("");
  const tContent = signal("");
  const tMedia = signal("");
  const tAi = signal("");
  const tTags = signal("");
  const tNewFolder = signal("");
  const tError = signal<string | null>(null);
  const tSaving = signal(false);
  const tPreview = signal<string | null>(null);

  const openSaveTpl = (i: number) => {
    const s = steps()[i];
    if (s === undefined) return;
    tName.set("");
    tChannel.set(s.channel);
    tFolder.set(null);
    tStage.set("cold");
    tSubject.set("");
    tContent.set(s.template_content);
    tMedia.set("");
    tAi.set(s.ai_prompt_override ?? "");
    tTags.set("");
    tNewFolder.set("");
    tError.set(null);
    tPreview.set(null);
    saveTplFor.set(i);
    void (async () => {
      try {
        tplFolders.set((await Templates.folders()) as TemplateFolder[]);
      } catch (e) {
        report(e, {});
      }
    })();
  };
  const addFolder = async () => {
    const n = tNewFolder().trim();
    if (n === "") return;
    try {
      const id = Number(await Templates.createFolder(n, null));
      tplFolders.set((await Templates.folders()) as TemplateFolder[]);
      tFolder.set(id);
      tNewFolder.set("");
    } catch (e) {
      const f = report(e, {});
      if (f.action === "fix") tError.set(f.message);
    }
  };
  const submitTpl = async () => {
    if (tSaving()) return;
    const i = saveTplFor();
    const problem = validateTemplateDraft(tName(), tContent());
    if (problem !== null) { tError.set(problem); return; }
    if (i === null) return;
    tSaving.set(true);
    tError.set(null);
    try {
      const id = Number(await Templates.createTemplate({
        name: tName().trim(),
        channel: tChannel(),
        folder_id: tFolder(),
        stage: tStage(),
        subject: tSubject().trim() === "" ? null : tSubject().trim(),
        content: tContent().trim(),
        media_url: tMedia().trim() === "" ? null : tMedia().trim(),
        ai_prompt: tAi().trim() === "" ? null : tAi().trim(),
        tags_json: stringifyTags(tTags()),
      }));
      patchStep(i, { template_id: id, template_content: tContent().trim() });
      saveTplFor.set(null);
      notify("Đã lưu thành mẫu", "success");
    } catch (e) {
      const f = report(e, { retry: () => { void submitTpl(); } });
      if (f.action === "fix") tError.set(f.message);
    } finally {
      tSaving.set(false);
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
                  <Button testID={`cc-edit-${c().id}`} label="Sửa" size="sm" onPress={() => openEditor(c())} />
                  <Button testID={`cc-enrol-${c().id}`} label="Ghi danh" size="sm" onPress={() => { void openEnrol(c()); }} />
                  <Button testID={`cc-analytics-${c().id}`} label="Báo cáo" size="sm" onPress={() => { void openAnalytics(c()); }} />
                  <Button testID={`cc-delete-${c().id}`} label="Xoá" size="sm" variant="ghost" onPress={() => deleting.set(c())} />
                </View>
              </View>
            )}
          </Each>
        </View>
      </Scroll>

      <Modal open={building() && pickFor() === null && saveTplFor() === null} title={editing() === null ? "Kịch bản chiến dịch mới" : "Sửa kịch bản chiến dịch"} onClose={closeBuilder}
        class="m-auto h-[640px] w-[720px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="flex-1 gap-3">
          <Input testID="cc-name" value={name()} placeholder="Tên chiến dịch" onValue={(v) => name.set(v)} />
          <Input testID="cc-desc" value={description()} placeholder="Mô tả (không bắt buộc)" onValue={(v) => description.set(v)} />
          <Text class="text-xs text-muted-foreground">{`Biến dùng được: ${PLACEHOLDERS.join(" ")} · spintax: {Chào|Xin chào}`}</Text>
          <If when={stepsState() === "loading"}>
            <View testID="cc-steps-loading" class="gap-2"><Skeleton /><Skeleton /></View>
          </If>
          <If when={stepsState() === "error"}>
            <View class="flex-row items-center justify-between gap-2 rounded-(--radius) border border-border p-3">
              <Text testID="cc-steps-error" class="text-sm text-destructive">Chưa tải được các bước của chiến dịch.</Text>
              <Button testID="cc-steps-retry" label="Thử lại" size="sm"
                onPress={() => { const c = editing(); if (c !== null) void loadEditSteps(c); }} />
            </View>
          </If>
          <If when={stepsState() === "ready" && steps().length === 0}>
            <Text testID="cc-steps-empty" class="text-sm text-muted-foreground">Chiến dịch này chưa có bước nào. Thêm bước để lưu.</Text>
          </If>
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
                    <View class="flex-row flex-wrap gap-1">
                      <Button testID={`cc-step-${s().step_order}-pick`} label="Chọn mẫu" size="sm" variant="secondary"
                        onPress={() => openPicker(i())} />
                      <Button testID={`cc-step-${s().step_order}-savetpl`} label="Lưu thành mẫu" size="sm" variant="ghost"
                        disabled={s().template_content.trim() === ""} onPress={() => openSaveTpl(i())} />
                      <If when={s().template_id !== null}>
                        <Text testID={`cc-step-${s().step_order}-tplmark`} class="self-center text-xs text-muted-foreground">Đang dùng mẫu</Text>
                      </If>
                    </View>
                    <Input testID={`cc-step-${s().step_order}-ai`} value={s().ai_prompt_override ?? ""}
                      placeholder="Gợi ý cho AI (không bắt buộc)"
                      onValue={(v) => patchStep(i(), { ai_prompt_override: v.trim() === "" ? null : v })} />
                  </View>
                )}
              </Each>
            </View>
          </Scroll>
          <Button testID="cc-add-step" label="+ Thêm bước" size="sm" disabled={stepsState() !== "ready"} onPress={addStep} />
          <If when={buildError()}>
            {(m) => <Text testID="cc-build-error" class="text-sm text-destructive">{m()}</Text>}
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-build-cancel" label="Huỷ" disabled={saving()} onPress={closeBuilder} />
            <Button testID="cc-save" variant="primary" label={saving() ? "Đang lưu…" : "Lưu chiến dịch"}
              disabled={saving() || stepsState() !== "ready"} onPress={() => { void save(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={pickFor() !== null} title="Chọn mẫu tin nhắn" onClose={() => pickFor.set(null)}
        class="m-auto h-[640px] w-[760px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="flex-1 gap-3">
          <Input testID="cc-tpl-search" value={tplQuery()} placeholder="Tìm theo tên, nội dung, thẻ" onValue={(v) => tplQuery.set(v)} />
          <View class="flex-row flex-wrap gap-1">
            <Each of={TEMPLATE_CHANNELS} key={(c) => c.id}>
              {(c) => (
                <Button testID={`cc-tpl-ch-${c().id}`} label={c().label} size="sm"
                  variant={tplChannel() === c().id ? "primary" : "secondary"} onPress={() => tplChannel.set(c().id)} />
              )}
            </Each>
          </View>
          <View class="flex-row flex-wrap gap-1">
            <Button testID="cc-tpl-folder-all" label="Mọi thư mục" size="sm" variant={tplFolder() === null ? "primary" : "ghost"}
              onPress={() => tplFolder.set(null)} />
            <Each of={tplFolders()} key={(f) => String(f.id)}>
              {(f) => (
                <Button testID={`cc-tpl-folder-${f().id}`} label={f().name} size="sm"
                  variant={tplFolder() === f().id ? "primary" : "ghost"} onPress={() => tplFolder.set(f().id)} />
              )}
            </Each>
          </View>
          <Scroll class="h-[240px]">
            <View class="gap-1">
              <If when={tplLoading()}><Skeleton class="h-10" /></If>
              <If when={!tplLoading() && filtered().length === 0}>
                <Text testID="cc-tpl-empty" class="text-sm text-muted-foreground">Chưa có mẫu phù hợp.</Text>
              </If>
              <Each of={filtered()} key={(t) => String(t.id)}>
                {(t) => (
                  <Button testID={`cc-tpl-item-${t().id}`} size="sm" variant={tplSelected() === t().id ? "primary" : "secondary"}
                    label={`${t().name} · ${t().channel}${parseTags(t().tags_json).length > 0 ? ` · #${parseTags(t().tags_json).join(" #")}` : ""}`}
                    onPress={() => selectTpl(t())} />
                )}
              </Each>
            </View>
          </Scroll>
          <If when={selected()}>
            {(t) => (
              <View class="gap-1 rounded-(--radius) border border-border p-3">
                <View class="flex-row items-center justify-between">
                  <Text class="text-xs font-semibold text-foreground">Xem trước</Text>
                  <Button testID="cc-tpl-shuffle" label="Thử biến thể khác" size="sm" variant="ghost"
                    onPress={() => tplPreview.set(previewSpintax(t().content))} />
                </View>
                <If when={t().subject}>{(sub) => <Text class="text-xs text-muted-foreground">{`Tiêu đề: ${sub()}`}</Text>}</If>
                <Text testID="cc-tpl-preview" class="text-sm text-foreground">{tplPreview()}</Text>
              </View>
            )}
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-tpl-cancel" label="Huỷ" onPress={() => pickFor.set(null)} />
            <Button testID="cc-tpl-choose" variant="primary" label="Dùng mẫu này" disabled={selected() === null} onPress={chooseTpl} />
          </View>
        </View>
      </Modal>

      <Modal open={saveTplFor() !== null} title="Lưu thành mẫu" onClose={() => saveTplFor.set(null)}
        class="m-auto h-[680px] w-[680px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
        <View class="flex-1 gap-2">
          <Scroll class="h-[540px]">
            <View class="gap-2">
              <Input testID="cc-st-name" value={tName()} placeholder="Tên mẫu" onValue={(v) => tName.set(v)} />
              <View class="flex-row flex-wrap gap-1">
                <Each of={TEMPLATE_CHANNELS.filter((c) => c.id !== "all")} key={(c) => c.id}>
                  {(c) => (
                    <Button testID={`cc-st-ch-${c().id}`} label={c().label} size="sm"
                      variant={tChannel() === c().id ? "primary" : "secondary"} onPress={() => tChannel.set(c().id)} />
                  )}
                </Each>
              </View>
              <View class="flex-row flex-wrap gap-1">
                <Each of={TEMPLATE_STAGES} key={(c) => c.id}>
                  {(c) => (
                    <Button testID={`cc-st-stage-${c().id}`} label={c().label} size="sm"
                      variant={tStage() === c().id ? "primary" : "ghost"} onPress={() => tStage.set(c().id)} />
                  )}
                </Each>
              </View>
              <View class="flex-row flex-wrap gap-1">
                <Button testID="cc-st-folder-none" label="Không thư mục" size="sm" variant={tFolder() === null ? "primary" : "ghost"}
                  onPress={() => tFolder.set(null)} />
                <Each of={tplFolders()} key={(f) => String(f.id)}>
                  {(f) => (
                    <Button testID={`cc-st-folder-${f().id}`} label={f().name} size="sm"
                      variant={tFolder() === f().id ? "primary" : "ghost"} onPress={() => tFolder.set(f().id)} />
                  )}
                </Each>
              </View>
              <View class="flex-row items-center gap-2">
                <View class="flex-1"><Input testID="cc-st-newfolder" value={tNewFolder()} placeholder="Thư mục mới" onValue={(v) => tNewFolder.set(v)} /></View>
                <Button testID="cc-st-addfolder" label="Tạo thư mục" size="sm" disabled={tNewFolder().trim() === ""} onPress={() => { void addFolder(); }} />
              </View>
              <If when={tChannel() === "email"}>
                <Input testID="cc-st-subject" value={tSubject()} placeholder="Tiêu đề email" onValue={(v) => tSubject.set(v)} />
              </If>
              <Input testID="cc-st-content" value={tContent()} multiline placeholder="Nội dung" onValue={(v) => { tContent.set(v); tPreview.set(null); }} />
              <View class="flex-row flex-wrap gap-1">
                <Each of={PLACEHOLDERS} key={(p) => p}>
                  {(p) => (
                    <Button testID={`cc-st-ph-${p()}`} label={p()} size="sm" variant="ghost"
                      onPress={() => tContent.set(`${tContent()}${tContent() === "" ? "" : " "}${p()}`)} />
                  )}
                </Each>
                <Button testID="cc-st-spin" label="+ Spintax" size="sm" variant="ghost"
                  onPress={() => tContent.set(`{Xin chào|Chào bạn|Kính chào} ${tContent()}`)} />
                <Button testID="cc-st-preview" label="Xem trước" size="sm" variant="secondary"
                  onPress={() => tPreview.set(previewSpintax(tContent()))} />
              </View>
              <If when={tPreview() !== null}>
                <Text testID="cc-st-preview-text" class="rounded-(--radius) border border-border p-2 text-sm text-foreground">{tPreview() ?? ""}</Text>
              </If>
              <Input testID="cc-st-media" value={tMedia()} placeholder="Đường dẫn ảnh/tệp (không bắt buộc)" onValue={(v) => tMedia.set(v)} />
              <Input testID="cc-st-ai" value={tAi()} placeholder="Gợi ý cho AI (không bắt buộc)" onValue={(v) => tAi.set(v)} />
              <Input testID="cc-st-tags" value={tTags()} placeholder="Thẻ, cách nhau bằng dấu phẩy" onValue={(v) => tTags.set(v)} />
            </View>
          </Scroll>
          <If when={tError()}>{(m) => <Text testID="cc-st-error" class="text-sm text-destructive">{m()}</Text>}</If>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-st-cancel" label="Huỷ" disabled={tSaving()} onPress={() => saveTplFor.set(null)} />
            <Button testID="cc-st-save" variant="primary" label={tSaving() ? "Đang lưu…" : "Lưu mẫu"} disabled={tSaving()}
              onPress={() => { void submitTpl(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removal() !== null} title="Bỏ bước đang có khách?" onClose={() => { if (!saving()) removal.set(null); }}>
        <View class="gap-3">
          <Each of={describeRemoved(removal()?.impact ?? [])} key={(line) => line}>
            {(line, i) => <Text testID={`cc-removal-line-${i()}`} class="text-sm text-foreground">{line()}</Text>}
          </Each>
          <Text class="text-sm text-muted-foreground">Việc gửi đang chờ của họ ở bước đó sẽ bị huỷ. Lịch sử đã gửi vẫn được giữ.</Text>
          <View class="flex-row justify-end gap-2">
            <Button testID="cc-removal-cancel" label="Quay lại sửa" disabled={saving()} onPress={() => removal.set(null)} />
            <Button testID="cc-removal-confirm" variant="primary" label={saving() ? "Đang lưu…" : "Vẫn lưu"} disabled={saving()}
              onPress={() => { void confirmRemoval(); }} />
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
