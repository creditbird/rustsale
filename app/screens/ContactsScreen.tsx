import { List, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Leads, LeadsMore } from "rustsale/native";
import { EmptyState, Input, Skeleton } from "../ui";
import { LeadDetail, money, tone, type Activity, type Lead } from "../components/LeadDetail";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { notify, report } from "../stores/toast";

const SORTS: { key: string; label: string; w: string }[] = [
  { key: "name", label: "Tên", w: "flex-[2]" },
  { key: "company", label: "Công ty", w: "flex-[2]" },
  { key: "owner", label: "Phụ trách", w: "flex-1" },
  { key: "value", label: "Giá trị", w: "flex-1" },
];

/** Contacts: the converted leads, searched and sorted in the core, with the shared detail panel. */
export function ContactsScreen() {
  const search = signal("");
  const sort = signal("name");
  const desc = signal(false);
  const selected = signal<Lead | null>(null);
  const activities = signal<Activity[]>([]);
  const failed = signal(false);

  const list = pager<Lead>(
    async (cursor) => {
      await startCore();
      const query = {
        search: search().trim() || undefined,
        status: "Converted",
        sort: sort(),
        descending: desc(),
      };
      return (await Leads.page(query, cursor, 50)) as Page<Lead>;
    },
    (e, retry) => { failed.set(true); report(e, { retry }); },
  );
  const reload = () => { failed.set(false); void list.reload(); };

  let timer: ReturnType<typeof setTimeout> | null = null;
  const onSearch = (value: string) => {
    search.set(value);
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
      if (next === "Converted") await open(lead);
      else selected.set(null);
      reload();
    } catch (e) {
      report(e, { refresh: () => { selected.set(null); reload(); }, retry: () => { void setStatus(lead, next); } });
    }
  };

  reload();

  return (
    <View class="flex-1 flex-row">
      <View class="flex-1 gap-3 p-6">
        <View class="flex-row items-center justify-between">
          <Text class="text-xl font-semibold text-foreground">Danh bạ</Text>
          <Text testID="contact-caption" class="text-sm text-muted-foreground">
            {list.ready() ? `${list.items().length} / ${list.total()} liên hệ` : (failed() ? "Chưa tải được" : "Đang mở dữ liệu…")}
          </Text>
        </View>
        <View class="flex-row flex-wrap items-center gap-2">
          <View class="w-72">
            <Input testID="contact-search" value={search()} placeholder="Tìm liên hệ…" onValue={onSearch} />
          </View>
        </View>
        <View class="flex-1 overflow-hidden rounded-(--radius) border border-border bg-card">
          <View class="flex-row border-b border-border bg-muted px-4 py-2">
            <Each of={SORTS} key={(c) => c.key}>
              {(c) => (
                <Base testID={`contact-sort-${c().key}`} class={c().w} onPress={() => setSort(c().key)}>
                  <Text class="text-xs font-medium text-muted-foreground">
                    {c().label + (sort() === c().key ? (desc() ? " ↓" : " ↑") : "")}
                  </Text>
                </Base>
              )}
            </Each>
          </View>
          <If when={!list.ready() && !failed()}><View class="gap-3 p-4"><Skeleton /><Skeleton /><Skeleton /></View></If>
          <If when={failed() && !list.ready()}>
            <EmptyState title="Chưa tải được danh sách" hint="Bấm Thử lại ở thông báo." />
          </If>
          <If when={list.ready() && list.total() === 0}>
            <EmptyState title="Không có liên hệ phù hợp" hint="Thử đổi từ khoá, hoặc chuyển một khách sang Converted." />
          </If>
          <List testID="contact-list" each={list.items()} keyExtractor={(l) => String(l.id)} estimatedItemSize={44}
            onLoadMore={() => { void list.more(); }} class="flex-1">
            {(row) => (
              <Base testID={`contact-row-${row().id}`} onPress={() => { void open(row()); }}
                class="flex-row items-center border-b border-border px-4 py-2.5 hover:bg-accent">
                <Text class="truncate flex-[2] text-sm text-foreground">{row().name}</Text>
                <Text class="truncate flex-[2] pr-3 text-sm text-muted-foreground">{row().company ?? ""}</Text>
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
    </View>
  );
}
