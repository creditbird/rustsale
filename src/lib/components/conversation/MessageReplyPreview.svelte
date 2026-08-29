<script lang="ts">
  import type { ConversationReplyTarget } from '$lib/services/conversationService';
  import { Image, Video, FileText, Mic, Sticker } from 'lucide-svelte';

  let {
    replyTarget = null,
    onClick = () => {}
  }: {
    replyTarget?: ConversationReplyTarget | null;
    onClick?: () => void;
  } = $props();

  let senderName = $derived.by(() => {
    if (!replyTarget?.senderDisplayName) return 'Message';
    return replyTarget.senderDisplayName.trim().replace(/^~\s*/, '');
  });
</script>

{#if replyTarget}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="message-reply-preview mb-1.5 flex items-stretch gap-2 rounded-lg bg-black/[0.04] dark:bg-white/[0.06] hover:bg-black/[0.07] dark:hover:bg-white/[0.09] transition-colors p-1.5 px-2.5 text-xs border-l-[3.5px] border-[#00a884] dark:border-[#25d366] cursor-pointer select-none overflow-hidden"
    data-testid="message-reply-preview"
    onclick={onClick}
  >
    <div class="flex-1 min-w-0 flex flex-col justify-center gap-0.5">
      <span class="reply-sender font-semibold text-[#00a884] dark:text-[#25d366] truncate text-[11.5px]">
        {senderName}
      </span>
      <div class="flex items-center gap-1 text-muted-foreground truncate text-[12px]">
        {#if replyTarget.mediaType === 'image'}
          <Image class="size-3.5 shrink-0 text-muted-foreground" />
          <span>Image</span>
        {:else if replyTarget.mediaType === 'video'}
          <Video class="size-3.5 shrink-0 text-muted-foreground" />
          <span>Video</span>
        {:else if replyTarget.mediaType === 'audio' || replyTarget.mediaType === 'voice'}
          <Mic class="size-3.5 shrink-0 text-muted-foreground" />
          <span>Audio</span>
        {:else if replyTarget.mediaType === 'sticker'}
          <Sticker class="size-3.5 shrink-0 text-muted-foreground" />
          <span>Sticker</span>
        {:else if replyTarget.mediaType === 'file' || replyTarget.mediaType === 'document'}
          <FileText class="size-3.5 shrink-0 text-muted-foreground" />
          <span>Document</span>
        {/if}
        {#if replyTarget.textContent}
          <span class="reply-text truncate">{replyTarget.textContent}</span>
        {:else if !replyTarget.mediaType && replyTarget.contentType}
          <span class="reply-text truncate capitalize">{replyTarget.contentType}</span>
        {/if}
      </div>
    </div>
  </div>
{/if}

