<script lang="ts">
  import {
    forwardConversationMessage,
    getConversationForwardOperation,
    retryConversationForwardTarget,
    type ConversationMessage,
    type ForwardConversationMessageResult,
    type ConversationForwardTargetResult,
    type InboxItem
  } from '$lib/services/conversationService';
  import {
    createForwardSnapshot,
    filterForwardTargets,
    mergeForwardResult,
    type ForwardDialogSnapshot
  } from './forwardState';
  import ForwardTargetResultList from './ForwardTargetResultList.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Search, X, Forward, AlertCircle } from 'lucide-svelte';

  let {
    open = $bindable(false),
    sourceMessage = null,
    inbox = [],
    maxTargets = 10,
    onForwarded = () => {}
  }: {
    open?: boolean;
    sourceMessage: ConversationMessage | null;
    inbox: InboxItem[];
    maxTargets?: number;
    onForwarded?: (result: ForwardConversationMessageResult) => void;
  } = $props();

  let searchQuery = $state('');
  let submitting = $state(false);
  let retryingTargetId = $state<number | null>(null);
  let snapshot = $state<ForwardDialogSnapshot>(createForwardSnapshot(0));
  let pollGeneration = 0;

  const FORWARD_POLL_INTERVAL_MS = 250;
  const FORWARD_POLL_ATTEMPTS = 40;

  // Reset snapshot whenever sourceMessage changes or dialog opens
  $effect(() => {
    if (open && sourceMessage) {
      pollGeneration += 1;
      snapshot = createForwardSnapshot(sourceMessage.id);
      searchQuery = '';
      submitting = false;
      retryingTargetId = null;
    }
  });

  let filteredTargets = $derived.by(() => {
    if (!sourceMessage) return [];
    return filterForwardTargets(
      inbox,
      sourceMessage.channelAccountId,
      sourceMessage.conversationId,
      searchQuery
    );
  });

  let targetTitlesMap = $derived.by(() => {
    const map: Record<number, string> = {};
    for (const item of inbox) {
      for (const rt of item.replyTargets) {
        map[rt.conversationId] = item.title;
      }
    }
    return map;
  });

  function toggleTarget(conversationId: number) {
    if (snapshot.status === 'submitted' || submitting) return;

    const exists = snapshot.selectedTargetIds.includes(conversationId);
    if (exists) {
      snapshot = {
        ...snapshot,
        selectedTargetIds: snapshot.selectedTargetIds.filter((id) => id !== conversationId)
      };
    } else {
      if (snapshot.selectedTargetIds.length >= maxTargets) return;
      snapshot = {
        ...snapshot,
        selectedTargetIds: [...snapshot.selectedTargetIds, conversationId]
      };
    }
  }

  async function handleSubmit() {
    if (!sourceMessage || snapshot.selectedTargetIds.length === 0 || submitting) return;
    submitting = true;
    snapshot = { ...snapshot, status: 'submitting', error: null };

    try {
      const res = await forwardConversationMessage({
        sourceMessageId: sourceMessage.id,
        targetConversationIds: snapshot.selectedTargetIds,
        clientOperationId: snapshot.clientOperationId
      });
      snapshot = mergeForwardResult(snapshot, res);
      onForwarded(res);
      if (res.status === 'queued' || res.status === 'sending') {
        const generation = ++pollGeneration;
        void pollForwardOperation(res.clientOperationId, generation);
      }
    } catch (err) {
      snapshot = {
        ...snapshot,
        status: 'failed',
        error: err instanceof Error ? err.message : String(err)
      };
    } finally {
      submitting = false;
    }
  }

  async function pollForwardOperation(clientOperationId: string, generation: number) {
    for (let attempt = 0; attempt < FORWARD_POLL_ATTEMPTS; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, FORWARD_POLL_INTERVAL_MS));
      if (!open || generation !== pollGeneration) return;

      try {
        const result = await getConversationForwardOperation(clientOperationId);
        if (!open || generation !== pollGeneration) return;
        snapshot = mergeForwardResult(snapshot, result);
        onForwarded(result);
        if (result.status !== 'queued' && result.status !== 'sending') return;
      } catch (err) {
        if (attempt === FORWARD_POLL_ATTEMPTS - 1 && generation === pollGeneration) {
          snapshot = {
            ...snapshot,
            error: err instanceof Error ? err.message : String(err)
          };
        }
      }
    }
  }

  async function handleRetryTarget(targetConversationId: number) {
    if (retryingTargetId !== null) return;
    retryingTargetId = targetConversationId;

    try {
      const targetRes = await retryConversationForwardTarget({
        clientOperationId: snapshot.result?.clientOperationId || snapshot.clientOperationId,
        targetConversationId
      });
      snapshot = mergeForwardResult(snapshot, targetRes);
      if (snapshot.result) {
        onForwarded(snapshot.result);
      }
    } catch (err) {
      snapshot = {
        ...snapshot,
        error: err instanceof Error ? err.message : String(err)
      };
    } finally {
      retryingTargetId = null;
    }
  }

  function handleClose() {
    pollGeneration += 1;
    open = false;
  }

  function getMessagePreviewText(msg: ConversationMessage): string {
    if (msg.textContent?.trim()) return msg.textContent.trim();
    if (msg.attachments.length > 0 && msg.attachments[0].fileName) {
      return `[File] ${msg.attachments[0].fileName}`;
    }
    return `[${msg.contentType}]`;
  }
