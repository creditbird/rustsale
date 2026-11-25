import { signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Conversations, ConversationNetwork } from "rustsale/native";
import { Button, Input, Modal } from "../ui";
import { accountLabel, parseDirectChat, type DirectChatAccount } from "../stores/inboxFilter";
import { notify, report } from "../stores/toast";

export interface NewChatDialogProps {
  open: boolean;
  onClose: () => void;
  /** The chat was opened and its first message queued; the id is the conversation to show. */
  onStarted: (conversationId: number) => void;
}

/**
 * "Tin nhắn mới": opens a direct chat with a phone number or an id on a Zalo or WhatsApp account and queues
 * the first message in the core's outbox. The channel supervisor delivers it once the account is connected.
 */
export function NewChatDialog(props: NewChatDialogProps) {
  const accounts = signal<DirectChatAccount[]>([]);
  const loaded = signal(false);
  const chosen = signal<number | null>(null);
  const target = signal("");
  const name = signal("");
  const message = signal("");
  const error = signal<string | null>(null);
  const busy = signal(false);

  const account = (): DirectChatAccount | null => accounts().find((a) => a.id === chosen()) ?? null;

  const load = async () => {
    try {
      const rows = (await Conversations.directChatAccounts()) as DirectChatAccount[];
      accounts.set(rows);
      if (rows.length === 1) chosen.set(rows[0].id);
      loaded.set(true);
    } catch (e) {
      report(e, { retry: () => { void load(); } });
    }
  };
  // The list is small and local, so read it once when the screen mounts.
  void load();

  const submit = async () => {
    if (busy()) return;
    const parsed = parseDirectChat(account(), target(), name(), message());
    if (!parsed.ok) { error.set(parsed.error); return; }
    error.set(null);
    busy.set(true);
    try {
      const first = (await ConversationNetwork.startDirect(parsed.value)) as { conversationId: number };
      target.set("");
      name.set("");
      message.set("");
      notify("Đã mở cuộc trò chuyện. Tin đầu tiên sẽ gửi khi tài khoản kết nối.", "success");
      props.onStarted(first.conversationId);
    } catch (e) {
      // Nothing was opened: the form keeps what was typed so it can be corrected.
      report(e, { retry: () => { void submit(); } });
    } finally {
      busy.set(false);
    }
  };

  return (
    <Modal open={props.open} title="Tin nhắn mới" onClose={props.onClose}>
      <View class="gap-3">
        <If when={loaded() && accounts().length === 0}>
          <Text testID="newchat-no-account" class="text-sm text-muted-foreground">
            Chưa có tài khoản Zalo hoặc WhatsApp nào. Thêm tài khoản ở Cài đặt rồi quay lại.
          </Text>
        </If>
        <If when={accounts().length > 0}>
          <View class="gap-3">
            <View class="gap-1">
              <Text class="text-xs text-muted-foreground">Gửi từ tài khoản</Text>
              <Each of={accounts()} key={(a) => String(a.id)}>
                {(a) => (
                  <Base
                    testID={`newchat-account-${a().id}`}
                    accessibilityLabel={accountLabel(a())}
                    onPress={() => chosen.set(a().id)}
                    class={`rounded-(--radius) border px-3 py-2 ${chosen() === a().id ? "border-primary bg-accent" : "border-border"}`}
                  >
                    <Text class="text-sm text-foreground">{`${accountLabel(a())}${a().connectionStatus === "connected" ? "" : " (chưa kết nối)"}`}</Text>
                  </Base>
                )}
              </Each>
              <If when={account() !== null && account()?.connectionStatus !== "connected"}>
                <Text testID="newchat-offline" class="text-xs text-muted-foreground">
                  Tài khoản này chưa kết nối: tin nằm trong hàng gửi và đi khi kết nối lại.
                </Text>
              </If>
            </View>
            <Input testID="newchat-target" value={target()} onValue={(v) => target.set(v)} placeholder="Số điện thoại hoặc mã người nhận" />
            <Input testID="newchat-name" value={name()} onValue={(v) => name.set(v)} placeholder="Tên hiển thị (không bắt buộc)" />
            <Input testID="newchat-message" value={message()} onValue={(v) => message.set(v)} placeholder="Tin nhắn đầu tiên" onSubmit={() => { void submit(); }} />
          </View>
        </If>
        <If when={error() !== null}><Text testID="newchat-error" class="text-sm text-destructive">{error() ?? ""}</Text></If>
        <View class="flex-row justify-end gap-2">
          <Button testID="newchat-cancel" variant="ghost" label="Huỷ" onPress={props.onClose} />
          <Button testID="newchat-send" variant="primary" label={busy() ? "Đang mở…" : "Gửi"} disabled={busy() || accounts().length === 0} onPress={() => { void submit(); }} />
        </View>
      </View>
    </Modal>
  );
}
