<script lang="ts">
  import type { ConversationReplySelection } from './composerDraftState';

  let {
    reply = null,
    onCancel = () => {}
  }: {
    reply?: ConversationReplySelection | null;
    onCancel?: () => void;
  } = $props();
</script>

{#if reply}
  <div class="composer-reply-bar flex items-center gap-2 border-t border-border/40 bg-muted/30 p-2 text-xs" data-testid="composer-reply-bar">
    <div class="reply-indicator h-6 w-[3px] rounded-sm bg-blue-500"></div>
    <div class="reply-content flex flex-1 flex-col overflow-hidden">
      <span class="reply-sender font-semibold text-blue-500">{reply.preview.senderDisplayName ?? 'Tin nhắn'}</span>
      <span class="reply-text truncate text-muted-foreground">{reply.preview.textContent ?? reply.preview.contentType}</span>
    </div>
    <button
      type="button"
      class="reply-cancel-btn cursor-pointer rounded border-none bg-transparent px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
      aria-label="Cancel reply"
      onclick={onCancel}
      data-testid="cancel-reply-btn"
    >
      ✕
    </button>
  </div>
{/if}

