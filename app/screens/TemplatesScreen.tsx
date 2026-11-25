import { Text, View, signal } from "nsh";
import { Button as Base, Select } from "nsh/base";
import { Templates } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Modal, Skeleton } from "../ui";
import { startCore } from "../stores/core";
import { notify, report } from "../stores/toast";

export interface Template {
  id: number;
  name: string;
  channel: string;
  content: string;
  subject: string | null;
  usage_count: number;
  folder_id?: number | null;
}

export interface TemplateFolder {
  id: number;
  name: string;
  color: string | null;
}

export const CHANNELS = [
  { id: "zalo", label: "Zalo" },
  { id: "whatsapp", label: "WhatsApp" },
  { id: "email", label: "Email" },
] as const;

/** The message every field must satisfy before the core is asked, so the form answers at once. */
export function validate(name: string, content: string): string | null {
  if (name.trim() === "") return "Cần nhập tên mẫu.";
  if (content.trim() === "") return "Cần nhập nội dung mẫu.";
  return null;
}

/** Templates: list, add/edit form in a dialog with validation, and a delete confirm dialog. */
export function TemplatesScreen() {
  const items = signal<Template[]>([]);
  const ready = signal(false);
  const editing = signal<number | null | "new">(null);
  const name = signal("");
  const body = signal("");
  const channel = signal<string>("zalo");
  const folderId = signal<number | null>(null);
  const unfiled = signal(false);
  const formFolder = signal<number | null>(null);
  const folders = signal<TemplateFolder[]>([]);
  const folderOpen = signal(false);
  const folderName = signal("");
  const folderColor = signal("");
  const folderError = signal<string | null>(null);
  const error = signal<string | null>(null);
  const removing = signal<Template | null>(null);

  const refresh = async () => {
    try {
      await startCore();
      const current = folderId();
      const filter = unfiled()
        ? { unfiled: true }
        : current === null ? null : { folder_id: current };
      items.set((await Templates.list(filter)) as Template[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void refresh(); } });
    }
  };
  const loadFolders = async () => {
    try {
      await startCore();
      folders.set((await Templates.folders()) as TemplateFolder[]);
    } catch (e) {
      report(e, { retry: () => { void loadFolders(); } });
    }
  };
  const pickFolder = (id: number | null) => {
    unfiled.set(false);
    folderId.set(id);
    void refresh();
  };
  const pickUnfiled = () => {
    folderId.set(null);
    unfiled.set(true);
    void refresh();
  };
  const createFolder = async () => {
    if (folderName().trim() === "") { folderError.set("Cần nhập tên thư mục."); return; }
    folderError.set(null);
    try {
      await Templates.createFolder(folderName().trim(), folderColor().trim() || null);
      folderName.set("");
      folderColor.set("");
      notify("Đã tạo thư mục", "success");
      await loadFolders();
    } catch (e) {
      const f = report(e, { retry: () => { void createFolder(); } });
      if (f.action === "fix") folderError.set(f.message);
    }
  };
  const removeFolder = async (id: number) => {
    try {
      await Templates.deleteFolder(id);
      if (folderId() === id) folderId.set(null);
      notify("Đã xoá thư mục", "success");
      await Promise.all([loadFolders(), refresh()]);
    } catch (e) {
      report(e, { retry: () => { void removeFolder(id); } });
    }
  };
  const folderLabel = (id: number | null | undefined) => {
    if (id === null || id === undefined) return "—";
    return folders().find((folder) => folder.id === id)?.name ?? "—";
  };

  const openForm = (t?: Template) => {
    name.set(t?.name ?? "");
    body.set(t?.content ?? "");
    channel.set(t?.channel ?? "zalo");
    formFolder.set(t?.folder_id ?? folderId());
    error.set(null);
    editing.set(t ? t.id : "new");
  };
  const submit = async () => {
    const problem = validate(name(), body());
    if (problem) { error.set(problem); return; }
    const id = editing();
    const input = { name: name().trim(), content: body().trim(), channel: channel(), folder_id: formFolder() };
    try {
      if (typeof id === "number") await Templates.updateTemplate({ id, ...input });
      else await Templates.createTemplate(input);
      editing.set(null);
      notify("Đã lưu mẫu", "success");
      await refresh();
    } catch (e) {
      const f = report(e, { refresh: () => { editing.set(null); void refresh(); }, retry: () => { void submit(); } });
      if (f.action === "fix") error.set(f.message);
    }
  };
  const confirmRemove = async () => {
    const t = removing();
    removing.set(null);
    if (!t) return;
    try {
      await Templates.deleteTemplate(t.id);
      notify("Đã xoá mẫu", "success");
    } catch (e) {
      report(e, { retry: () => { removing.set(t); } });
    }
    await refresh();
  };

  void refresh();
  void loadFolders();

  return (
    <View class="flex-1">
      <View class="flex-row items-center justify-between p-6">
        <Text class="text-xl font-semibold text-foreground">Thư viện Mẫu</Text>
        <View class="flex-row items-center gap-2">
          <Button testID="tpl-folders-open" label="Quản lý thư mục" onPress={() => folderOpen.set(true)} />
          <Button testID="tpl-add" variant="primary" label="Thêm mẫu" onPress={() => openForm()} />
        </View>
      </View>

      <View class="flex-row flex-wrap items-center gap-2 px-6 pb-3">
        <Text class="text-xs text-muted-foreground">Thư mục</Text>
        <Button testID="tpl-folder-all" label="Tất cả" size="sm"
          variant={folderId() === null && !unfiled() ? "primary" : "secondary"} onPress={() => pickFolder(null)} />
        <Button testID="tpl-folder-unfiled" label="Chưa xếp" size="sm"
          variant={unfiled() ? "primary" : "secondary"} onPress={pickUnfiled} />
        <Each of={folders()} key={(f) => String(f.id)}>
          {(f) => (
            <Button testID={`tpl-folder-${f().id}`} label={f().name} size="sm"
              variant={folderId() === f().id && !unfiled() ? "primary" : "secondary"}
              onPress={() => pickFolder(f().id)} />
          )}
        </Each>
      </View>

      <If when={!ready()}><View class="gap-3 px-6"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && items().length === 0}>
        <EmptyState title="Chưa có mẫu nào" hint="Bấm “Thêm mẫu” để tạo mẫu tin đầu tiên." />
      </If>
      <If when={items().length > 0}>
        <View class="gap-2 px-6" testID="tpl-list">
          <Each of={items()} key={(t) => String(t.id)}>
            {(t) => (
              <View testID={`tpl-item-${t().id}`} class="flex-row items-center gap-3 rounded-(--radius) border border-border bg-card p-3">
                <View class="flex-1 gap-1">
                  <Text class="font-medium text-card-foreground">{t().name}</Text>
                  <Text class="truncate text-sm text-muted-foreground">{t().content}</Text>
                  <Text class="text-xs text-muted-foreground">{folderLabel(t().folder_id)}</Text>
                </View>
                <Badge label={CHANNELS.find((c) => c.id === t().channel)?.label ?? t().channel} tone="neutral" />
                <Button size="sm" variant="ghost" label="Sửa" testID={`tpl-edit-${t().id}`} onPress={() => openForm(t())} />
                <Button size="sm" variant="ghost" label="Xóa" testID={`tpl-del-${t().id}`} onPress={() => removing.set(t())} />
              </View>
            )}
          </Each>
        </View>
      </If>

      <Modal open={editing() !== null} title={editing() === "new" ? "Thêm mẫu" : "Sửa mẫu"} onClose={() => editing.set(null)}>
        <View class="gap-3">
          <Input testID="tpl-name" value={name()} onValue={(v) => name.set(v)} placeholder="Tên mẫu" />
          <Input testID="tpl-body" value={body()} onValue={(v) => body.set(v)} placeholder="Nội dung" onSubmit={() => { void submit(); }} />
          <View class="w-64">
            <Select testID="tpl-folder"
              items={[{ value: "", title: "— Không thư mục —" },
                ...folders().map((f) => ({ value: String(f.id), title: f.name }))]}
              value={formFolder() === null ? "" : String(formFolder())}
              placeholder="— Không thư mục —"
              onChange={({ value }) => formFolder.set(value === null || value === "" ? null : Number(value))}
              trigger={
                <View testID="tpl-folder-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">{formFolder() === null ? "— Không thư mục —" : folderLabel(formFolder())}</Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={`tpl-folder-item-${entry.value === "" ? "none" : entry.value}`}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
          <View class="flex-row gap-2">
            <Each of={CHANNELS} key={(c) => c.id}>
              {(c) => (
                <Base
                  testID={`tpl-channel-${c().id}`}
                  accessibilityLabel={c().label}
                  onPress={() => channel.set(c().id)}
                  class={`rounded-full border px-3 py-1 ${channel() === c().id ? "border-primary bg-primary" : "border-border bg-card"}`}
                >
                  <Text class={`text-sm ${channel() === c().id ? "text-primary-foreground" : "text-foreground"}`}>{c().label}</Text>
                </Base>
              )}
            </Each>
          </View>
          <If when={error() !== null}>
            <Text testID="tpl-error" class="text-sm text-destructive">{error() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button label="Hủy" onPress={() => editing.set(null)} />
            <Button testID="tpl-save" variant="primary" label="Lưu" onPress={() => { void submit(); }} />
          </View>
        </View>
      </Modal>

      <Modal open={folderOpen()} title="Thư mục mẫu tin" onClose={() => folderOpen.set(false)}>
        <View class="gap-3">
          <View class="flex-row items-end gap-2">
            <View class="flex-1">
              <Input testID="tpl-folder-new-name" value={folderName()} placeholder="Tên thư mục"
                onValue={(v) => folderName.set(v)} />
            </View>
            <View class="w-24">
              <Input testID="tpl-folder-new-color" value={folderColor()} placeholder="#22c55e"
                onValue={(v) => folderColor.set(v)} />
            </View>
            <Button testID="tpl-folder-create" label="Tạo" variant="primary" onPress={() => { void createFolder(); }} />
          </View>
          <If when={folderError() !== null}>
            <Text testID="tpl-folder-error" class="text-sm text-destructive">{folderError() ?? ""}</Text>
          </If>
          <If when={folders().length === 0}>
            <Text class="text-sm text-muted-foreground">Chưa có thư mục.</Text>
          </If>
          <Each of={folders()} key={(f) => String(f.id)}>
            {(f) => (
              <View class="flex-row items-center gap-2">
                <Text testID={`tpl-folder-row-${f().id}`} class="min-w-0 flex-1 truncate text-sm text-foreground">{f().name}</Text>
                <Button testID={`tpl-folder-del-${f().id}`} label="Xoá" size="sm" variant="ghost"
                  onPress={() => { void removeFolder(f().id); }} />
              </View>
            )}
          </Each>
          <View class="flex-row justify-end">
            <Button testID="tpl-folders-close" label="Đóng" onPress={() => folderOpen.set(false)} />
          </View>
        </View>
      </Modal>

      <Modal open={removing() !== null} title="Xóa mẫu?" onClose={() => removing.set(null)}>
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">{`“${removing()?.name ?? ""}” sẽ bị xóa.`}</Text>
          <View class="flex-row justify-end gap-2">
            <Button label="Giữ lại" onPress={() => removing.set(null)} />
            <Button testID="tpl-confirm-del" variant="destructive" label="Xóa" onPress={() => { void confirmRemove(); }} />
          </View>
        </View>
      </Modal>
    </View>
  );
}
