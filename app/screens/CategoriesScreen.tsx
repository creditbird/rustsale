import { computed, List, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Categories, Leads } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { tone, type Lead } from "../components/LeadDetail";
import { startCore, type Page } from "../stores/core";
import { notify, report } from "../stores/toast";

interface Category { id: number; name: string }

const LEAD_COLUMNS: { label: string; w: string }[] = [
  { label: "Tên", w: "flex-[2]" },
  { label: "Công ty", w: "flex-[2]" },
  { label: "Điện thoại", w: "flex-1" },
  { label: "Trạng thái", w: "flex-1" },
];

/** Categories: the category list on the left, its leads on the right, with add, create and delete. */
export function CategoriesScreen() {
  const categories = signal<Category[]>([]);
  const ready = signal(false);
  const selectedId = signal<number | null>(null);
  const leads = signal<Lead[]>([]);
  const leadsReady = signal(false);
  const creating = signal(false);
  const newName = signal("");
  const formError = signal<string | null>(null);
  const removing = signal<Category | null>(null);
  const addSearch = signal("");
  const addRows = signal<Lead[]>([]);
  const adding = signal(false);
  const saving = signal(false);
  const removingLead = signal(false);

  const available = computed(() => addRows().filter((r) => !leads().some((l) => l.id === r.id)));

  const loadCategories = async () => {
    try {
      await startCore();
      categories.set((await Categories.list()) as Category[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void loadCategories(); } });
    }
  };

  const loadLeads = async (id: number) => {
    leadsReady.set(false);
    try {
      const rows = (await Categories.leads(id)) as Lead[];
      if (selectedId() !== id) return;
      leads.set(rows);
      leadsReady.set(true);
    } catch (e) {
      if (selectedId() !== id) return;
      report(e, {
        refresh: () => { selectedId.set(null); leads.set([]); void loadCategories(); },
        retry: () => { void loadLeads(id); },
      });
    }
  };

  const select = (category: Category) => {
    selectedId.set(category.id);
    leads.set([]);
    addSearch.set("");
    addRows.set([]);
    void loadLeads(category.id);
  };

  const openCreate = () => {
    newName.set("");
    formError.set(null);
    creating.set(true);
  };
  const submitCreate = async () => {
    if (saving()) return;
    if (newName().trim() === "") { formError.set("Cần nhập tên danh mục."); return; }
    saving.set(true);
    try {
      await Categories.create(newName().trim());
      creating.set(false);
      notify("Đã tạo danh mục", "success");
      await loadCategories();
    } catch (e) {
      const f = report(e, { retry: () => { void submitCreate(); } });
      if (f.action === "fix") formError.set(f.message);
    } finally {
      saving.set(false);
    }
  };
  const confirmRemove = async () => {
    const category = removing();
    removing.set(null);
    if (!category) return;
    try {
      await Categories.delete(category.id);
      notify("Đã xoá danh mục", "success");
    } catch (e) {
      report(e, {
        refresh: () => { void loadCategories(); },
        retry: () => { removing.set(category); },
      });
    }
    if (selectedId() === category.id) {
      selectedId.set(null);
      leads.set([]);
    }
    await loadCategories();
  };

  const searchAdd = (value: string) => {
    addSearch.set(value);
    void loadAddRows(value);
  };
  const loadAddRows = async (query: string) => {
    if (query.trim() === "") { addRows.set([]); return; }
    try {
      await startCore();
      const page = (await Leads.page({ search: query.trim(), sort: "name" }, null, 20)) as Page<Lead>;
      if (addSearch() === query) addRows.set(page.items);
    } catch (e) {
      if (addSearch() === query) report(e, { retry: () => { void loadAddRows(query); } });
    }
  };
  const addLead = async (lead: Lead) => {
    const id = selectedId();
    if (id === null || adding()) return;
    adding.set(true);
    try {
      await Categories.addLeads([lead.id], id);
      notify("Đã thêm lead vào danh mục", "success");
      addRows.set(addRows().filter((r) => r.id !== lead.id));
      await loadLeads(id);
    } catch (e) {
      report(e, {
        refresh: () => { void loadLeads(id); },
        retry: () => { void addLead(lead); },
      });
    } finally {
      adding.set(false);
    }
  };
  const removeLead = async (lead: Lead) => {
    const id = selectedId();
    if (id === null || removingLead()) return;
    removingLead.set(true);
    try {
      await Categories.removeLeads([lead.id], id);
      notify("Đã bỏ lead khỏi danh mục", "success");
      await loadLeads(id);
    } catch (e) {
      report(e, {
        refresh: () => { void loadLeads(id); },
        retry: () => { void removeLead(lead); },
      });
    } finally {
      removingLead.set(false);
    }
  };

  void loadCategories();

  return (
    <View class="flex-1 flex-row">
      <View class="w-72 gap-2 border-r border-border p-4">
        <View class="flex-row items-center justify-between">
          <Text class="text-lg font-semibold text-foreground">Danh mục</Text>
          <Button testID="cat-create" label="Tạo" variant="primary" size="sm" onPress={openCreate} />
        </View>
        <View class="flex-1 overflow-hidden rounded-(--radius) border border-border bg-card">
          <If when={!ready()}><View class="gap-2 p-3"><Skeleton /><Skeleton /></View></If>
          <If when={ready() && categories().length === 0}>
            <EmptyState title="Chưa có danh mục" hint="Bấm “Tạo” để thêm danh mục đầu tiên." />
          </If>
          <List testID="cat-list" each={categories()} keyExtractor={(c) => String(c.id)} estimatedItemSize={40} class="flex-1">
            {(row) => (
              <View testID={`cat-row-${row().id}`}
                class={`flex-row items-center border-b border-border px-2 py-1 ${selectedId() === row().id ? "bg-accent" : ""}`}>
                <Base testID={`cat-open-${row().id}`} onPress={() => select(row())}
                  class="flex-1 flex-row items-center gap-2 rounded-(--radius) px-2 py-1.5 hover:bg-accent">
                  <Text class="truncate text-sm text-foreground">{row().name}</Text>
                </Base>
                <Button testID={`cat-del-${row().id}`} label="Xoá" size="sm" variant="ghost" onPress={() => removing.set(row())} />
              </View>
            )}
          </List>
        </View>
      </View>

      <View class="flex-1 gap-3 p-6">
        <If when={selectedId() === null}>
          <EmptyState title="Chọn một danh mục" hint="Chọn danh mục bên trái để xem lead, hoặc tạo danh mục mới." />
        </If>
        <If when={selectedId() !== null}>
          <View class="flex-1 gap-3">
            <View class="flex-row items-center justify-between">
              <Text testID="cat-detail-name" class="text-xl font-semibold text-foreground">
                {categories().find((c) => c.id === selectedId())?.name ?? ""}
              </Text>
              <Text testID="cat-caption" class="text-sm text-muted-foreground">
                {leadsReady() ? `${leads().length} lead` : "Đang mở…"}
              </Text>
            </View>

            <View class="gap-2 rounded-(--radius) border border-border bg-card p-3">
              <Text class="text-xs font-medium text-muted-foreground">Thêm lead vào danh mục</Text>
              <Input testID="cat-add-search" value={addSearch()} placeholder="Tìm lead theo tên, công ty, SĐT…" onValue={searchAdd} />
              <If when={addSearch().trim() !== ""}>
                <View class="gap-1">
                  <If when={available().length === 0}>
                    <Text class="text-sm text-muted-foreground">Không còn lead nào để thêm.</Text>
                  </If>
                  <Each of={available()} key={(l) => String(l.id)}>
                    {(l) => (
                      <View class="flex-row items-center gap-2">
                        <Text class="flex-1 truncate text-sm text-foreground">{l().name}</Text>
                        <Text class="truncate text-sm text-muted-foreground">{l().company ?? ""}</Text>
                        <Button testID={`cat-add-${l().id}`} label="Thêm" size="sm" onPress={() => { void addLead(l()); }} />
                      </View>
                    )}
                  </Each>
                </View>
              </If>
            </View>

            <View class="flex-1 overflow-hidden rounded-(--radius) border border-border bg-card">
              <View class="flex-row border-b border-border bg-muted px-4 py-2">
                <Each of={LEAD_COLUMNS} key={(c) => c.label}>
                  {(c) => (
                    <View class={c().w}>
                      <Text class="text-xs font-medium text-muted-foreground">{c().label}</Text>
                    </View>
                  )}
                </Each>
                <View class="w-16" />
              </View>
              <If when={!leadsReady()}><View class="gap-3 p-4"><Skeleton /><Skeleton /></View></If>
              <If when={leadsReady() && leads().length === 0}>
                <EmptyState title="Danh mục chưa có lead" hint="Dùng ô tìm phía trên để thêm lead." />
              </If>
              <List testID="cat-leads" each={leads()} keyExtractor={(l) => String(l.id)} estimatedItemSize={44} class="flex-1">
                {(row) => (
                  <View testID={`cat-lead-${row().id}`} class="flex-row items-center border-b border-border px-4 py-2.5">
                    <Text class="truncate flex-[2] text-sm text-foreground">{row().name}</Text>
                    <Text class="truncate flex-[2] pr-3 text-sm text-muted-foreground">{row().company ?? ""}</Text>
                    <Text class="truncate flex-1 text-sm text-muted-foreground">{row().phone ?? "—"}</Text>
                    <View class="flex-1"><Badge label={row().status} tone={tone(row().status)} /></View>
                    <View class="w-16 items-end">
                      <Button testID={`cat-remove-${row().id}`} label="Bỏ" size="sm" variant="ghost" onPress={() => { void removeLead(row()); }} />
                    </View>
                  </View>
                )}
              </List>
            </View>
          </View>
        </If>
      </View>

      <Modal open={creating()} title="Tạo danh mục" onClose={() => creating.set(false)}>
        <View class="gap-3">
          <Input testID="cat-name" value={newName()} placeholder="Tên danh mục" onValue={(v) => newName.set(v)} onSubmit={() => { void submitCreate(); }} />
          <If when={formError() !== null}>
            <Text testID="cat-error" class="text-sm text-destructive">{formError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Huỷ" onPress={() => creating.set(false)} />
            <Button testID="cat-save" label="Lưu" variant="primary" onPress={() => { void submitCreate(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xoá danh mục?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.name ?? ""}” sẽ bị xoá. Lead vẫn được giữ.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="cat-confirm-del" variant="destructive" label="Xoá" onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
