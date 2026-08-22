<script lang="ts">
  // Facebook 24-Hour Policy Gate banner (revised plan Task 4).
  // Inside the window: informational countdown. Outside: policy mode that
  // requires selecting one of the approved CSKH Message Tags before sending.
  import {
    FACEBOOK_MESSAGE_TAGS,
    evaluateFacebookWindow,
    formatRemainingHm
  } from './facebookPolicyGate';

  let {
    lastInboundAt = null,
    nowMs = Date.now(),
    selectedTag = $bindable(null),
    onTagChange = () => {}
  }: {
    /** RFC-3339 timestamp of the customer's last inbound message. */
    lastInboundAt?: string | null;
    /** Injectable clock for deterministic rendering/tests. */
    nowMs?: number;
    selectedTag?: string | null;
    onTagChange?: (tag: string | null) => void;
  } = $props();

  let parsedMs = $derived(lastInboundAt === null || lastInboundAt === undefined ? null : Date.parse(lastInboundAt));
  let status = $derived(evaluateFacebookWindow(parsedMs, nowMs));

  function handleSelect(event: Event): void {
    const value = (event.currentTarget as HTMLSelectElement).value;
    selectedTag = value === '' ? null : value;
    onTagChange(selectedTag);
  }
</script>

{#if status.outsideWindow}
  <div
    class="flex items-center justify-between gap-2 rounded-lg border border-amber-500/20 bg-amber-500/10 p-2.5 text-xs text-amber-600 dark:text-amber-400"
    data-testid="facebook-policy-outside"
  >
    <span>⚠️ Đã quá 24h kể từ tương tác cuối — bắt buộc chọn thẻ tin nhắn CSKH hợp lệ.</span>
    <select
      class="rounded border border-border bg-background px-2 py-1 text-foreground"
      aria-label="Chọn thẻ tin nhắn CSKH"
      value={selectedTag ?? ''}
      onchange={handleSelect}
    >
      <option value="">— Chọn thẻ tin nhắn CSKH —</option>
      {#each FACEBOOK_MESSAGE_TAGS as tag (tag.value)}
        <option value={tag.value}>{tag.value} ({tag.labelVi})</option>
      {/each}
    </select>
  </div>
{:else}
  <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground" data-testid="facebook-policy-inside">
    <span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>
    <span>Còn {formatRemainingHm(status.remainingMs)} trong cửa sổ 24h của Meta</span>
  </div>
{/if}
