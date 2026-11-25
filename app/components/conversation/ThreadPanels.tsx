import { Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Badge, Button, Input } from "../../ui";
import { bodyOf, hhmm, type TimelineMessage } from "../../stores/format";
import {
  attachmentTypeLabel,
  evaluateFacebookWindow,
  formatBytes,
  formatRemainingHm,
  type ComposerAttachmentItem,
  type PinnedMessage,
} from "../../stores/conversationAdv";

// ---- 1. Pinned Message Banner ----

export interface PinnedMessageBannerProps {
  pins: () => PinnedMessage[];
  activeIndex: () => number;
  onCycle: () => void;
  onJump: (messageId: number) => void;
  onUnpin: (messageId: number) => void;
}

export function PinnedMessageBanner(props: PinnedMessageBannerProps) {
  const activePin = (): PinnedMessage | null => {
    const list = props.pins();
    if (list.length === 0) return null;
    const idx = props.activeIndex() % list.length;
    return list[idx] ?? list[0] ?? null;
  };

  return (
    <If when={activePin() !== null}>
      <View
        testID="pinned-message-banner"
        class="flex-row items-center justify-between gap-3 border-b border-border bg-accent px-4 py-2"
      >
        <View class="min-w-0 flex-1 gap-0.5">
          <Text class="text-xs font-semibold text-primary">
            {props.pins().length > 1
              ? `Tin đã ghim (${(props.activeIndex() % props.pins().length) + 1}/${props.pins().length})`
              : "Tin đã ghim"}
          </Text>
          <Text testID="pinned-message-text" class="truncate text-xs text-foreground">
            {`${activePin()?.senderName ? `${activePin()?.senderName}: ` : ""}${activePin()?.messageText || "[Tệp đính kèm]"}`}
          </Text>
        </View>
        <View class="flex-row items-center gap-1">
          <If when={props.pins().length > 1}>
            <Button testID="pin-cycle-btn" size="sm" variant="ghost" label="Tiếp" onPress={props.onCycle} />
          </If>
          <Button
            testID="pin-jump-btn"
            size="sm"
            variant="ghost"
            label="Xem"
            onPress={() => {
              const p = activePin();
              if (p) props.onJump(p.messageId);
            }}
          />
          <Button
            testID="pin-unpin-btn"
            size="sm"
            variant="ghost"
            label="Bỏ ghim"
            onPress={() => {
              const p = activePin();
              if (p) props.onUnpin(p.messageId);
            }}
          />
        </View>
      </View>
    </If>
  );
}

// ---- 2. In-Thread Search Bar ----

export interface InThreadSearchBarProps {
  open: () => boolean;
  query: () => string;
  matchCount: () => number;
  currentIndex: () => number;
  activeMatch: () => TimelineMessage | null;
  onQuery: (q: string) => void;
  onPrev: () => void;
  onNext: () => void;
  onClose: () => void;
}

export function InThreadSearchBar(props: InThreadSearchBarProps) {
  return (
    <If when={props.open()}>
      <View testID="in-thread-search-bar" class="gap-1.5 border-b border-border bg-card px-4 py-2">
        <View class="flex-row items-center gap-2">
          <View class="flex-1">
            <Input
              testID="thread-search-input"
              value={props.query()}
              onValue={props.onQuery}
              onSubmit={props.onNext}
              placeholder="Tìm tin nhắn trong hội thoại…"
              autoFocus
            />
          </View>
          <If when={props.query().trim() !== ""}>
            <Text testID="thread-search-count" class="text-xs font-medium text-muted-foreground">
              {props.matchCount() > 0 ? `${props.currentIndex() + 1}/${props.matchCount()}` : "0 kết quả"}
            </Text>
          </If>
          <Button
            testID="thread-search-prev"
            size="sm"
            variant="secondary"
            label="Trước"
            disabled={props.matchCount() === 0}
            onPress={props.onPrev}
          />
          <Button
            testID="thread-search-next"
            size="sm"
            variant="secondary"
            label="Sau"
            disabled={props.matchCount() === 0}
            onPress={props.onNext}
          />
          <Button testID="thread-search-close" size="sm" variant="ghost" label="Đóng" onPress={props.onClose} />
        </View>
        <If when={props.activeMatch() !== null}>
          <View testID="thread-search-preview" class="flex-row items-center gap-2 rounded-(--radius) bg-muted px-3 py-1.5">
            <Text class="text-xs font-semibold text-primary">
              {hhmm(props.activeMatch()?.occurredAt ?? "")}
            </Text>
            <Text class="flex-1 truncate text-xs text-foreground">
              {props.activeMatch() ? bodyOf(props.activeMatch()!) : ""}
            </Text>
          </View>
        </If>
      </View>
    </If>
  );
}

// ---- 3. Facebook 24h Policy Banner ----

export interface Facebook24hPolicyBannerProps {
  visible: () => boolean;
  lastInboundAt: () => string | null;
  nowMs: () => number;
  selectedTag: () => string | null;
  onSelectTag: (tag: string | null) => void;
}

