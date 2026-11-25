import { computed, List, cleanup, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { ConversationActions, ConversationNetwork, Conversations, Host } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import { CHANNELS, TABS, inboxQuery, isFiltered, type InboxChannel, type InboxTab } from "../stores/inboxFilter";
import { NewChatDialog } from "./NewChatDialog";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { bodyOf, hhmm, mergeMessages, type TimelineMessage } from "../stores/format";
import {
  evaluateFacebookWindow,
  findSlashToken,
  findThreadMatches,
  itemConversationIds,
  mergePinnedMessages,
  parseAttachmentPaths,
  replaceSlashToken,
  resolveFacebookLastInboundAt,
  stepMatchIndex,
  validateFacebookOutbound,
  type ComposerAttachmentItem,
  type InboxItem,
  type PinnedMessage,
  type QuickTemplate,
  type SlashToken,
  type StagedAttachment,
} from "../stores/conversationAdv";
import { LeadLinkActions, type LeadLinkChange } from "../components/conversation/LeadLinkDialogs";
import {
  AttachmentPreviewQueue,
  AttachmentStageBar,
  Facebook24hPolicyBanner,
  InThreadSearchBar,
  PinnedMessageBanner,
} from "../components/conversation/ThreadPanels";
import { QuickTemplateManagerModal, QuickTemplatesPicker } from "../components/conversation/QuickTemplatesPanel";
import { notify, report } from "../stores/toast";

type Message = TimelineMessage;
interface TimelinePage { items: Message[]; nextCursor?: unknown | null; nextBefore?: unknown | null }

const SEND_SAYS = {
  unavailable: "Chưa gửi được: kênh chưa đăng nhập hoặc chưa bật. Nội dung vẫn được giữ, thử lại sau khi kết nối kênh.",
  not_found: "Chưa gửi được: tài khoản kênh này không còn nữa. Nội dung vẫn được giữ.",
  invalid_request: "Chưa gửi được: kênh này chưa được hỗ trợ hoặc nội dung chưa hợp lệ. Nội dung vẫn được giữ.",
};

function Bubble(props: {
  row: () => Message;
  isPinned: () => boolean;
  isHighlighted: () => boolean;
  onTogglePin: (m: Message) => void;
}) {
  const r = props.row;
  const out = () => r().direction === "outbound";
  return (
    <View class={`px-4 py-1 ${out() ? "items-end" : "items-start"}`}>
      <View
        testID={`msg-bubble-${r().id}`}
        class={`max-w-[70%] rounded-(--radius) px-3 py-2 ${out() ? "bg-bubble-out" : "bg-bubble-in"} ${props.isHighlighted() ? "border-2 border-primary" : ""}`}
      >
        <Text class={`text-sm ${out() ? "text-bubble-out-foreground" : "text-bubble-in-foreground"}`}>{bodyOf(r())}</Text>
        <View class="mt-1 flex-row items-center justify-end gap-2">
          <If when={props.isPinned()}>
            <Text testID={`msg-pinned-tag-${r().id}`} class="text-xs font-medium text-primary">Đã ghim</Text>
          </If>
          <Base
            testID={`msg-pin-${r().id}`}
            accessibilityLabel={props.isPinned() ? "Bỏ ghim tin nhắn" : "Ghim tin nhắn"}
            onPress={() => props.onTogglePin(r())}
            class="rounded-(--radius) px-1 hover:bg-accent"
          >
            <Text class="text-xs text-bubble-out-meta">{props.isPinned() ? "Bỏ ghim" : "Ghim"}</Text>
          </Base>
          <Text class="text-xs text-bubble-out-meta">{hhmm(r().occurredAt)}</Text>
        </View>
      </View>
    </View>
  );
}

/**
 * Conversations: the inbox (cursor paged in the core) on the left, the chosen timeline on the right.
 * Includes UI-47 (lead linking/creation/relink/unlink) and UI-48 (pinned banner, in-thread search,
 * quick reply templates picker & CRUD, Facebook 24h policy banner, staged attachment preview queue).
 */
export function ConversationScreen() {
  const current = signal<InboxItem | null>(null);
  const messages = signal<Message[]>([]);
  const timelineReady = signal(false);
  const draft = signal("");
  const sending = signal(false);
  const tab = signal<InboxTab>("all");
  const channel = signal<InboxChannel>("all");
  const search = signal("");
  const newChat = signal(false);

  // UI-48: Pinned messages
  const pins = signal<PinnedMessage[]>([]);
  const activePinIndex = signal(0);
  const highlightedMessageId = signal<number | null>(null);

  // UI-48: In-thread search
  const threadSearchOpen = signal(false);
  const threadSearchQuery = signal("");
  const threadMatchIndex = signal(0);

  // UI-48: Quick reply templates
  const quickTemplates = signal<QuickTemplate[]>([]);
  const quickPickerOpen = signal(false);
  const quickPickerQuery = signal("");
  const quickPickerSlashDriven = signal(false);
  const activeSlashToken = signal<SlashToken | null>(null);
  const quickManagerOpen = signal(false);

  // UI-48: Facebook 24h policy
  const fbNow = signal(Date.now());
  const fbSelectedTag = signal<string | null>(null);

  // UI-48: Staged attachments
  const attachments = signal<ComposerAttachmentItem[]>([]);
  const attachBarOpen = signal(false);
  const attachPathInput = signal("");
  const attachStaging = signal(false);
  const attachError = signal<string | null>(null);

  let before: unknown | null = null;
  let loadingOlder = false;
  let opened = 0;
  let pinEpoch = 0;
  let attachmentSeq = 0;

  const inbox = pager<InboxItem>(
    async (cursor) => {
      await startCore();
      return (await Conversations.inboxPage(inboxQuery(tab(), channel(), search()), cursor, 50)) as Page<InboxItem>;
    },
    (e, retry) => { report(e, { retry }); },
  );

  const target = (c: InboxItem) => c.replyTargets[0] ?? null;
  const cursorOf = (page: TimelinePage) => page.nextCursor ?? page.nextBefore ?? null;

  const pinnedIdSet = computed(() => new Set(pins().map((p) => p.messageId)));
  const threadMatches = computed(() => findThreadMatches(messages(), threadSearchQuery()));
  const activeMatchId = computed(() => {
    const matches = threadMatches();
    if (matches.length === 0) return null;
    const idx = stepMatchIndex(threadMatchIndex(), 0, matches.length);
    return matches[idx] ?? null;
  });
  const activeMatchMessage = computed(() => {
    const id = activeMatchId();
    if (id === null) return null;
    return messages().find((m) => m.id === id) ?? null;
  });

  const activeTarget = () => {
    const c = current();
    return c ? target(c) : null;
  };
  const isFacebookThread = () => activeTarget()?.channel === "messenger";
  const facebookLastInbound = () => resolveFacebookLastInboundAt(activeTarget(), messages());

  const fetchTimelinePage = async (c: InboxItem, cursor: unknown | null): Promise<TimelinePage> => {
    if (c.kind === "customer" && typeof c.leadId === "number") {
      const ch = c.channels && c.channels.length > 0
        ? c.channels.map((b) => b.channel)
        : ["zalo", "whatsapp", "messenger", "email"];
      return (await ConversationActions.contactTimeline(c.leadId, ch, cursor, 50)) as TimelinePage;
    }
    if (c.kind === "unknown" && typeof c.identityId === "number") {
      return (await ConversationActions.identityTimeline(c.identityId, cursor, 50)) as TimelinePage;
    }
    const t = target(c);
    if (!t) return { items: [], nextCursor: null };
    return (await Conversations.timeline(t.conversationId, cursor, 50)) as TimelinePage;
  };

  const loadPins = async (c: InboxItem) => {
    pinEpoch += 1;
    const mine = pinEpoch;
    const convIds = itemConversationIds(c);
    if (convIds.length === 0) {
      pins.set([]);
      activePinIndex.set(0);
      return;
    }
    try {
      const lists = await Promise.all(
        convIds.map((id) => ConversationActions.pinnedMessages(id) as Promise<PinnedMessage[]>),
      );
      if (mine !== pinEpoch) return;
      pins.set(mergePinnedMessages(lists));
      activePinIndex.set(0);
    } catch {
      if (mine === pinEpoch) pins.set([]);
    }
  };

  const loadQuickTemplates = async () => {
    try {
      await startCore();
      const rows = (await ConversationActions.quickTemplates({ includeInactive: true })) as QuickTemplate[];
      quickTemplates.set(Array.isArray(rows) ? rows : []);
    } catch {
      // Quick templates are auxiliary; keep existing list and retry on next manager open.
    }
  };

  const clearStagedAttachments = () => {
    const prev = attachments();
    attachments.set([]);
    attachBarOpen.set(false);
    attachPathInput.set("");
    attachError.set(null);
    for (const item of prev) {
      if (item.stagedAttachmentId !== null) {
        void ConversationNetwork.discardStagedAttachment(item.stagedAttachmentId).catch(() => {});
      }
    }
  };

  /** Shows the badge as read at once; the core catches up (and says so with an event). */
  const markRead = (c: InboxItem) => {
    if (c.unreadCount <= 0) return;
    inbox.patch((rows) => rows.map((r) => (r.key === c.key ? { ...r, unreadCount: 0 } : r)));
    current.set({ ...c, unreadCount: 0 });
    void Conversations.markRead(c.key).catch(() => { void inbox.reload(); });
  };

  const loadTimeline = async (c: InboxItem) => {
    opened += 1;
    const mine = opened;
    timelineReady.set(false);
    messages.set([]);
    before = null;
    if (c.kind !== "customer" && c.kind !== "unknown" && !target(c)) {
      timelineReady.set(true);
      return;
    }
    try {
      const page = await fetchTimelinePage(c, null);
      if (mine !== opened) return;
      before = cursorOf(page);
      messages.set(mergeMessages([], page.items));
      timelineReady.set(true);
      markRead(c);
    } catch (e) {
      if (mine === opened) report(e, { refresh: () => { current.set(null); void inbox.reload(); }, retry: () => { void loadTimeline(c); } });
    }
  };

  /** Pulls the newest page again and folds it in, leaving older messages and the scroll where they are. */
  const refreshTimeline = async (c: InboxItem) => {
    const mine = opened;
    try {
      const page = await fetchTimelinePage(c, null);
      if (mine !== opened) return;
      messages.set(mergeMessages(messages(), page.items));
    } catch {
      // The next event or a reopen tries again; a failed background refresh is not worth a toast.
    }
  };

  const older = async () => {
    const c = current();
    if (!c || before === null || loadingOlder) return;
    loadingOlder = true;
    const mine = opened;
    try {
      const page = await fetchTimelinePage(c, before);
      if (mine !== opened) return;
      before = cursorOf(page);
      messages.set(mergeMessages(messages(), page.items));
    } catch (e) {
      if (mine === opened) report(e, { retry: () => { void older(); } });
    } finally {
      loadingOlder = false;
    }
  };

  const choose = (c: InboxItem) => {
    const prevKey = current()?.key ?? null;
    if (prevKey !== c.key) {
      clearStagedAttachments();
      threadSearchOpen.set(false);
      threadSearchQuery.set("");
      threadMatchIndex.set(0);
      highlightedMessageId.set(null);
      fbSelectedTag.set(null);
    }
    current.set(c);
    void loadTimeline(c);
    void loadPins(c);
  };

  // A filter is asked of the core (it filters and pages), never applied to the rows already loaded.
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  const setTab = (t: InboxTab) => { if (tab() === t) return; tab.set(t); void inbox.reload(); };
  const setChannel = (c: InboxChannel) => { if (channel() === c) return; channel.set(c); void inbox.reload(); };
  const setSearch = (text: string) => {
    search.set(text);
    if (searchTimer !== null) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => { searchTimer = null; void inbox.reload(); }, 250);
  };

  /** After "Tin nhắn mới": show the whole inbox again and open the chat that was just made. */
  const openStarted = async (conversationId: number) => {
    newChat.set(false);
    tab.set("all");
    channel.set("all");
    search.set("");
    await inbox.reload();
    const row = inbox.items().find((c) => c.replyTargets.some((t) => t.conversationId === conversationId));
    if (row) choose(row);
  };

  /** After UI-47 link / create / relink / unlink: refresh inbox and re-select the moved thread. */
  const handleLeadLinkChanged = async (change: LeadLinkChange) => {
    await inbox.reload();
    const matchRow = () => inbox.items().find((r) => {
      if (typeof change.leadId === "number") return r.leadId === change.leadId || r.key === `lead:${change.leadId}`;
      if (typeof change.unlinkedIdentityId === "number") {
        return r.identityId === change.unlinkedIdentityId || r.key === `identity:${change.unlinkedIdentityId}`;
      }
      return false;
    });
    let row = matchRow();
    if (!row && isFiltered(tab(), channel(), search())) {
      tab.set("all");
      channel.set("all");
      search.set("");
      await inbox.reload();
      row = matchRow();
    }
    if (row) choose(row);
    else current.set(null);
  };

  // ---- UI-48 actions ----

  const togglePin = async (m: Message) => {
    const c = current();
    if (!c) return;
    const convId = m.conversationId ?? target(c)?.conversationId ?? null;
    if (convId === null) return;
    const already = pinnedIdSet().has(m.id);
    try {
      if (already) {
        await ConversationActions.unpinMessage({ conversationId: convId, messageId: m.id });
        notify("Đã bỏ ghim tin nhắn.", "info");
      } else {
        await ConversationActions.pinMessage({ conversationId: convId, messageId: m.id });
        notify("Đã ghim tin nhắn.", "success");
      }
      await loadPins(c);
    } catch (e) {
      report(e, { retry: () => { void togglePin(m); } });
    }
  };

  const unpinById = async (messageId: number) => {
    const c = current();
    if (!c) return;
    const pin = pins().find((p) => p.messageId === messageId);
    const convId = pin?.conversationId ?? target(c)?.conversationId ?? null;
    if (convId === null) return;
    try {
      await ConversationActions.unpinMessage({ conversationId: convId, messageId });
      notify("Đã bỏ ghim tin nhắn.", "info");
      await loadPins(c);
    } catch (e) {
      report(e, { retry: () => { void unpinById(messageId); } });
    }
  };

  const handleDraftChange = (v: string) => {
    draft.set(v);
    const tok = findSlashToken(v, v.length);
    if (tok !== null) {
      activeSlashToken.set(tok);
      quickPickerQuery.set(tok.query);
      quickPickerSlashDriven.set(true);
      quickPickerOpen.set(true);
    } else if (quickPickerSlashDriven()) {
      activeSlashToken.set(null);
      quickPickerSlashDriven.set(false);
      quickPickerOpen.set(false);
    }
  };

  const selectQuickTemplate = (tpl: QuickTemplate) => {
    const tok = activeSlashToken();
    if (tok !== null) {
      const next = replaceSlashToken(draft(), tok, tpl.bodyText);
      draft.set(next.value);
    } else {
      const cur = draft();
      if (cur === "") draft.set(tpl.bodyText);
      else draft.set(cur.endsWith(" ") ? `${cur}${tpl.bodyText}` : `${cur} ${tpl.bodyText}`);
    }
    activeSlashToken.set(null);
    quickPickerSlashDriven.set(false);
    quickPickerOpen.set(false);
    void ConversationActions.markQuickTemplateUsed({ id: tpl.id })
      .then(() => loadQuickTemplates())
      .catch(() => {});
  };

  const stageAttachments = async () => {
    const c = current();
    const t = c ? target(c) : null;
    if (!c || !t) {
      attachError.set("Hội thoại này chưa có kênh để đính kèm tệp.");
      return;
    }
    const paths = parseAttachmentPaths(attachPathInput());
    if (paths.length === 0) {
      attachError.set("Nhập ít nhất một đường dẫn tệp.");
      return;
    }
    attachError.set(null);
    attachStaging.set(true);
    try {
      const staged = (await ConversationNetwork.stageAttachmentPaths(t.conversationId, paths)) as StagedAttachment[];
      const added: ComposerAttachmentItem[] = staged.map((s) => {
        attachmentSeq += 1;
        return {
          localId: `att-${attachmentSeq}`,
          stagedAttachmentId: s.stagedAttachmentId,
          fileName: s.fileName,
          mediaType: s.mediaType,
          mimeType: s.mimeType,
          sizeBytes: s.fileSizeBytes,
          status: "ready",
        };
      });
      attachments.set([...attachments(), ...added]);
      attachPathInput.set("");
      notify(`Đã đính kèm ${added.length} tệp vào hàng chờ.`, "success");
    } catch (e) {
      const f = report(e, { retry: () => { void stageAttachments(); } });
      attachError.set(f.message);
    } finally {
      attachStaging.set(false);
    }
  };

  const removeAttachment = async (item: ComposerAttachmentItem) => {
    if (item.stagedAttachmentId === null) {
      attachments.set(attachments().filter((a) => a.localId !== item.localId));
      return;
    }
    attachments.set(attachments().map((a) => (a.localId === item.localId ? { ...a, status: "removing" } : a)));
    try {
      await ConversationNetwork.discardStagedAttachment(item.stagedAttachmentId);
      attachments.set(attachments().filter((a) => a.localId !== item.localId));
    } catch (e) {
      const f = report(e, { retry: () => { void removeAttachment(item); } });
      if (f.code === "not_found") {
        attachments.set(attachments().filter((a) => a.localId !== item.localId));
      } else {
        attachments.set(attachments().map((a) => (a.localId === item.localId ? { ...a, status: "ready" } : a)));
      }
    }
  };

  const send = async () => {
    const c = current();
    const t = c ? target(c) : null;
    const text = draft().trim();
    const readyAttachments = attachments().filter((a) => a.status === "ready" && a.stagedAttachmentId !== null);
    if (!c || !t) { notify("Hội thoại này chưa có kênh để trả lời.", "warning"); return; }
    if (text === "" && readyAttachments.length === 0) {
      notify("Nhập nội dung trước khi gửi.", "warning");
      return;
    }
    if (t.channel === "messenger") {
      const policy = validateFacebookOutbound(
        evaluateFacebookWindow(facebookLastInbound(), fbNow()),
        fbSelectedTag(),
      );
      if (!policy.allowed) {
        notify(policy.reason ?? "Không thể gửi tin do chính sách 24 giờ của Facebook.", "warning");
        return;
      }
    }
    sending.set(true);
    try {
      if (readyAttachments.length > 0 || (t.channel === "messenger" && fbSelectedTag() !== null)) {
        const batchId = `batch-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const parts: Record<string, unknown>[] = [];
        let partIndex = 0;
        if (text !== "") {
          partIndex += 1;
          parts.push({ clientPartId: `${batchId}-p${partIndex}`, type: "text", text });
        }
        for (const att of readyAttachments) {
          partIndex += 1;
          parts.push({
            clientPartId: `${batchId}-p${partIndex}`,
            type: "attachment",
            stagedAttachmentId: att.stagedAttachmentId,
          });
        }
        await ConversationActions.sendBatch({
          conversationId: t.conversationId,
          clientBatchId: batchId,
          replyToMessageId: null,
          parts,
          messageTag: t.channel === "messenger" ? fbSelectedTag() : null,
        });
      } else {
        await ConversationActions.sendMessage({
          conversationId: t.conversationId,
          channelAccountId: t.channelAccountId,
          text,
        });
      }
    } catch (e) {
      // The composer keeps the text/attachments and no bubble is added: nothing was sent.
      report(e, { say: SEND_SAYS, retry: () => { void send(); } });
      sending.set(false);
      return;
    }
    draft.set("");
    attachments.set([]);
    attachBarOpen.set(false);
    fbSelectedTag.set(null);
    sending.set(false);
    await refreshTimeline(c);
  };

  let inboxTimer: ReturnType<typeof setTimeout> | null = null;
  const reloadInboxSoon = () => {
    if (inboxTimer !== null) clearTimeout(inboxTimer);
    inboxTimer = setTimeout(() => { inboxTimer = null; void inbox.reload(); }, 150);
  };
  const conversationOf = (payload: unknown): number | null => {
    const id = (payload as { conversationId?: unknown } | null)?.conversationId;
    return typeof id === "number" ? id : null;
  };
  // The core says when a conversation changed: fold a delta in, refetch on an invalidation.
  const stop = Host.onEvent((name, payload) => {
    if (name !== "conversation-invalidated" && name !== "conversation-message-delta") return;
    reloadInboxSoon();
    const c = current();
    const convId = conversationOf(payload);
    if (!c || convId === null || !itemConversationIds(c).includes(convId)) return;
    const delta = (payload as { message?: Message } | null)?.message;
    if (name === "conversation-message-delta" && delta && typeof delta.id === "number") {
      messages.set(mergeMessages(messages(), [delta]));
    } else {
      void refreshTimeline(c);
    }
  });
  const fbClock = setInterval(() => fbNow.set(Date.now()), 30_000);
  cleanup(() => {
    stop();
    clearInterval(fbClock);
    if (inboxTimer !== null) clearTimeout(inboxTimer);
    if (searchTimer !== null) clearTimeout(searchTimer);
    clearStagedAttachments();
  });

  void inbox.reload();
  void loadQuickTemplates();

  return (
    <View class="flex-1 flex-row">
      <View class="w-[320px] border-r border-border bg-card">
        <View class="border-b border-border px-4 py-3">
          <View class="flex-row items-center justify-between">
            <Text class="text-base font-semibold text-foreground">Hộp thư</Text>
            <Button testID="new-chat" size="sm" label="Tin nhắn mới" onPress={() => newChat.set(true)} />
          </View>
          <Text testID="inbox-caption" class="text-xs text-muted-foreground">{inbox.ready() ? `${inbox.total()} hội thoại` : "Đang mở hộp thư…"}</Text>
          <View class="mt-2 gap-2">
            <Input testID="inbox-search" value={search()} onValue={setSearch} placeholder="Tìm theo tên hoặc tin nhắn" />
            <View class="flex-row flex-wrap gap-1">
              <Each of={TABS} key={(t) => t.id}>
                {(t) => (
                  <Base testID={`inbox-tab-${t().id}`} accessibilityLabel={t().label} onPress={() => setTab(t().id)}
                    class={`rounded-full border px-2.5 py-1 ${tab() === t().id ? "border-primary bg-primary" : "border-border bg-card"}`}>
                    <Text class={`text-xs ${tab() === t().id ? "text-primary-foreground" : "text-foreground"}`}>{t().label}</Text>
                  </Base>
                )}
              </Each>
            </View>
            <View class="flex-row flex-wrap gap-1">
              <Each of={CHANNELS} key={(c) => c.id}>
                {(c) => (
                  <Base testID={`inbox-channel-${c().id}`} accessibilityLabel={c().label} onPress={() => setChannel(c().id)}
                    class={`rounded-full border px-2.5 py-1 ${channel() === c().id ? "border-primary bg-accent" : "border-border bg-card"}`}>
                    <Text class="text-xs text-foreground">{c().label}</Text>
                  </Base>
                )}
              </Each>
            </View>
          </View>
        </View>
        <If when={!inbox.ready()}><View class="gap-3 p-4"><Skeleton /><Skeleton /></View></If>
        <If when={inbox.ready() && inbox.total() === 0 && !isFiltered(tab(), channel(), search())}><EmptyState title="Hộp thư trống" /></If>
        <If when={inbox.ready() && inbox.total() === 0 && isFiltered(tab(), channel(), search())}>
          <EmptyState title="Không có hội thoại khớp" hint="Đổi bộ lọc hoặc xoá từ khoá." />
        </If>
        <List testID="inbox-list" each={inbox.items()} keyExtractor={(c) => c.key} estimatedItemSize={64}
          onLoadMore={() => { void inbox.more(); }} class="flex-1">
          {(c) => (
            <Base testID={`inbox-${c().key}`} onPress={() => choose(c())}
              class="gap-0.5 border-b border-border px-4 py-3 hover:bg-accent">
              <View class="flex-row items-center justify-between">
                <Text class="truncate flex-1 text-sm font-medium text-foreground">{c().title}</Text>
                <If when={c().unreadCount > 0}><Badge label={String(c().unreadCount)} tone="success" /></If>
              </View>
              <Text class="truncate text-xs text-muted-foreground">{c().lastMessagePreview ?? ""}</Text>
            </Base>
          )}
        </List>
      </View>

      <View class="flex-1 bg-chat-canvas">
        <If when={!current()}><EmptyState title="Chọn một hội thoại" hint="Tin nhắn hiện ở đây." /></If>
        <If when={current()}>
          {(c) => (
            <View class="flex-1">
              {/* Header: title, caption, in-thread search toggle, UI-47 lead linking actions */}
              <View class="flex-row flex-wrap items-center justify-between gap-2 border-b border-border bg-card px-5 py-3">
                <View class="min-w-0 flex-1">
                  <Text testID="chat-title" class="truncate text-base font-semibold text-foreground">{c().title}</Text>
                  <Text testID="chat-caption" class="text-xs text-muted-foreground">{`${messages().length} tin đã tải`}</Text>
                </View>
                <View class="flex-row flex-wrap items-center gap-2">
                  <Button
                    testID="conv-search-toggle"
                    size="sm"
                    variant={threadSearchOpen() ? "primary" : "ghost"}
                    label="Tìm tin"
                    onPress={() => {
                      const next = !threadSearchOpen();
                      threadSearchOpen.set(next);
                      if (!next) {
                        threadSearchQuery.set("");
                        threadMatchIndex.set(0);
                      }
                    }}
                  />
                  <LeadLinkActions
                    item={c}
                    onChanged={(change) => { void handleLeadLinkChanged(change); }}
                  />
                </View>
              </View>

              {/* UI-48: Pinned Message Banner */}
              <PinnedMessageBanner
                pins={pins}
                activeIndex={activePinIndex}
                onCycle={() => activePinIndex.set(stepMatchIndex(activePinIndex(), 1, pins().length))}
                onJump={(messageId) => highlightedMessageId.set(messageId)}
                onUnpin={(messageId) => { void unpinById(messageId); }}
              />

              {/* UI-48: In-Thread Search Bar */}
              <InThreadSearchBar
                open={threadSearchOpen}
                query={threadSearchQuery}
                matchCount={() => threadMatches().length}
                currentIndex={() => stepMatchIndex(threadMatchIndex(), 0, threadMatches().length)}
                activeMatch={activeMatchMessage}
                onQuery={(q) => {
                  threadSearchQuery.set(q);
                  threadMatchIndex.set(0);
                }}
                onPrev={() => threadMatchIndex.set(stepMatchIndex(threadMatchIndex(), -1, threadMatches().length))}
                onNext={() => threadMatchIndex.set(stepMatchIndex(threadMatchIndex(), 1, threadMatches().length))}
                onClose={() => {
                  threadSearchOpen.set(false);
                  threadSearchQuery.set("");
                  threadMatchIndex.set(0);
                }}
              />

              <If when={!timelineReady()}><View class="gap-3 p-4"><Skeleton /><Skeleton /></View></If>
              {/* Mounted once the page is in, so "start at the end" sees the messages (an empty list has no end). */}
              <If when={timelineReady()}>
                <List testID="chat-list" each={messages()} keyExtractor={(m) => String(m.id)} estimatedItemSize={56}
                  initialScrollAtEnd onLoadMoreAtStart={() => { void older(); }} class="flex-1">
                  {(row) => (
                    <Bubble
                      row={row}
                      isPinned={() => pinnedIdSet().has(row().id)}
                      isHighlighted={() =>
                        highlightedMessageId() === row().id || activeMatchId() === row().id
                      }
                      onTogglePin={(m) => { void togglePin(m); }}
                    />
                  )}
                </List>
              </If>

              {/* UI-48: Facebook 24h Policy Banner */}
              <Facebook24hPolicyBanner
                visible={isFacebookThread}
                lastInboundAt={facebookLastInbound}
                nowMs={fbNow}
                selectedTag={fbSelectedTag}
                onSelectTag={(tag) => fbSelectedTag.set(tag)}
              />

              {/* UI-48: Staged Attachment Preview Queue & Path Staging Bar */}
              <AttachmentPreviewQueue
                items={attachments}
                onRemove={(item) => { void removeAttachment(item); }}
              />
              <AttachmentStageBar
                open={attachBarOpen}
                staging={attachStaging}
                pathText={attachPathInput}
                error={attachError}
                onPathText={(v) => attachPathInput.set(v)}
                onStage={() => { void stageAttachments(); }}
                onClose={() => {
                  attachBarOpen.set(false);
                  attachError.set(null);
                }}
              />

              {/* UI-48: Quick Reply Templates Picker */}
              <QuickTemplatesPicker
                open={quickPickerOpen}
                templates={quickTemplates}
                query={quickPickerQuery}
                onQuery={(q) => quickPickerQuery.set(q)}
                onSelect={selectQuickTemplate}
                onOpenManager={() => {
                  quickPickerOpen.set(false);
                  quickPickerSlashDriven.set(false);
                  activeSlashToken.set(null);
                  quickManagerOpen.set(true);
                  void loadQuickTemplates();
                }}
                onClose={() => {
                  quickPickerOpen.set(false);
                  quickPickerSlashDriven.set(false);
                  activeSlashToken.set(null);
                }}
              />

              {/* Composer */}
              <View class="flex-row items-center gap-2 border-t border-border bg-card p-3">
                <Button
                  testID="composer-templates-btn"
                  size="sm"
                  variant={quickPickerOpen() ? "primary" : "secondary"}
                  label="Mẫu"
                  onPress={() => {
                    const next = !quickPickerOpen();
                    quickPickerSlashDriven.set(false);
                    activeSlashToken.set(null);
                    quickPickerQuery.set("");
                    quickPickerOpen.set(next);
                    if (next) void loadQuickTemplates();
                  }}
                />
                <Button
                  testID="composer-attach-btn"
                  size="sm"
                  variant={attachBarOpen() ? "primary" : "secondary"}
                  label="Đính kèm"
                  onPress={() => {
                    const next = !attachBarOpen();
                    attachBarOpen.set(next);
                    if (!next) attachError.set(null);
                  }}
                />
                <View class="flex-1">
                  <Input
                    testID="composer"
                    value={draft()}
                    placeholder="Nhập tin nhắn… (gõ / để chọn mẫu nhanh)"
                    onValue={handleDraftChange}
                    onSubmit={() => { void send(); }}
                  />
                </View>
                <Button testID="send" label="Gửi" variant="primary" disabled={sending()} onPress={() => { void send(); }} />
              </View>
            </View>
          )}
        </If>
      </View>

      <NewChatDialog open={newChat()} onClose={() => newChat.set(false)} onStarted={(id) => { void openStarted(id); }} />
      <QuickTemplateManagerModal
        open={quickManagerOpen}
        templates={quickTemplates}
        onClose={() => quickManagerOpen.set(false)}
        onChanged={loadQuickTemplates}
      />
    </View>
  );
}
