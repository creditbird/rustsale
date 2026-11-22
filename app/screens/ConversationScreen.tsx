import { List, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { ConversationActions, Conversations, Host } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { notify, report } from "../stores/toast";

interface InboxItem {
  key: string;
  title: string;
  lastMessagePreview: string | null;
  lastMessageAt: string | null;
  unreadCount: number;
  replyTargets: { channel: string; channelAccountId: number; conversationId: number }[];
}
interface Message {
  id: number;
  direction: string;
  textContent: string | null;
  occurredAt: string;
  status: string;
}
interface TimelinePage { items: Message[]; nextBefore: unknown | null }

const hhmm = (iso: string) => (iso.length >= 16 ? iso.slice(11, 16) : "");

function Bubble(props: { row: () => Message }) {
  const r = props.row;
  const out = () => r().direction === "outbound";
  return (
    <View class={`px-4 py-1 ${out() ? "items-end" : "items-start"}`}>
      <View class={`max-w-[70%] rounded-(--radius) px-3 py-2 ${out() ? "bg-bubble-out" : "bg-bubble-in"}`}>
        <Text class={`text-sm ${out() ? "text-bubble-out-foreground" : "text-bubble-in-foreground"}`}>{r().textContent ?? ""}</Text>
        <Text class="mt-1 self-end text-xs text-bubble-out-meta">{hhmm(r().occurredAt)}</Text>
      </View>
    </View>
  );
}

/**
 * Conversations: the inbox (cursor paged in the core) on the left, the chosen timeline on the right.
 * The timeline is newest first from the core; it is stored oldest first and older pages load as you scroll up.
 */
export function ConversationScreen() {
  const current = signal<InboxItem | null>(null);
  const messages = signal<Message[]>([]);
  const timelineReady = signal(false);
  const draft = signal("");
  const sending = signal(false);
  let before: unknown | null = null;
  let loadingOlder = false;
  let opened = 0;

  const inbox = pager<InboxItem>(
    async (cursor) => {
      await startCore();
      return (await Conversations.inboxPage({ sort: "recent" }, cursor, 50)) as Page<InboxItem>;
    },
    (e, retry) => { report(e, { retry }); },
  );

  const target = (c: InboxItem) => c.replyTargets[0] ?? null;

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
      before = page.nextBefore ?? null;
      messages.set(page.items.slice().reverse());
      timelineReady.set(true);
      if (c.unreadCount > 0) void Conversations.markRead(c.key).then(() => inbox.reload()).catch(() => undefined);
    } catch (e) {
      if (mine === opened) report(e, { refresh: () => { current.set(null); void inbox.reload(); }, retry: () => { void loadTimeline(c); } });
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
      before = page.nextBefore ?? null;
      messages.set(page.items.slice().reverse().concat(messages()));
    } catch (e) {
      if (mine === opened) report(e, { retry: () => { void older(); } });
    } finally {
      loadingOlder = false;
    }
  };
  const choose = (c: InboxItem) => { current.set(c); void loadTimeline(c); };

  const send = async () => {
    const c = current();
    const t = c ? target(c) : null;
    const text = draft().trim();
    if (!c || !t) { notify("Hội thoại này chưa có kênh để trả lời.", "warning"); return; }
    if (text === "") { notify("Nhập nội dung trước khi gửi.", "warning"); return; }
    sending.set(true);
    try {
      await ConversationActions.sendMessage({ conversationId: t.conversationId, channelAccountId: t.channelAccountId, text });
      draft.set("");
      await loadTimeline(c);
    } catch (e) {
      report(e, { retry: () => { void send(); } });
    } finally {
      sending.set(false);
    }
  };

  // The core says when a conversation changed; reload what is on screen.
  const stop = Host.onEvent((name) => {
    if (name !== "conversation-updated") return;
    void inbox.reload();
    const c = current();
    if (c) void loadTimeline(c);
  });
  void stop;

  void inbox.reload();

  return (
    <View class="flex-1 flex-row">
      <View class="w-[320px] border-r border-border bg-card">
        <View class="border-b border-border px-4 py-3">
          <Text class="text-base font-semibold text-foreground">Hộp thư</Text>
          <Text testID="inbox-caption" class="text-xs text-muted-foreground">{`${inbox.total()} hội thoại`}</Text>
        </View>
        <If when={!inbox.ready()}><View class="gap-3 p-4"><Skeleton /><Skeleton /></View></If>
        <If when={inbox.ready() && inbox.total() === 0}><EmptyState title="Hộp thư trống" /></If>
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
              <List testID="chat-list" each={messages()} keyExtractor={(m) => String(m.id)} estimatedItemSize={56}
                initialScrollAtEnd onLoadMoreAtStart={() => { void older(); }} class="flex-1">
                {(row) => <Bubble row={row} />}
              </List>
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
    </View>
  );
}
