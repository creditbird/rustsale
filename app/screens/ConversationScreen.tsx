import { List, cleanup, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { ConversationActions, Conversations, Host } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import { CHANNELS, TABS, inboxQuery, isFiltered, type InboxChannel, type InboxTab } from "../stores/inboxFilter";
import { NewChatDialog } from "./NewChatDialog";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { bodyOf, hhmm, mergeMessages, type TimelineMessage } from "../stores/format";
import { notify, report } from "../stores/toast";

interface InboxItem {
  key: string;
  title: string;
  lastMessagePreview: string | null;
  lastMessageAt: string | null;
  unreadCount: number;
  replyTargets: { channel: string; channelAccountId: number; conversationId: number }[];
}
type Message = TimelineMessage;
interface TimelinePage { items: Message[]; nextCursor?: unknown | null; nextBefore?: unknown | null }

const SEND_SAYS = {
  unavailable: "Chưa gửi được: kênh chưa đăng nhập hoặc chưa bật. Nội dung vẫn được giữ, thử lại sau khi kết nối kênh.",
  not_found: "Chưa gửi được: tài khoản kênh này không còn nữa. Nội dung vẫn được giữ.",
  invalid_request: "Chưa gửi được: kênh này chưa được hỗ trợ. Nội dung vẫn được giữ.",
};

function Bubble(props: { row: () => Message }) {
  const r = props.row;
  const out = () => r().direction === "outbound";
  return (
    <View class={`px-4 py-1 ${out() ? "items-end" : "items-start"}`}>
      <View class={`max-w-[70%] rounded-(--radius) px-3 py-2 ${out() ? "bg-bubble-out" : "bg-bubble-in"}`}>
        <Text class={`text-sm ${out() ? "text-bubble-out-foreground" : "text-bubble-in-foreground"}`}>{bodyOf(r())}</Text>
        <Text class="mt-1 self-end text-xs text-bubble-out-meta">{hhmm(r().occurredAt)}</Text>
      </View>
    </View>
  );
}

/**
 * Conversations: the inbox (cursor paged in the core) on the left, the chosen timeline on the right.
 * Messages are kept oldest first: the newest is at the bottom and an older page loads above it as
 * you scroll up. The core answers by itself when something changes (`conversation-invalidated`,
 * `conversation-message-delta`), so nothing here has to be reopened.
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
  let before: unknown | null = null;
  let loadingOlder = false;
  let opened = 0;

  const inbox = pager<InboxItem>(
    async (cursor) => {
      await startCore();
      return (await Conversations.inboxPage(inboxQuery(tab(), channel(), search()), cursor, 50)) as Page<InboxItem>;
    },
    (e, retry) => { report(e, { retry }); },
  );

  const target = (c: InboxItem) => c.replyTargets[0] ?? null;
  const cursorOf = (page: TimelinePage) => page.nextCursor ?? page.nextBefore ?? null;

  /** Shows the badge as read at once; the core catches up (and says so with an event). */
  const markRead = (c: InboxItem) => {
    if (c.unreadCount <= 0) return;
    inbox.patch((rows) => rows.map((r) => (r.key === c.key ? { ...r, unreadCount: 0 } : r)));
    current.set({ ...c, unreadCount: 0 });
    void Conversations.markRead(c.key).catch(() => { void inbox.reload(); });
  };

  const loadTimeline = async (c: InboxItem) => {
    const t = target(c);
    opened += 1;
    const mine = opened;
    timelineReady.set(false);
    messages.set([]);
    before = null;
    if (!t) { timelineReady.set(true); return; }
    try {
      const page = (await Conversations.timeline(t.conversationId, null, 50)) as TimelinePage;
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
    const t = target(c);
    if (!t) return;
    const mine = opened;
    try {
      const page = (await Conversations.timeline(t.conversationId, null, 50)) as TimelinePage;
      if (mine !== opened) return;
      messages.set(mergeMessages(messages(), page.items));
    } catch {
      // The next event or a reopen tries again; a failed background refresh is not worth a toast.
    }
  };
  const older = async () => {
    const c = current();
    const t = c ? target(c) : null;
    if (!c || !t || before === null || loadingOlder) return;
    loadingOlder = true;
    const mine = opened;
    try {
      const page = (await Conversations.timeline(t.conversationId, before, 50)) as TimelinePage;
      if (mine !== opened) return;
      before = cursorOf(page);
      messages.set(mergeMessages(messages(), page.items));
    } catch (e) {
      if (mine === opened) report(e, { retry: () => { void older(); } });
    } finally {
      loadingOlder = false;
    }
  };
  const choose = (c: InboxItem) => { current.set(c); void loadTimeline(c); };

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

  const send = async () => {
    const c = current();
    const t = c ? target(c) : null;
    const text = draft().trim();
    if (!c || !t) { notify("Hội thoại này chưa có kênh để trả lời.", "warning"); return; }
    if (text === "") { notify("Nhập nội dung trước khi gửi.", "warning"); return; }
    sending.set(true);
    try {
      await ConversationActions.sendMessage({ conversationId: t.conversationId, channelAccountId: t.channelAccountId, text });
    } catch (e) {
      // The composer keeps the text and no bubble is added: nothing was sent.
      report(e, { say: SEND_SAYS, retry: () => { void send(); } });
      sending.set(false);
      return;
    }
    draft.set("");
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
    const t = c ? target(c) : null;
    if (!c || !t || conversationOf(payload) !== t.conversationId) return;
    const delta = (payload as { message?: Message } | null)?.message;
    if (name === "conversation-message-delta" && delta && typeof delta.id === "number") {
      messages.set(mergeMessages(messages(), [delta]));
    } else {
      void refreshTimeline(c);
    }
  });
  cleanup(() => {
    stop();
    if (inboxTimer !== null) clearTimeout(inboxTimer);
    if (searchTimer !== null) clearTimeout(searchTimer);
  });

  void inbox.reload();

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
              <View class="border-b border-border bg-card px-6 py-3">
                <Text testID="chat-title" class="text-base font-semibold text-foreground">{c().title}</Text>
                <Text testID="chat-caption" class="text-xs text-muted-foreground">{`${messages().length} tin đã tải`}</Text>
              </View>
              <If when={!timelineReady()}><View class="gap-3 p-4"><Skeleton /><Skeleton /></View></If>
              {/* Mounted once the page is in, so "start at the end" sees the messages (an empty list has no end). */}
              <If when={timelineReady()}>
                <List testID="chat-list" each={messages()} keyExtractor={(m) => String(m.id)} estimatedItemSize={56}
                  initialScrollAtEnd onLoadMoreAtStart={() => { void older(); }} class="flex-1">
                  {(row) => <Bubble row={row} />}
                </List>
              </If>
              <View class="flex-row items-center gap-2 border-t border-border bg-card p-3">
                <View class="flex-1">
                  <Input testID="composer" value={draft()} placeholder="Nhập tin nhắn…" onValue={(v) => draft.set(v)} onSubmit={() => { void send(); }} />
                </View>
                <Button testID="send" label="Gửi" variant="primary" disabled={sending()} onPress={() => { void send(); }} />
              </View>
            </View>
          )}
        </If>
      </View>
      <NewChatDialog open={newChat()} onClose={() => newChat.set(false)} onStarted={(id) => { void openStarted(id); }} />
    </View>
  );
}
