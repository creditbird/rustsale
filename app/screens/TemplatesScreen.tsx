import { Text, View, signal } from "nsh";
import { Button as Base } from "nsh/base";
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
  const error = signal<string | null>(null);
  const removing = signal<Template | null>(null);

  const refresh = async () => {
    try {
      await startCore();
      items.set((await Templates.list(null)) as Template[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void refresh(); } });
    }
  };

  const openForm = (t?: Template) => {
    name.set(t?.name ?? "");
    body.set(t?.content ?? "");
    channel.set(t?.channel ?? "zalo");
    error.set(null);
    editing.set(t ? t.id : "new");
  };
  const submit = async () => {
    const problem = validate(name(), body());
    if (problem) { error.set(problem); return; }
    const id = editing();
    const input = { name: name().trim(), content: body().trim(), channel: channel() };
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

  return (
    <View class="flex-1">
      <View class="flex-row items-center justify-between p-6">
        <Text class="text-xl font-semibold text-foreground">Thư viện Mẫu</Text>
        <Button testID="tpl-add" variant="primary" label="Thêm mẫu" onPress={() => openForm()} />
      </View>

      <If when={!ready()}><View class="gap-3 px-6"><Skeleton /><Skeleton /></View></If>
      <If when={ready() && items().length === 0}>
        <EmptyState title="Chưa có mẫu nào" hint="Bấm “Thêm mẫu” để tạo mẫu tin đầu tiên." />
      </If>
      <If when={items().length > 0}>
        <View class="gap-2 px-6" testID="tpl-list">
          <Each of={items()} key={(t) => String(t.id)}>
            {(t) => (
              <View class="flex-row items-center gap-3 rounded-(--radius) border border-border bg-card p-3">
                <View class="flex-1 gap-1">
                  <Text class="font-medium text-card-foreground">{t().name}</Text>
                  <Text class="truncate text-sm text-muted-foreground">{t().content}</Text>
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
