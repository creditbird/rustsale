import { computed, List, Scroll, signal, Text, View } from "nsh";
import { Deals, Leads, LeadsMore } from "rustsale/native";
import { Button as Base } from "nsh/base";
import { Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { money, type Lead } from "../components/LeadDetail";
import { startCore, type Page } from "../stores/core";
import { notify, report } from "../stores/toast";
import { group, neighbour, OTHER, STAGES, totalOf, type Deal } from "../stores/pipeline";

const COLUMNS = [...STAGES.map((s) => ({ key: s.key, label: s.label, hint: s.prob })), { key: OTHER, label: "Khác", hint: "" }];

/**
 * Pipeline: a board with one column per stage. A deal moves with the ‹ › buttons on its card, or
 * with ← → (or [ ]) while the card has the keyboard focus; Delete asks to remove it.
 * Drag and drop is not used: see docs/v3/screen-port-plan.md §6.
 */
export function PipelineScreen() {
  const deals = signal<Deal[]>([]);
  const leads = signal<Map<number, Lead>>(new Map());
  const ready = signal(false);
  const search = signal("");
  const busy = signal(false);
  const removing = signal<Deal | null>(null);

  const creating = signal(false);
  const leadQuery = signal("");
  const leadRows = signal<Lead[]>([]);
  const picked = signal<Lead | null>(null);
  const title = signal("");
  const value = signal("10000000");
  const currency = signal("VND");
  const stage = signal(STAGES[0].key);
  const formError = signal<string | null>(null);
  const saving = signal(false);

  const columns = computed(() => group(deals(), search(), leads()));

  const load = async () => {
    try {
      await startCore();
      const [d, l] = await Promise.all([Deals.list(), LeadsMore.list()]);
      deals.set(d as Deal[]);
      leads.set(new Map((l as Lead[]).map((x) => [x.id, x])));
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };

  const move = async (deal: Deal, to: string | null) => {
    if (to === null || to === deal.stage || busy()) return;
    busy.set(true);
    try {
      await Deals.updateStage(deal.id, to);
      notify("Đã chuyển deal", "success");
    } catch (e) {
      report(e, { refresh: () => { void load(); }, retry: () => { void move(deal, to); } });
    } finally {
      busy.set(false);
    }
    await load();
  };

  const confirmRemove = async () => {
    const deal = removing();
    removing.set(null);
    if (!deal) return;
    try {
      await Deals.delete(deal.id);
      notify("Đã xoá deal", "success");
    } catch (e) {
      report(e, { refresh: () => { void load(); }, retry: () => { removing.set(deal); } });
    }
    await load();
  };

  const searchLeads = async (q: string) => {
    leadQuery.set(q);
    picked.set(null);
    if (q.trim() === "") { leadRows.set([]); return; }
    try {
      await startCore();
      const page = (await Leads.page({ search: q.trim(), sort: "name" }, null, 6)) as Page<Lead>;
      if (leadQuery() === q) leadRows.set(page.items);
    } catch (e) {
      if (leadQuery() === q) report(e, { retry: () => { void searchLeads(q); } });
    }
  };

  const pick = (lead: Lead) => {
    picked.set(lead);
    leadQuery.set(`${lead.name}${lead.company ? ` (${lead.company})` : ""}`);
    leadRows.set([]);
    if (title().trim() === "") title.set(`Deal - ${lead.company || lead.name}`);
  };

  const openCreate = (at: string) => {
    leadQuery.set("");
    leadRows.set([]);
    picked.set(null);
    title.set("");
    value.set("10000000");
    currency.set("VND");
    stage.set(at === OTHER ? STAGES[0].key : at);
    formError.set(null);
    creating.set(true);
  };

  const submitCreate = async () => {
    if (saving()) return;
    const lead = picked();
    if (lead === null) { formError.set("Chọn khách hàng cho deal này."); return; }
    if (title().trim() === "") { formError.set("Cần nhập tên deal."); return; }
    const amount = Number(value().replace(/[\s,.]/g, ""));
    if (!Number.isFinite(amount) || amount < 0) { formError.set("Giá trị phải là số, không âm."); return; }
    saving.set(true);
    try {
      await Deals.create({ leadId: lead.id, title: title().trim(), value: amount, stage: stage(), currency: currency() });
      creating.set(false);
      notify("Đã tạo deal", "success");
      await load();
    } catch (e) {
      const f = report(e, { retry: () => { void submitCreate(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      saving.set(false);
    }
  };

  void load();

  return (
    <View class="flex-1 gap-3 p-6">
      <View class="flex-row items-center justify-between gap-3">
        <View>
          <Text class="text-xl font-semibold text-foreground">Pipeline</Text>
          <Text testID="pipe-caption" class="text-sm text-muted-foreground">
            {ready() ? `${deals().length} deal` : "Đang mở…"}
          </Text>
        </View>
        <View class="w-72"><Input testID="pipe-search" value={search()} placeholder="Tìm deal, khách, công ty…" onValue={(v) => search.set(v)} /></View>
        <Button testID="pipe-create" label="Tạo deal" variant="primary" onPress={() => openCreate(STAGES[0].key)} />
      </View>

      <If when={!ready()}><View class="gap-2"><Skeleton /><Skeleton /></View></If>
      <If when={ready()}>
        <Scroll horizontal class="flex-1">
          <View class="flex-1 flex-row gap-3">
            <Each of={COLUMNS} key={(c) => c.key}>
              {(col) => (
                <If when={col().key !== OTHER || (columns()[OTHER] ?? []).length > 0}>
                  <View testID={`pipe-col-${col().key}`} class="w-64 overflow-hidden rounded-(--radius) border border-border bg-muted">
                    <View class="gap-0.5 border-b border-border px-3 py-2">
                      <View class="flex-row items-center justify-between">
                        <Text class="text-sm font-semibold text-foreground">{`${col().label} · ${(columns()[col().key] ?? []).length}`}</Text>
                        <If when={col().key !== OTHER}>
                          <Button testID={`pipe-add-${col().key}`} label="+" size="sm" variant="ghost" onPress={() => openCreate(col().key)} />
                        </If>
                      </View>
                      <Text testID={`pipe-total-${col().key}`} class="text-xs text-muted-foreground">
                        {`${totalOf(columns()[col().key] ?? [])}${col().hint ? ` · xác suất ${col().hint}` : ""}`}
                      </Text>
                    </View>
                    <If when={(columns()[col().key] ?? []).length === 0}>
                      <EmptyState title="Chưa có deal" />
                    </If>
                    <List testID={`pipe-list-${col().key}`} each={columns()[col().key] ?? []} keyExtractor={(d) => String(d.id)} estimatedItemSize={112} class="flex-1">
                      {(row) => (
                        <View class="p-2">
                          <View
                            testID={`deal-${row().id}`}
                            onKeyDown={(e: { key: string }) => {
                              if (e.key === "ArrowLeft" || e.key === "[") void move(row(), neighbour(row().stage, -1));
                              else if (e.key === "ArrowRight" || e.key === "]") void move(row(), neighbour(row().stage, 1));
                              else if (e.key === "Delete" || e.key === "Backspace") removing.set(row());
                            }}
                            class="gap-1 rounded-(--radius) border border-border bg-card p-3"
                          >
                            <Base testID={`deal-focus-${row().id}`} accessibilityLabel={row().title} onPress={() => undefined} class="rounded-(--radius) focus-visible:ring-2 focus-visible:ring-ring">
                              <Text class="truncate text-sm font-medium text-card-foreground">{row().title}</Text>
                            </Base>
                            <Text class="truncate text-xs text-muted-foreground">
                              {`${leads().get(row().lead_id)?.name ?? "Khách đã xoá"}${leads().get(row().lead_id)?.company ? ` · ${leads().get(row().lead_id)?.company}` : ""}`}
                            </Text>
                            <Text class="text-sm font-medium text-foreground">{money(row().value, row().currency)}</Text>
                            <View class="flex-row items-center justify-between pt-1">
                              <View class="flex-row gap-1">
                                <Button testID={`deal-prev-${row().id}`} label="‹" size="sm" variant="ghost" disabled={neighbour(row().stage, -1) === null}
                                  onPress={() => { void move(row(), neighbour(row().stage, -1)); }} />
                                <Button testID={`deal-next-${row().id}`} label="›" size="sm" variant="ghost" disabled={neighbour(row().stage, 1) === null}
                                  onPress={() => { void move(row(), neighbour(row().stage, 1)); }} />
                              </View>
                              <Button testID={`deal-del-${row().id}`} label="Xoá" size="sm" variant="ghost" onPress={() => removing.set(row())} />
                            </View>
                          </View>
                        </View>
                      )}
                    </List>
                  </View>
                </If>
              )}
            </Each>
          </View>
        </Scroll>
      </If>

      <Modal open={creating()} title="Tạo deal" onClose={() => creating.set(false)}>
        <View class="gap-3">
          <Input testID="deal-lead" autoFocus value={leadQuery()} placeholder="Tìm khách hàng…" onValue={(v) => { void searchLeads(v); }} />
          <Each of={leadRows()} key={(l) => String(l.id)}>
            {(l) => (
              <Button testID={`deal-pick-${l().id}`} label={`${l().name}${l().company ? ` · ${l().company}` : ""}`} variant="ghost" onPress={() => pick(l())} />
            )}
          </Each>
          <Input testID="deal-title" value={title()} placeholder="Tên deal" onValue={(v) => title.set(v)} />
          <View class="flex-row items-center gap-2">
            <View class="flex-1"><Input testID="deal-value" value={value()} placeholder="Giá trị" onValue={(v) => value.set(v)} /></View>
            <Button testID="deal-cur-VND" label="VND" variant={currency() === "VND" ? "primary" : "secondary"} onPress={() => currency.set("VND")} />
            <Button testID="deal-cur-USD" label="USD" variant={currency() === "USD" ? "primary" : "secondary"} onPress={() => currency.set("USD")} />
          </View>
          <View class="flex-row flex-wrap gap-1">
            <Each of={STAGES} key={(s) => s.key}>
              {(s) => <Button testID={`deal-stage-${s().key}`} label={s().label} size="sm" variant={stage() === s().key ? "primary" : "secondary"} onPress={() => stage.set(s().key)} />}
            </Each>
          </View>
          <If when={formError() !== null}>
            <Text testID="deal-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => creating.set(false)} />
            <Button testID="deal-save" label="Lưu" variant="primary" onPress={() => { void submitCreate(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá deal?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.title ?? ""}” sẽ bị xoá. Khách hàng vẫn được giữ.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="deal-confirm-del" variant="destructive" label="Xoá" onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
