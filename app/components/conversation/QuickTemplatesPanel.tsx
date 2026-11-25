import { Scroll, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { ConversationActions } from "rustsale/native";
import { Badge, Button, Input, Modal } from "../../ui";
import {
  MAX_QUICK_TEMPLATES,
  rankQuickTemplates,
  validateQuickTemplate,
  type QuickTemplate,
} from "../../stores/conversationAdv";
import { notify, report } from "../../stores/toast";

// ---- 1. Inline Quick Templates Picker (above composer) ----

export interface QuickTemplatesPickerProps {
  open: () => boolean;
  templates: () => QuickTemplate[];
  query: () => string;
  onQuery: (q: string) => void;
  onSelect: (template: QuickTemplate) => void;
  onOpenManager: () => void;
  onClose: () => void;
}

export function QuickTemplatesPicker(props: QuickTemplatesPickerProps) {
  const ranked = () => rankQuickTemplates(props.templates(), props.query());

  return (
    <If when={props.open()}>
      <View
        testID="quick-template-popover"
        class="gap-2 border-t border-border bg-popover px-3 py-2"
      >
        <View class="flex-row items-center justify-between gap-2">
          <Text class="text-xs font-semibold text-popover-foreground">Mẫu trả lời nhanh</Text>
          <View class="flex-row items-center gap-1.5">
            <Button
              testID="manage-quick-templates-btn"
              size="sm"
              variant="secondary"
              label="Quản lý mẫu"
              onPress={props.onOpenManager}
            />
            <Button
              testID="close-quick-templates-btn"
              size="sm"
              variant="ghost"
              label="Đóng"
              onPress={props.onClose}
            />
          </View>
        </View>

        <Input
          testID="quick-tpl-picker-search"
          value={props.query()}
          onValue={props.onQuery}
          placeholder="Lọc theo phím tắt, tiêu đề, nội dung…"
        />

        <If when={ranked().length === 0}>
          <Text testID="quick-tpl-picker-empty" class="py-2 text-center text-xs text-muted-foreground">
            Không có mẫu trả lời nhanh phù hợp.
          </Text>
        </If>

        <If when={ranked().length > 0}>
          <Scroll class="max-h-[180px] rounded-(--radius) border border-border">
            <View class="gap-0.5 p-1">
              <Each of={ranked().slice(0, 50)} key={(t) => String(t.id)}>
                {(item) => (
                  <Base
                    testID={`quick-tpl-item-${item().id}`}
                    accessibilityLabel={`${item().shortcut} ${item().title}`}
                    onPress={() => props.onSelect(item())}
                    class="gap-0.5 rounded-(--radius) px-2.5 py-1.5 hover:bg-accent"
                  >
                    <View class="flex-row items-center justify-between gap-2">
                      <Text class="text-xs font-semibold text-primary">{`/${item().shortcut}`}</Text>
                      <If when={(item().category ?? "") !== ""}>
                        <Badge label={item().category ?? ""} tone="neutral" />
                      </If>
                    </View>
                    <Text class="truncate text-xs font-medium text-foreground">{item().title}</Text>
                    <Text class="truncate text-xs text-muted-foreground">{item().bodyText}</Text>
                  </Base>
                )}
              </Each>
            </View>
          </Scroll>
        </If>
      </View>
    </If>
  );
}

// ---- 2. Quick Templates Manager Modal (CRUD) ----

export interface QuickTemplateManagerModalProps {
  open: () => boolean;
  templates: () => QuickTemplate[];
  onClose: () => void;
  onChanged: () => Promise<void> | void;
}

export function QuickTemplateManagerModal(props: QuickTemplateManagerModalProps) {
  const mode = signal<"list" | "form" | "delete">("list");
  const search = signal("");
  const editing = signal<QuickTemplate | null>(null);
  const deleting = signal<QuickTemplate | null>(null);

  const shortcut = signal("");
  const title = signal("");
  const bodyText = signal("");
  const category = signal("");
  const isActive = signal(true);
  const formError = signal<string | null>(null);
  const busy = signal(false);

  const filtered = () => {
    const q = search().trim().toLowerCase();
    const list = props.templates();
    if (q === "") return list;
    return list.filter(
      (t) =>
        t.shortcut.toLowerCase().includes(q) ||
        t.title.toLowerCase().includes(q) ||
        t.bodyText.toLowerCase().includes(q) ||
        (t.category ?? "").toLowerCase().includes(q),
    );
  };

  const resetToList = () => {
    mode.set("list");
    editing.set(null);
    deleting.set(null);
    formError.set(null);
  };

  const closeAll = () => {
    search.set("");
    resetToList();
    props.onClose();
  };

  const openCreate = () => {
    editing.set(null);
    shortcut.set("");
    title.set("");
    bodyText.set("");
    category.set("");
    isActive.set(true);
    formError.set(null);
    mode.set("form");
  };

  const openEdit = (tpl: QuickTemplate) => {
    editing.set(tpl);
    shortcut.set(tpl.shortcut);
    title.set(tpl.title);
    bodyText.set(tpl.bodyText);
    category.set(tpl.category ?? "");
    isActive.set(tpl.isActive);
    formError.set(null);
    mode.set("form");
  };

  const submitForm = async () => {
    if (busy()) return;
    const currentEdit = editing();
    const parsed = validateQuickTemplate(
      {
        shortcut: shortcut(),
        title: title(),
        bodyText: bodyText(),
        category: category(),
        isActive: isActive(),
      },
      props.templates(),
      currentEdit?.id ?? null,
    );
    if (!parsed.ok) {
      formError.set(parsed.error);
      return;
    }
    formError.set(null);
    busy.set(true);
    try {
      if (currentEdit !== null) {
        await ConversationActions.updateQuickTemplate({
          id: currentEdit.id,
          shortcut: parsed.value.shortcut,
          title: parsed.value.title,
          bodyText: parsed.value.bodyText,
          category: parsed.value.category,
          isActive: parsed.value.isActive,
        });
        notify("Đã cập nhật mẫu trả lời nhanh.", "success");
      } else {
        await ConversationActions.createQuickTemplate({
          shortcut: parsed.value.shortcut,
          title: parsed.value.title,
          bodyText: parsed.value.bodyText,
          category: parsed.value.category,
        });
        notify("Đã tạo mẫu trả lời nhanh.", "success");
      }
      await props.onChanged();
      resetToList();
    } catch (e) {
      const f = report(e, { retry: () => { void submitForm(); } });
      formError.set(f.message);
    } finally {
      busy.set(false);
    }
  };

  const toggleActive = async (tpl: QuickTemplate) => {
    if (busy()) return;
    busy.set(true);
    try {
      await ConversationActions.updateQuickTemplate({
        id: tpl.id,
        shortcut: tpl.shortcut,
        title: tpl.title,
        bodyText: tpl.bodyText,
        category: tpl.category,
        isActive: !tpl.isActive,
      });
      await props.onChanged();
    } catch (e) {
      report(e, { retry: () => { void toggleActive(tpl); } });
    } finally {
      busy.set(false);
    }
  };

  const confirmDelete = async () => {
    const target = deleting();
    if (!target || busy()) return;
    busy.set(true);
    try {
      await ConversationActions.deleteQuickTemplate({ id: target.id });
      notify("Đã xoá mẫu trả lời nhanh.", "success");
      await props.onChanged();
      resetToList();
    } catch (e) {
      report(e, { retry: () => { void confirmDelete(); } });
    } finally {
      busy.set(false);
    }
  };

  return (
    <Modal
      open={props.open()}
      title={
        mode() === "form"
          ? editing() !== null ? "Sửa mẫu trả lời nhanh" : "Thêm mẫu trả lời nhanh"
          : mode() === "delete"
            ? "Xoá mẫu trả lời nhanh"
            : "Quản lý mẫu trả lời nhanh"
      }
      onClose={closeAll}
      class="m-auto w-[620px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
    >
      <View class="gap-3">
        {/* View 1: List */}
        <If when={mode() === "list"}>
          <View class="gap-3">
            <View class="flex-row items-center justify-between gap-2">
              <Text testID="qt-count-badge" class="text-xs text-muted-foreground">
                {`${props.templates().length} / ${MAX_QUICK_TEMPLATES} mẫu`}
              </Text>
              <Button
                testID="qt-add-btn"
                size="sm"
                variant="primary"
                label="+ Thêm mẫu"
                disabled={props.templates().length >= MAX_QUICK_TEMPLATES}
                onPress={openCreate}
              />
            </View>

            <If when={props.templates().length >= MAX_QUICK_TEMPLATES}>
              <View testID="qt-limit-warning" class="rounded-(--radius) border border-warning bg-warning/15 px-3 py-2">
                <Text class="text-xs text-warning">
                  {`Đã đạt giới hạn ${MAX_QUICK_TEMPLATES} mẫu trả lời nhanh. Xoá bớt trước khi thêm mới.`}
                </Text>
              </View>
            </If>

            <Input
              testID="qt-manager-search"
              value={search()}
              onValue={(v) => search.set(v)}
              placeholder="Tìm theo phím tắt, tiêu đề, nội dung…"
            />

            <If when={filtered().length === 0}>
              <Text testID="qt-manager-empty" class="py-6 text-center text-xs text-muted-foreground">
                {search().trim() !== "" ? "Không tìm thấy mẫu khớp." : "Chưa có mẫu trả lời nhanh nào."}
              </Text>
            </If>

            <If when={filtered().length > 0}>
              <Scroll class="max-h-[320px] rounded-(--radius) border border-border">
                <View class="p-1">
                  <Each of={filtered().slice(0, 100)} key={(t) => String(t.id)}>
                    {(item) => (
                      <View
                        testID={`qt-row-${item().id}`}
                        class="flex-row items-start justify-between gap-3 border-b border-border px-2.5 py-2"
                      >
                        <View class="min-w-0 flex-1 gap-0.5">
                          <View class="flex-row flex-wrap items-center gap-1.5">
                            <Text class="text-xs font-semibold text-primary">{`/${item().shortcut}`}</Text>
                            <Text class="truncate text-xs font-medium text-foreground">{item().title}</Text>
                            <If when={(item().category ?? "") !== ""}>
                              <Badge label={item().category ?? ""} tone="neutral" />
                            </If>
                          </View>
                          <Text class="truncate text-xs text-muted-foreground">{item().bodyText}</Text>
                          <Text testID={`qt-usage-${item().id}`} class="text-xs text-muted-foreground">
                            {`Đã dùng ${item().usageCount} lần`}
                          </Text>
                        </View>
                        <View class="flex-row items-center gap-1">
                          <Button
                            testID={`qt-toggle-${item().id}`}
                            size="sm"
                            variant={item().isActive ? "secondary" : "ghost"}
                            label={item().isActive ? "Đang bật" : "Đang tắt"}
                            disabled={busy()}
                            onPress={() => { void toggleActive(item()); }}
                          />
                          <Button
                            testID={`qt-edit-${item().id}`}
                            size="sm"
                            variant="ghost"
                            label="Sửa"
                            disabled={busy()}
                            onPress={() => openEdit(item())}
                          />
                          <Button
                            testID={`qt-delete-${item().id}`}
                            size="sm"
                            variant="ghost"
                            label="Xoá"
                            disabled={busy()}
                            onPress={() => {
                              deleting.set(item());
                              mode.set("delete");
                            }}
                          />
                        </View>
                      </View>
                    )}
                  </Each>
                </View>
              </Scroll>
            </If>

            <View class="flex-row justify-end">
              <Button testID="qt-manager-close" variant="ghost" label="Đóng" onPress={closeAll} />
            </View>
          </View>
        </If>

        {/* View 2: Create / Edit Form */}
        <If when={mode() === "form"}>
          <View class="gap-3">
            <View class="flex-row gap-2">
              <View class="flex-1 gap-1">
                <Text class="text-xs font-medium text-foreground">Phím tắt *</Text>
                <Input
                  testID="qt-form-shortcut"
                  value={shortcut()}
                  onValue={(v) => shortcut.set(v)}
                  placeholder="baogia"
                  autoFocus
                />
              </View>
              <View class="flex-1 gap-1">
                <Text class="text-xs font-medium text-foreground">Danh mục</Text>
                <Input
                  testID="qt-form-category"
                  value={category()}
                  onValue={(v) => category.set(v)}
                  placeholder="Bán hàng"
                />
              </View>
            </View>

            <View class="gap-1">
              <Text class="text-xs font-medium text-foreground">Tiêu đề *</Text>
              <Input
                testID="qt-form-title"
                value={title()}
                onValue={(v) => title.set(v)}
                placeholder="Báo giá sản phẩm"
              />
            </View>

            <View class="gap-1">
              <Text class="text-xs font-medium text-foreground">Nội dung *</Text>
              <Input
                testID="qt-form-body"
                value={bodyText()}
                onValue={(v) => bodyText.set(v)}
                placeholder="Nhập nội dung sẽ chèn khi chọn mẫu…"
                multiline
                class="h-24 rounded-(--radius) border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
              />
            </View>

            <If when={formError() !== null}>
              <Text testID="qt-form-error" class="text-xs text-destructive">{formError() ?? ""}</Text>
            </If>

            <View class="flex-row justify-end gap-2">
              <Button testID="qt-form-cancel" variant="ghost" label="Quay lại" onPress={resetToList} />
              <Button
                testID="qt-form-save"
                variant="primary"
                label={busy() ? "Đang lưu…" : "Lưu mẫu"}
                disabled={busy()}
                onPress={() => { void submitForm(); }}
              />
            </View>
          </View>
        </If>

        {/* View 3: Confirm Delete */}
        <If when={mode() === "delete"}>
          <View class="gap-3">
            <Text class="text-sm text-foreground">
              {`Xoá mẫu "/${deleting()?.shortcut ?? ""}" (${deleting()?.title ?? ""})? Hành động này không thể hoàn tác.`}
            </Text>
            <View class="flex-row justify-end gap-2">
              <Button testID="qt-delete-cancel" variant="ghost" label="Huỷ" onPress={resetToList} />
              <Button
                testID="qt-delete-confirm"
                variant="destructive"
                label={busy() ? "Đang xoá…" : "Xoá mẫu"}
                disabled={busy()}
                onPress={() => { void confirmDelete(); }}
              />
            </View>
          </View>
        </If>
      </View>
    </Modal>
  );
}