</script>

{#if open && sourceMessage}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-xs"
    role="dialog"
    aria-modal="true"
    data-testid="forward-conversation-dialog"
  >
    <div
      class="flex max-h-[90vh] w-full max-w-md flex-col rounded-lg border bg-background p-4 shadow-lg animate-in fade-in-50 zoom-in-95 space-y-4"
    >
      <!-- Header -->
      <div class="flex items-center justify-between border-b pb-3">
        <div class="flex items-center gap-2">
          <Forward class="size-5 text-primary" />
          <h3 class="font-semibold text-base">Chuyển tiếp tin nhắn</h3>
        </div>
        <button
          type="button"
          aria-label="Đóng"
          class="rounded-sm p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
          onclick={handleClose}
        >
          <X class="size-4" />
        </button>
      </div>

      <!-- Source Message Preview -->
      <div class="rounded-md border bg-muted/40 p-2.5 text-xs text-muted-foreground space-y-1">
        <span class="font-medium text-foreground">Nội dung tin nhắn:</span>
        <p class="line-clamp-2 italic text-foreground/90">{getMessagePreviewText(sourceMessage)}</p>
      </div>

      {#if snapshot.error}
        <div
          class="flex items-center gap-2 rounded-md bg-destructive/10 p-2.5 text-xs text-destructive"
          role="alert"
        >
          <AlertCircle class="size-4 shrink-0" />
          <span>{snapshot.error}</span>
        </div>
      {/if}

      {#if snapshot.status === 'submitted' && snapshot.result}
        <!-- Results View -->
        <ForwardTargetResultList
          targets={snapshot.result.targets}
          targetTitles={targetTitlesMap}
          {retryingTargetId}
          onRetryTarget={handleRetryTarget}
        />
      {:else}
        <!-- Selection View -->
        <div class="space-y-3 flex-1 min-h-0 flex flex-col">
          <!-- Search Bar -->
          <div class="relative">
            <Search class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
            <input
              type="text"
              placeholder="Tìm kiếm cuộc trò chuyện..."
              bind:value={searchQuery}
              class="w-full rounded-md border border-input bg-background pl-8 pr-3 py-1.5 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              data-testid="forward-dialog-search-input"
            />
          </div>

          <!-- Target Selection Count -->
          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span>Chọn người nhận (tối đa {maxTargets}):</span>
            <span class="font-medium text-foreground" data-slot="selected-count">
              Đã chọn {snapshot.selectedTargetIds.length}/{maxTargets}
            </span>
          </div>

          <!-- Target List -->
          <div
            class="max-h-60 overflow-y-auto rounded-md border divide-y divide-border/50 bg-card"
            data-testid="forward-target-selection-list"
          >
            {#if filteredTargets.length === 0}
              <div class="p-4 text-center text-xs text-muted-foreground">
                Không tìm thấy cuộc trò chuyện phù hợp cùng tài khoản.
              </div>
            {:else}
              {#each filteredTargets as item (item.key)}
                {@const convId = item.replyTargets.find(rt => rt.channelAccountId === sourceMessage.channelAccountId)?.conversationId ?? item.conversationId}
                {#if convId !== null}
                  {@const isSelected = snapshot.selectedTargetIds.includes(convId)}
                  {@const isDisabled = !isSelected && snapshot.selectedTargetIds.length >= maxTargets}
                  <label
                    class="flex items-center justify-between p-2.5 text-xs hover:bg-muted/50 cursor-pointer {isDisabled ? 'opacity-50 cursor-not-allowed' : ''}"
                    data-testid={`forward-target-item-${convId}`}
                  >
                    <div class="flex items-center gap-2.5 min-w-0">
                      {#if item.avatarUrl}
                        <img
                          src={item.avatarUrl}
                          alt={item.title}
                          class="size-7 rounded-full object-cover shrink-0"
                        />
                      {:else}
                        <div
                          class="size-7 rounded-full bg-primary/10 text-primary font-semibold flex items-center justify-center shrink-0 text-[10px]"
                        >
                          {item.title ? item.title.slice(0, 2).toUpperCase() : '?'}
                        </div>
                      {/if}
                      <span class="font-medium truncate text-foreground">{item.title}</span>
                    </div>

                    <input
                      type="checkbox"
                      checked={isSelected}
                      disabled={isDisabled}
                      onchange={() => toggleTarget(convId)}
                      class="size-4 rounded border-gray-300 text-primary focus:ring-primary"
                    />
                  </label>
                {/if}
              {/each}
            {/if}
          </div>
        </div>
      {/if}

      <!-- Footer Buttons -->
      <div class="flex items-center justify-end gap-2 border-t pt-3">
        <Button type="button" variant="outline" size="sm" onclick={handleClose}>
          {snapshot.status === 'submitted' ? 'Đóng' : 'Hủy'}
        </Button>
        {#if snapshot.status !== 'submitted'}
          <Button
            type="button"
            variant="default"
            size="sm"
            disabled={snapshot.selectedTargetIds.length === 0 || submitting}
            onclick={handleSubmit}
            data-testid="submit-forward-button"
          >
            {submitting ? 'Đang chuyển...' : 'Chuyển tiếp'}
          </Button>
        {/if}
      </div>
    </div>
  </div>
{/if}
