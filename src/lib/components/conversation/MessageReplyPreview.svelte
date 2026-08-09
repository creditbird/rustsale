<script lang="ts">
  import type { ConversationReplyTarget } from '$lib/services/conversationService';

  let { replyTarget = null }: { replyTarget?: ConversationReplyTarget | null } = $props();
</script>

{#if replyTarget}
  <div class="message-reply-preview mb-1 flex items-center gap-1.5 rounded bg-muted/40 p-1 px-2 text-[11px]" data-testid="message-reply-preview">
    <div class="reply-line h-5 w-[2px] rounded-[1px] bg-blue-500"></div>
    <div class="reply-body flex flex-col overflow-hidden">
      <span class="reply-sender font-semibold text-blue-500">{replyTarget.senderDisplayName ?? 'Tin nhắn'}</span>
      <span class="reply-text truncate text-muted-foreground">{replyTarget.textContent ?? replyTarget.contentType}</span>
    </div>
  </div>
{/if}

