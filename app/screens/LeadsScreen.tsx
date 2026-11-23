import { List, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Leads, LeadsMore } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { LeadDetail, STATUSES, money, tone, type Activity, type Lead } from "../components/LeadDetail";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { notify, report } from "../stores/toast";

const SORTS: { key: string; label: string; w: string }[] = [
  { key: "name", label: "Tên", w: "flex-[2]" },
  { key: "company", label: "Công ty", w: "flex-[2]" },
  { key: "status", label: "Trạng thái", w: "flex-1" },
  { key: "owner", label: "Phụ trách", w: "flex-1" },
  { key: "value", label: "Giá trị", w: "flex-1" },
];

/** Leads: a cursor-paged table filtered and sorted in the core, a detail panel and an add dialog. */
export function LeadsScreen() {
  const search = signal("");
  const status = signal("");
  const sort = signal("name");
  const desc = signal(false);
  const selected = signal<Lead | null>(null);
  const activities = signal<Activity[]>([]);
  const adding = signal(false);
  const newName = signal("");
  const newCompany = signal("");
  const formError = signal<string | null>(null);
  const failed = signal(false);

  const list = pager<Lead>(
    async (cursor) => {
      await startCore();
      const query = {
        search: search().trim() || undefined,
        status: status() || undefined,
        sort: sort(),
        descending: desc(),
      };
      return (await Leads.page(query, cursor, 50)) as Page<Lead>;
    },
    (e, retry) => { failed.set(true); report(e, { retry }); },
  );
  const reload = () => { failed.set(false); void list.reload(); };

  let timer: ReturnType<typeof setTimeout> | null = null;
  const onSearch = (v: string) => {
    search.set(v);
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(reload, 200);
  };
  const setSort = (key: string) => {
    if (sort() === key) desc.set(!desc());
    else { sort.set(key); desc.set(false); }
    reload();
  };

  const open = async (lead: Lead) => {
    selected.set(lead);
    activities.set([]);
    try {
      const [fresh, acts] = await Promise.all([Leads.details(lead.id), LeadsMore.activities(lead.id)]);
      if (selected()?.id === lead.id) {
        selected.set(fresh as Lead);
        activities.set(acts as Activity[]);
      }
    } catch (e) {
      report(e, { refresh: () => { selected.set(null); reload(); }, retry: () => { void open(lead); } });
    }
  };
  const setStatus = async (lead: Lead, next: string) => {
    try {
      await Leads.updateStatus(lead.id, next);
      notify(`Đã chuyển sang ${next}`, "success");
      await open(lead);
      reload();
    } catch (e) {
      report(e, { refresh: () => { selected.set(null); reload(); }, retry: () => { void setStatus(lead, next); } });
    }
  };
  const submitAdd = async () => {
    if (newName().trim() === "") { formError.set("Cần nhập tên khách."); return; }
    try {
      await Leads.add({ name: newName().trim(), company: newCompany().trim() || undefined, status: "New" });
      adding.set(false);
      newName.set(""); newCompany.set("");
      notify("Đã thêm khách", "success");
      reload();
    } catch (e) {
      const f = report(e, { retry: () => { void submitAdd(); } });
      if (f.action === "fix") formError.set(f.message);
    }
  };

  reload();

  return (
    <View class="flex-1 flex-row">
      <View class="flex-1 gap-3 p-6">
        <View class="flex-row items-center justify-between">
          <Text class="text-xl font-semibold text-foreground">Khách hàng</Text>
          <View class="flex-row items-center gap-3">
            <Text testID="lead-caption" class="text-sm text-muted-foreground">{list.ready() ? `${list.items().length} / ${list.total()} khách` : (failed() ? "Chưa tải được" : "Đang mở dữ liệu…")}</Text>
            <Button testID="lead-add" label="Thêm khách" variant="primary" size="sm" onPress={() => { formError.set(null); adding.set(true); }} />
          </View>
        </View>
        <View class="flex-row flex-wrap items-center gap-2">
          <View class="w-72">
            <Input testID="lead-search" value={search()} placeholder="Tìm tên, công ty, SĐT…" onValue={onSearch} />
          </View>
          <Button testID="status-all" label="Tất cả" size="sm" variant={status() === "" ? "primary" : "secondary"}
            onPress={() => { status.set(""); reload(); }} />
          <Each of={STATUSES} key={(s) => s}>
            {(s) => (
              <Button testID={`status-${s()}`} label={s()} size="sm" variant={status() === s() ? "primary" : "secondary"}
                onPress={() => { status.set(s()); reload(); }} />
            )}
          </Each>
        </View>
        <View class="flex-1 overflow-hidden rounded-(--radius) border border-border bg-card">
          <View class="flex-row border-b border-border bg-muted px-4 py-2">
            <Each of={SORTS} key={(c) => c.key}>
              {(c) => (
                <Base testID={`sort-${c().key}`} class={c().w} onPress={() => setSort(c().key)}>
                  <Text class="text-xs font-medium text-muted-foreground">
                    {c().label + (sort() === c().key ? (desc() ? " ↓" : " ↑") : "")}
                  </Text>
                </Base>
              )}
            </Each>
          </View>
          <If when={!list.ready() && !failed()}><View class="gap-3 p-4"><Skeleton /><Skeleton /><Skeleton /></View></If>
          <If when={failed() && !list.ready()}>
            <EmptyState title="Chưa tải được danh sách" hint="Bấm Thử lại ở thông báo hoặc đổi bộ lọc." />
          </If>
          <If when={list.ready() && list.total() === 0}>
            <EmptyState title="Không có khách phù hợp" hint="Thử bỏ bớt bộ lọc hoặc đổi từ khoá." />
          </If>
          <List testID="lead-list" each={list.items()} keyExtractor={(l) => String(l.id)} estimatedItemSize={44}
            onLoadMore={() => { void list.more(); }} class="flex-1">
            {(row) => (
              <Base testID={`lead-row-${row().id}`} onPress={() => { void open(row()); }}
                class="flex-row items-center border-b border-border px-4 py-2.5 hover:bg-accent">
                <Text class="truncate flex-[2] text-sm text-foreground">{row().name}</Text>
                <Text class="truncate flex-[2] pr-3 text-sm text-muted-foreground">{row().company ?? ""}</Text>
                <View class="flex-1"><Badge label={row().status} tone={tone(row().status)} /></View>
                <Text class="truncate flex-1 text-sm text-muted-foreground">{row().owner ?? ""}</Text>
                <Text class="truncate flex-1 text-sm text-foreground">{money(row().value, row().currency)}</Text>
              </Base>
            )}
          </List>
        </View>
      </View>
      <If when={selected()}>
        {(lead) => (
          <LeadDetail
            lead={lead}
            activities={activities}
            onClose={() => selected.set(null)}
            onStatus={(next) => { void setStatus(lead(), next); }}
          />
        )}
      </If>
      <Modal open={adding()} title="Thêm khách" onClose={() => adding.set(false)}>
        <View class="gap-3">
          <Input testID="new-lead-name" value={newName()} placeholder="Tên khách" onValue={(v) => newName.set(v)} />
          <Input testID="new-lead-company" value={newCompany()} placeholder="Công ty" onValue={(v) => newCompany.set(v)} />
          <If when={formError()}>{(m) => <Text testID="lead-form-error" class="text-sm text-destructive">{m()}</Text>}</If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => adding.set(false)} />
            <Button testID="new-lead-save" label="Lưu" variant="primary" onPress={() => { void submitAdd(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
