<script module lang="ts">
  import type {
    ConversationReactionCode,
    ConversationReactionGroup,
    ConversationReactionMutationResult
  } from '$lib/services/conversationService';

  export type ReactionMutationKey = `${number}:${ConversationReactionCode}`;
</script>

<script lang="ts">
  import {
    setConversationReaction,
    type SetConversationReactionRequest
  } from '$lib/services/conversationService';
  import { AlertCircle, X } from 'lucide-svelte';

  let {
    onReactionsUpdated = () => {}
  }: {
    onReactionsUpdated?: (messageId: number, reactions: ConversationReactionGroup[]) => void;
  } = $props();

  let pendingReactions = $state(new Map<ReactionMutationKey, ConversationReactionMutationResult>());
  let mutationErrors = $state<
    Array<{
      id: string;
      messageId: number;
      reaction: ConversationReactionCode;
      status: 'failed' | 'delivery_unknown';
      errorMessage: string;
    }>
  >([]);

  type QueuedReactionBatch = {
    incrementBy: number;
    timer: ReturnType<typeof setTimeout> | null;
    promise: Promise<ConversationReactionMutationResult | null>;
    resolve: (result: ConversationReactionMutationResult | null) => void;
  };

  const queuedIncrements = new Map<ReactionMutationKey, QueuedReactionBatch>();

  function mutationErrorMessage(error: unknown): string {
    if (error instanceof Error && error.message.trim()) return error.message;
    if (typeof error === 'object' && error !== null && 'message' in error) {
      const message = Reflect.get(error, 'message');
      if (typeof message === 'string' && message.trim()) return message;
    }
    if (typeof error === 'string' && error.trim()) return error;
    return 'Unable to update reaction';
  }

  export function isPending(messageId: number, reactionCode: ConversationReactionCode): boolean {
    return pendingReactions.has(`${messageId}:${reactionCode}`);
  }

  export function getPendingReactionCodes(messageId: number): ConversationReactionCode[] {
    const codes: ConversationReactionCode[] = [];
    for (const key of pendingReactions.keys()) {
      const [msgIdStr, codeStr] = key.split(':');
      if (Number(msgIdStr) === messageId) {
        codes.push(codeStr as ConversationReactionCode);
      }
    }
    return codes;
  }

  async function sendReactionRequest(
    messageId: number,
    reactionCode: ConversationReactionCode,
    active: boolean,
    count?: number
  ): Promise<ConversationReactionMutationResult | null> {
    const key: ReactionMutationKey = `${messageId}:${reactionCode}`;
    const clientMutationId = `${messageId}-${reactionCode}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    const pendingResult: ConversationReactionMutationResult = {
      clientMutationId,
      messageId,
      reaction: reactionCode,
      requestedActive: active,
      status: 'running',
      reactions: [],
      errorCode: null,
      errorMessage: null
    };

    const nextPending = new Map(pendingReactions);
    nextPending.set(key, pendingResult);
    pendingReactions = nextPending;

    try {
      const request: SetConversationReactionRequest = {
        messageId,
        reaction: reactionCode,
        active,
        count,
        clientMutationId
      };

      const result = await setConversationReaction(request);

      const clearedPending = new Map(pendingReactions);
      clearedPending.delete(key);
      pendingReactions = clearedPending;

      if (result.status === 'succeeded') {
        onReactionsUpdated(messageId, result.reactions);
        return result;
      } else {
        onReactionsUpdated(messageId, result.reactions);
        const errorItem = {
          id: clientMutationId,
          messageId,
          reaction: reactionCode,
          status: result.status === 'delivery_unknown' ? ('delivery_unknown' as const) : ('failed' as const),
          errorMessage: result.errorMessage || `Reaction error (${result.status})`
        };
        mutationErrors = [...mutationErrors, errorItem];
        return result;
      }
    } catch (err) {
      const clearedPending = new Map(pendingReactions);
      clearedPending.delete(key);
      pendingReactions = clearedPending;

      const errorMsg = mutationErrorMessage(err);
      const errorItem = {
        id: clientMutationId,
        messageId,
        reaction: reactionCode,
        status: 'failed' as const,
        errorMessage: errorMsg
      };
      mutationErrors = [...mutationErrors, errorItem];
      return null;
    }
  }

  function cancelQueuedIncrement(key: ReactionMutationKey) {
    const batch = queuedIncrements.get(key);
    if (!batch) return;
    if (batch.timer !== null) clearTimeout(batch.timer);
    queuedIncrements.delete(key);
    batch.resolve(null);
  }

  function scheduleIncrementBatch(
    key: ReactionMutationKey,
    messageId: number,
    reactionCode: ConversationReactionCode,
    batch: QueuedReactionBatch,
    debounceMs: number
  ) {
    if (batch.timer !== null) clearTimeout(batch.timer);

    const flush = async () => {
      if (queuedIncrements.get(key) !== batch) return;
      queuedIncrements.delete(key);
      batch.timer = null;
      const result = await sendReactionRequest(
        messageId,
        reactionCode,
        true,
        batch.incrementBy
      );
      batch.resolve(result);
    };

    if (debounceMs <= 0) {
      void flush();
    } else {
      batch.timer = setTimeout(() => void flush(), debounceMs);
    }
  }

  export function executeMutation(
    messageId: number,
    reactionCode: ConversationReactionCode,
    action: 'increment' | 'remove' = 'increment',
    _currentCount: number = 0,
    debounceMs: number = typeof process !== 'undefined' && process.env?.NODE_ENV === 'test' ? 0 : 400
  ): Promise<ConversationReactionMutationResult | null> {
    const key: ReactionMutationKey = `${messageId}:${reactionCode}`;

    if (pendingReactions.has(key)) {
      return Promise.resolve(null);
    }

    if (action === 'remove') {
      cancelQueuedIncrement(key);
      return sendReactionRequest(messageId, reactionCode, false);
    }

    const existingBatch = queuedIncrements.get(key);
    if (existingBatch) {
      existingBatch.incrementBy += 1;
      scheduleIncrementBatch(key, messageId, reactionCode, existingBatch, debounceMs);
      return existingBatch.promise;
    }

    let resolveBatch: (result: ConversationReactionMutationResult | null) => void = () => {};
    const promise = new Promise<ConversationReactionMutationResult | null>((resolve) => {
      resolveBatch = resolve;
    });
    const batch: QueuedReactionBatch = {
      incrementBy: 1,
      timer: null,
      promise,
      resolve: resolveBatch
    };
    queuedIncrements.set(key, batch);
    scheduleIncrementBatch(key, messageId, reactionCode, batch, debounceMs);
    return promise;
  }

  function dismissError(id: string) {
    mutationErrors = mutationErrors.filter((e) => e.id !== id);
  }
</script>

{#if mutationErrors.length > 0}
  <div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-md" data-testid="reaction-mutation-errors">
    {#each mutationErrors as err (err.id)}
      <div
        class="flex items-center justify-between gap-3 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive shadow-md"
        role="alert"
      >
        <div class="flex items-center gap-2 min-w-0">
          <AlertCircle class="size-4 shrink-0" />
          <span class="truncate">{err.errorMessage}</span>
        </div>
        <button
          type="button"
          aria-label="Dismiss notification"
          class="rounded-sm p-0.5 hover:bg-destructive/20"
          onclick={() => dismissError(err.id)}
        >
          <X class="size-3.5" />
        </button>
      </div>
    {/each}
  </div>
{/if}
