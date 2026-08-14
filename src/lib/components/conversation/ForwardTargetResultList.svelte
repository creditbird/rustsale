<script lang="ts">
  import type { ConversationForwardTargetResult } from '$lib/services/conversationService';
  import { canRetryForwardTarget } from './forwardState';
  import { Button } from '$lib/components/ui/button';
  import { RefreshCw, CheckCircle2, AlertCircle, Clock, HelpCircle } from 'lucide-svelte';

  let {
    targets = [],
    targetTitles = {},
    retryingTargetId = null,
    onRetryTarget = () => {}
  }: {
    targets: ConversationForwardTargetResult[];
    targetTitles?: Record<number, string>;
    retryingTargetId?: number | null;
    onRetryTarget?: (targetConversationId: number) => void;
  } = $props();

  function getStatusLabel(status: ConversationForwardTargetResult['status']): string {
    switch (status) {
      case 'queued':
        return 'Queued';
      case 'sending':
        return 'Sending';
      case 'sent':
        return 'Sent';
      case 'failed':
        return 'Failed';
      case 'delivery_unknown':
        return 'Status Unknown';
      default:
        return status;
    }
  }

  function getBadgeClass(status: ConversationForwardTargetResult['status']): string {
    switch (status) {
      case 'sent':
        return 'bg-emerald-500/10 text-emerald-600 border-emerald-500/20';
      case 'failed':
        return 'bg-destructive/10 text-destructive border-destructive/20';
      case 'delivery_unknown':
        return 'bg-amber-500/10 text-amber-600 border-amber-500/20';
      case 'sending':
      case 'queued':
      default:
        return 'bg-blue-500/10 text-blue-600 border-blue-500/20';
    }
  }
</script>

<div class="space-y-2 text-sm" data-testid="forward-target-result-list">
  <h4 class="font-medium text-xs uppercase tracking-wider text-muted-foreground">
    Forwarding results ({targets.length})
  </h4>

  <div class="divide-y divide-border/60 rounded-md border border-border bg-card">
    {#each targets as target (target.targetConversationId)}
      <div
        class="flex items-center justify-between gap-3 p-2.5 text-xs"
        data-testid={`forward-target-result-${target.targetConversationId}`}
      >
        <div class="min-w-0 flex-1 space-y-0.5">
          <div class="flex items-center gap-2">
            <span class="font-medium text-foreground truncate">
              {targetTitles[target.targetConversationId] || `Conversation #${target.targetConversationId}`}
            </span>
            <span
              class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold {getBadgeClass(
                target.status
              )}"
              data-slot="target-status-badge"
            >
              {#if target.status === 'sent'}
                <CheckCircle2 class="size-3" />
              {:else if target.status === 'failed'}
                <AlertCircle class="size-3" />
              {:else if target.status === 'delivery_unknown'}
                <HelpCircle class="size-3" />
              {:else}
                <Clock class="size-3" />
              {/if}
              <span>{getStatusLabel(target.status)}</span>
            </span>
          </div>

          {#if target.errorMessage}
            <p class="text-[11px] text-destructive truncate">{target.errorMessage}</p>
          {/if}
        </div>

        {#if canRetryForwardTarget(target)}
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={retryingTargetId === target.targetConversationId}
            onclick={() => onRetryTarget(target.targetConversationId)}
            class="h-7 px-2 text-xs flex items-center gap-1 shrink-0"
            data-testid={`retry-forward-target-${target.targetConversationId}`}
          >
            <RefreshCw
              class="size-3 {retryingTargetId === target.targetConversationId ? 'animate-spin' : ''}"
            />
            <span>Retry</span>
          </Button>
        {/if}
      </div>
    {/each}
  </div>
</div>