export function Facebook24hPolicyBanner(props: Facebook24hPolicyBannerProps) {
  const status = () => evaluateFacebookWindow(props.lastInboundAt(), props.nowMs());

  return (
    <If when={props.visible()}>
      <View testID="facebook-policy-banner" class="border-t border-border bg-card px-3 pt-2">
        <If when={!status().freeWindow && !status().humanAgentEligible}>
          <View
            testID="facebook-policy-expired"
            class="rounded-(--radius) border border-destructive bg-destructive/15 px-3 py-2"
          >
            <Text class="text-xs font-medium text-destructive">
              Đã quá cửa sổ 7 ngày của Facebook Messenger. Không thể gửi tin mới cho đến khi khách nhắn lại.
            </Text>
          </View>
        </If>

        <If when={!status().freeWindow && status().humanAgentEligible}>
          <View
            testID="facebook-policy-outside"
            class="flex-row flex-wrap items-center justify-between gap-2 rounded-(--radius) border border-warning bg-warning/15 px-3 py-2"
          >
            <Text class="flex-1 text-xs font-medium text-warning">
              Đã quá 24 giờ kể từ tin cuối của khách. Chọn thẻ HUMAN_AGENT (cửa sổ CSKH 7 ngày) để tiếp tục gửi.
            </Text>
            <Base
              testID="fb-tag-human-agent"
              accessibilityLabel="Chọn thẻ HUMAN_AGENT"
              onPress={() => props.onSelectTag(props.selectedTag() === "HUMAN_AGENT" ? null : "HUMAN_AGENT")}
              class={`rounded-(--radius) border px-2.5 py-1 ${props.selectedTag() === "HUMAN_AGENT" ? "border-primary bg-primary" : "border-border bg-card"}`}
            >
              <Text class={`text-xs font-medium ${props.selectedTag() === "HUMAN_AGENT" ? "text-primary-foreground" : "text-foreground"}`}>
                {props.selectedTag() === "HUMAN_AGENT" ? "HUMAN_AGENT ✓" : "Chọn HUMAN_AGENT"}
              </Text>
            </Base>
          </View>
        </If>

        <If when={status().freeWindow}>
          <View testID="facebook-policy-inside" class="flex-row items-center gap-2 py-1">
            <Badge label="Facebook 24h" tone="success" />
            <Text class="text-xs text-muted-foreground">
              {`Cửa sổ miễn phí 24h · còn ${formatRemainingHm(status().remainingMs)}`}
            </Text>
          </View>
        </If>
      </View>
    </If>
  );
}

// ---- 4. Attachment Preview Queue + Staging Bar ----

export interface AttachmentPreviewQueueProps {
  items: () => ComposerAttachmentItem[];
  onRemove: (item: ComposerAttachmentItem) => void;
}

export function AttachmentPreviewQueue(props: AttachmentPreviewQueueProps) {
  return (
    <If when={props.items().length > 0}>
      <View
        testID="attachment-preview-queue"
        class="flex-row flex-wrap gap-2 border-t border-border bg-muted px-3 py-2"
      >
        <Each of={props.items()} key={(it) => it.localId}>
          {(item) => (
            <View
              testID={`attachment-item-${item().localId}`}
              class={`flex-row items-center gap-2 rounded-(--radius) border px-2.5 py-1.5 ${item().status === "error" ? "border-destructive bg-destructive/15" : "border-border bg-card"}`}
            >
              <Badge label={attachmentTypeLabel(item().mediaType)} tone="neutral" />
              <View class="max-w-[180px]">
                <Text class="truncate text-xs font-medium text-foreground">{item().fileName}</Text>
                <Text class="text-xs text-muted-foreground">{formatBytes(item().sizeBytes)}</Text>
              </View>
              <If when={item().status === "staging"}>
                <Text testID={`attachment-status-${item().localId}`} class="text-xs text-primary">Đang tải…</Text>
              </If>
              <If when={item().status === "removing"}>
                <Text testID={`attachment-status-${item().localId}`} class="text-xs text-muted-foreground">Đang xoá…</Text>
              </If>
              <If when={item().status === "error"}>
                <Text testID={`attachment-status-${item().localId}`} class="text-xs text-destructive">Lỗi</Text>
              </If>
              <Base
                testID={`remove-attachment-${item().localId}`}
                accessibilityLabel={`Gỡ ${item().fileName}`}
                disabled={item().status === "removing"}
                onPress={() => props.onRemove(item())}
                class="rounded-(--radius) px-1.5 py-0.5 hover:bg-destructive/15"
              >
                <Text class="text-xs font-medium text-muted-foreground">✕</Text>
              </Base>
            </View>
          )}
        </Each>
      </View>
    </If>
  );
}

export interface AttachmentStageBarProps {
  open: () => boolean;
  staging: () => boolean;
  pathText: () => string;
  error: () => string | null;
  onPathText: (value: string) => void;
  onStage: () => void;
  onClose: () => void;
}

export function AttachmentStageBar(props: AttachmentStageBarProps) {
  return (
    <If when={props.open()}>
      <View testID="attachment-stage-bar" class="gap-1.5 border-t border-border bg-card px-3 py-2">
        <Text class="text-xs text-muted-foreground">
          Nhập đường dẫn tệp cục bộ để đưa vào hàng chờ đính kèm (hỗ trợ nhiều tệp cách nhau bằng dấu phẩy).
        </Text>
        <View class="flex-row items-center gap-2">
          <View class="flex-1">
            <Input
              testID="attach-path-input"
              value={props.pathText()}
              onValue={props.onPathText}
              onSubmit={props.onStage}
              placeholder="/đường/dẫn/tới/tệp.png"
              autoFocus
            />
          </View>
          <Button
            testID="attach-stage-btn"
            size="sm"
            variant="primary"
            label={props.staging() ? "Đang nạp…" : "Thêm tệp"}
            disabled={props.staging()}
            onPress={props.onStage}
          />
          <Button testID="attach-panel-close" size="sm" variant="ghost" label="Đóng" onPress={props.onClose} />
        </View>
        <If when={props.error() !== null}>
          <Text testID="attach-stage-error" class="text-xs text-destructive">{props.error() ?? ""}</Text>
        </If>
      </View>
    </If>
  );
}
