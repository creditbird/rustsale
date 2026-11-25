import { effect, Scroll, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Categories, ConversationActions, Leads } from "rustsale/native";
import { Badge, Button, Input, Modal } from "../../ui";
import type { Page } from "../../stores/core";
import {
  itemConversationIds,
  resolveIdentityId,
  resolveInitialPhone,
  validateNewLeadForm,
  type InboxItem,
} from "../../stores/conversationAdv";
import { notify, report } from "../../stores/toast";

interface LeadOption {
  id: number;
  name: string;
  company: string | null;
  status: string;
  phone: string | null;
  email: string | null;
}

interface LeadCategory {
  id: number;
  name: string;
}

export interface LeadLinkChange {
  leadId?: number;
  unlinkedIdentityId?: number | null;
  conversationIds?: number[];
}

export interface LeadLinkActionsProps {
  item: () => InboxItem;
  onChanged: (change: LeadLinkChange) => void;
}

/**
 * UI-47: Lead linking & creation in the Conversation workspace.
 * - Unknown contact: Create Lead & link, or Link to an existing lead.
 * - Linked customer: compact lead summary (status, name/company/phone, categories), Change Lead (relink), Unlink.
 */
export function LeadLinkActions(props: LeadLinkActionsProps) {
  const leadDetail = signal<LeadOption | null>(null);
  const leadCategories = signal<LeadCategory[]>([]);

  const dialog = signal<"none" | "create" | "link" | "relink" | "unlink">("none");

  const name = signal("");
  const company = signal("");
  const phone = signal("");
  const email = signal("");
  const createError = signal<string | null>(null);
  const creating = signal(false);

  const leadSearch = signal("");
  const leadOptions = signal<LeadOption[]>([]);
  const loadingLeads = signal(false);
  const selectedLeadId = signal<number | null>(null);
  const linkError = signal<string | null>(null);
  const linking = signal(false);

  const unlinking = signal(false);

  let detailSeq = 0;
  effect(() => {
    const leadId = props.item().leadId ?? null;
    detailSeq += 1;
    const seq = detailSeq;
    if (leadId === null) {
      leadDetail.set(null);
      leadCategories.set([]);
      return;
    }
    void (async () => {
      try {
        const [detail, cats] = await Promise.all([
          Leads.details(leadId) as Promise<LeadOption>,
          Categories.forLead(leadId) as Promise<LeadCategory[]>,
        ]);
        if (seq !== detailSeq) return;
        leadDetail.set(detail);
        leadCategories.set(Array.isArray(cats) ? cats : []);
      } catch {
        if (seq === detailSeq) {
          leadDetail.set(null);
          leadCategories.set([]);
        }
      }
    })();
  });

  const closeDialog = () => {
    dialog.set("none");
    createError.set(null);
    linkError.set(null);
  };

  let searchSeq = 0;
  const fetchLeadOptions = async (query: string) => {
    searchSeq += 1;
    const seq = searchSeq;
    loadingLeads.set(true);
    linkError.set(null);
    try {
      const trimmed = query.trim();
      const page = (await Leads.page(
        trimmed === "" ? { sort: "id", descending: true } : { search: trimmed, sort: "id", descending: true },
        null,
        30,
      )) as Page<LeadOption>;
      if (seq !== searchSeq) return;
      const currentLeadId = props.item().leadId ?? null;
      const filtered = dialog() === "relink" && currentLeadId !== null
        ? page.items.filter((l) => l.id !== currentLeadId)
        : page.items;
      leadOptions.set(filtered);
    } catch (e) {
      if (seq === searchSeq) {
        const f = report(e, { retry: () => { void fetchLeadOptions(query); } });
        linkError.set(f.message);
      }
    } finally {
      if (seq === searchSeq) loadingLeads.set(false);
    }
  };

  const openCreate = () => {
    const it = props.item();
    name.set(it.title || it.participants?.[0]?.displayName || "");
    company.set("");
    phone.set(resolveInitialPhone(it));
    email.set("");
    createError.set(null);
    dialog.set("create");
  };

  const openPicker = (mode: "link" | "relink") => {
    leadSearch.set("");
    selectedLeadId.set(null);
    linkError.set(null);
    dialog.set(mode);
    void fetchLeadOptions("");
  };

  const submitCreate = async () => {
    if (creating()) return;
    const err = validateNewLeadForm(name());
    if (err !== null) {
      createError.set(err);
      return;
    }
    const it = props.item();
    const identityId = resolveIdentityId(it);
    createError.set(null);
    creating.set(true);
    try {
      const channelName = it.channels?.[0]?.channel ?? it.replyTargets[0]?.channel ?? "conversation";
      const leadId = (await Leads.add({
        name: name().trim(),
        company: company().trim() || "Cá nhân",
        status: "New",
        value: 0,
        owner: "Self",
        email: email().trim(),
        phone: phone().trim(),
        about: `Tạo từ hội thoại (${channelName})`,
      })) as number;
      if (identityId !== null) {
        await ConversationActions.linkIdentity({ identityId, leadId });
      }
      dialog.set("none");
      notify("Đã tạo và liên kết khách hàng.", "success");
      props.onChanged({ leadId, conversationIds: itemConversationIds(it) });
    } catch (e) {
      const f = report(e, { retry: () => { void submitCreate(); } });
      createError.set(f.message);
    } finally {
      creating.set(false);
    }
  };

  const submitLink = async () => {
    if (linking()) return;
    const targetLeadId = selectedLeadId();
    if (targetLeadId === null) {
      linkError.set("Chọn một khách hàng trước khi xác nhận.");
      return;
    }
    const it = props.item();
    const sourceLeadId = it.leadId ?? null;
    const identityId = resolveIdentityId(it);
    const isRelink = dialog() === "relink";
    if (isRelink && sourceLeadId !== null && sourceLeadId === targetLeadId) {
      linkError.set("Khách hàng mới phải khác khách hàng hiện tại.");
      return;
    }
    linkError.set(null);
    linking.set(true);
    try {
      if (isRelink && sourceLeadId !== null) {
        await ConversationActions.relinkLead({ sourceLeadId, targetLeadId });
      } else if (identityId !== null) {
        await ConversationActions.linkIdentity({ identityId, leadId: targetLeadId });
      } else {
        linkError.set("Không xác định được danh tính liên hệ để liên kết.");
        return;
      }
      dialog.set("none");
      notify(isRelink ? "Đã đổi liên kết khách hàng." : "Đã liên kết khách hàng.", "success");
      props.onChanged({ leadId: targetLeadId, conversationIds: itemConversationIds(it) });
    } catch (e) {
      const f = report(e, { retry: () => { void submitLink(); } });
      linkError.set(f.message);
    } finally {
      linking.set(false);
    }
  };

  const submitUnlink = async () => {
    if (unlinking()) return;
    const it = props.item();
    const leadId = it.leadId ?? null;
    const identityId = resolveIdentityId(it);
    if (leadId === null && identityId === null) return;
    unlinking.set(true);
    try {
      if (leadId !== null) {
        await ConversationActions.unlinkLead({ leadId });
      } else if (identityId !== null) {
        await ConversationActions.unlinkIdentity({ identityId });
      }
      dialog.set("none");
      notify("Đã bỏ liên kết khách hàng.", "success");
      props.onChanged({ unlinkedIdentityId: identityId, conversationIds: itemConversationIds(it) });
    } catch (e) {
      report(e, { retry: () => { void submitUnlink(); } });
    } finally {
      unlinking.set(false);
    }
  };

  const isGroup = () => props.item().kind === "group";
  const isLinked = () => (props.item().leadId ?? null) !== null;
  const hasIdentity = () => resolveIdentityId(props.item()) !== null;

  return (
    <View class="flex-row flex-wrap items-center gap-2">
      <If when={!isGroup() && isLinked()}>
        <View testID="linked-lead-summary" class="flex-row flex-wrap items-center gap-1.5">
          <If when={leadDetail() !== null}>
            <Badge label={leadDetail()?.status ?? "Lead"} tone="success" />
          </If>
          <If when={leadDetail() !== null && (leadDetail()?.name ?? "").trim() !== "" && leadDetail()?.name !== props.item().title}>
            <Text testID="linked-lead-name" class="text-xs font-medium text-foreground">
              {leadDetail()?.name ?? ""}
            </Text>
          </If>
          <If when={leadDetail() !== null && ((leadDetail()?.company ?? "") !== "" || (leadDetail()?.phone ?? "") !== "")}>
            <Text class="text-xs text-muted-foreground">
              {[leadDetail()?.company, leadDetail()?.phone].filter((v) => v && v.trim() !== "").join(" · ")}
            </Text>
          </If>
          <Each of={leadCategories()} key={(c) => String(c.id)}>
            {(cat) => <Badge label={cat().name} tone="neutral" />}
          </Each>
        </View>
        <Button testID="conv-relink-btn" size="sm" variant="secondary" label="Đổi lead" onPress={() => openPicker("relink")} />
        <Button testID="conv-unlink-btn" size="sm" variant="ghost" label="Bỏ liên kết" onPress={() => dialog.set("unlink")} />
      </If>

      <If when={!isGroup() && !isLinked()}>
        <Button
          testID="conv-link-btn"
          size="sm"
          variant="secondary"
          label="Liên kết lead"
          disabled={!hasIdentity()}
          onPress={() => openPicker("link")}
        />
        <Button
          testID="conv-create-lead-btn"
          size="sm"
          variant="primary"
          label="Tạo lead"
          disabled={!hasIdentity()}
          onPress={openCreate}
        />
      </If>

      {/* Modal 1: Create Lead & Link */}
      <Modal open={dialog() === "create"} title="Tạo khách hàng từ liên hệ" onClose={closeDialog}>
        <View class="gap-3">
          <View class="gap-1">
            <Text class="text-xs font-medium text-foreground">Họ tên *</Text>
            <Input
              testID="create-lead-name"
              value={name()}
              onValue={(v) => name.set(v)}
              placeholder="Nhập tên khách hàng…"
              autoFocus
            />
          </View>
          <View class="gap-1">
            <Text class="text-xs font-medium text-foreground">Công ty</Text>
            <Input
              testID="create-lead-company"
              value={company()}
              onValue={(v) => company.set(v)}
              placeholder="Cá nhân"
            />
          </View>
          <View class="gap-1">
            <Text class="text-xs font-medium text-foreground">Số điện thoại</Text>
            <Input
              testID="create-lead-phone"
              value={phone()}
              onValue={(v) => phone.set(v)}
              placeholder="0901234567"
            />
          </View>
          <View class="gap-1">
            <Text class="text-xs font-medium text-foreground">Email</Text>
            <Input
              testID="create-lead-email"
              value={email()}
              onValue={(v) => email.set(v)}
              placeholder="email@example.com"
              onSubmit={() => { void submitCreate(); }}
            />
          </View>
          <If when={createError() !== null}>
            <Text testID="create-lead-error" class="text-xs text-destructive">{createError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="create-lead-cancel" variant="ghost" label="Huỷ" onPress={closeDialog} />
            <Button
              testID="create-lead-submit"
              variant="primary"
              label={creating() ? "Đang tạo…" : "Tạo & liên kết"}
              disabled={creating()}
              onPress={() => { void submitCreate(); }}
            />
          </View>
        </View>
      </Modal>

      {/* Modal 2: Link / Change Lead */}
      <Modal
        open={dialog() === "link" || dialog() === "relink"}
        title={dialog() === "relink" ? "Đổi khách hàng liên kết" : "Liên kết với khách hàng có sẵn"}
        onClose={closeDialog}
        class="m-auto w-[480px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
      >
        <View class="gap-3">
          <Input
            testID="link-lead-search"
            value={leadSearch()}
            onValue={(v) => {
              leadSearch.set(v);
              void fetchLeadOptions(v);
            }}
            placeholder="Tìm theo tên, công ty, số điện thoại…"
            autoFocus
          />
          <If when={loadingLeads()}>
            <Text class="text-xs text-muted-foreground">Đang tải danh sách khách hàng…</Text>
          </If>
          <If when={!loadingLeads() && leadOptions().length === 0}>
            <Text testID="link-lead-empty" class="text-xs text-muted-foreground">Không tìm thấy khách hàng phù hợp.</Text>
          </If>
          <Scroll class="max-h-[240px] rounded-(--radius) border border-border p-1">
            <View class="gap-1">
              <Each of={leadOptions()} key={(l) => String(l.id)}>
                {(lead) => (
                  <Base
                    testID={`link-lead-option-${lead().id}`}
                    accessibilityLabel={lead().name}
                    onPress={() => selectedLeadId.set(lead().id)}
                    class={`flex-row items-center justify-between rounded-(--radius) px-3 py-2 ${selectedLeadId() === lead().id ? "bg-accent" : "hover:bg-muted"}`}
                  >
                    <View class="min-w-0 flex-1">
                      <Text class="truncate text-sm font-medium text-foreground">{lead().name}</Text>
                      <Text class="truncate text-xs text-muted-foreground">
                        {[lead().company || "Cá nhân", lead().phone].filter((v) => v && v.trim() !== "").join(" · ")}
                      </Text>
                    </View>
                    <If when={selectedLeadId() === lead().id}>
                      <Text class="text-xs font-semibold text-primary">Đã chọn</Text>
                    </If>
                  </Base>
                )}
              </Each>
            </View>
          </Scroll>
          <If when={linkError() !== null}>
            <Text testID="link-lead-error" class="text-xs text-destructive">{linkError() ?? ""}</Text>
          </If>
          <View class="flex-row justify-end gap-2">
            <Button testID="link-lead-cancel" variant="ghost" label="Huỷ" onPress={closeDialog} />
            <Button
              testID="link-lead-confirm"
              variant="primary"
              label={linking() ? "Đang lưu…" : dialog() === "relink" ? "Xác nhận đổi" : "Xác nhận liên kết"}
              disabled={linking() || selectedLeadId() === null}
              onPress={() => { void submitLink(); }}
            />
          </View>
        </View>
      </Modal>

      {/* Modal 3: Confirm Unlink */}
      <Modal open={dialog() === "unlink"} title="Bỏ liên kết khách hàng" onClose={closeDialog}>
        <View class="gap-3">
          <Text class="text-sm text-foreground">
            {`Bỏ liên kết hội thoại với "${props.item().title}"? Tin nhắn vẫn được giữ nguyên và hội thoại chuyển sang nhóm Chưa liên kết.`}
          </Text>
          <View class="flex-row justify-end gap-2">
            <Button testID="unlink-lead-cancel" variant="ghost" label="Huỷ" onPress={closeDialog} />
            <Button
              testID="unlink-lead-confirm"
              variant="destructive"
              label={unlinking() ? "Đang xử lý…" : "Bỏ liên kết"}
              disabled={unlinking()}
              onPress={() => { void submitUnlink(); }}
            />
          </View>
        </View>
      </Modal>
    </View>
  );
}
