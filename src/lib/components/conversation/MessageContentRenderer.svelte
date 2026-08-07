<script lang="ts">
  import type { ConversationMessage } from '$lib/services/conversationService';
  import ImageMessageContent from './ImageMessageContent.svelte';
  import FileMessageContent from './FileMessageContent.svelte';
  import StickerMessageContent from './StickerMessageContent.svelte';
  import VideoMessageContent from './VideoMessageContent.svelte';
  import AudioMessageContent from './AudioMessageContent.svelte';
  import LinkMessageContent from './LinkMessageContent.svelte';
  import MessageMentionText from './MessageMentionText.svelte';
  import { safeConversationUrl, extractLegacyImageUrl } from './messageMedia';

  let {
    message,
    onImageClick = () => {}
  }: {
    message: ConversationMessage;
    onImageClick?: (url: string) => void;
  } = $props();

  let displayText = $derived(
    typeof message.textContent === 'string' && message.textContent.trim() !== ''
      ? message.textContent
      : typeof message.content?.caption === 'string' && message.content.caption.trim() !== ''
        ? message.content.caption
        : typeof message.content?.description === 'string' && message.content.description.trim() !== ''
          ? message.content.description
          : typeof message.content?.title === 'string' && message.content.title.trim() !== ''
            ? message.content.title
            : typeof message.content?.text === 'string' && message.content.text.trim() !== ''
              ? message.content.text
              : typeof message.content?.message === 'string' && message.content.message.trim() !== ''
                ? message.content.message
                : typeof message.content?.subTitle === 'string' && message.content.subTitle.trim() !== ''
                  ? message.content.subTitle
                  : null
  );

  let isImageContentType = $derived(
    message.contentType === 'image' || message.contentType.startsWith('image/')
  );
  let isLinkContentType = $derived(message.contentType === 'link');

  let rawLegacyUrl = $derived(
    typeof message.content?.url === 'string'
      ? message.content.url
      : typeof message.content?.src === 'string'
        ? message.content.src
        : typeof message.content?.href === 'string'
          ? message.content.href
          : typeof message.content?.path === 'string'
            ? message.content.path
            : null
  );

  let legacyMediaUrl = $derived(rawLegacyUrl);
  let safeLegacyUrl = $derived(extractLegacyImageUrl(message.content));
  let isUnsafeLegacyImage = $derived(
    isImageContentType && legacyMediaUrl !== null && safeLegacyUrl === null
  );
</script>

<div class="message-content-renderer" data-testid="message-content-renderer">
  {#if message.contentType === 'sticker' && message.attachments && message.attachments.length > 0}
    <StickerMessageContent attachment={message.attachments[0]} />
  {:else}
    {#if message.attachments && message.attachments.length > 0}
      <div class="message-attachments-list">
        {#each message.attachments as att (att.id)}
          {#if att.mediaType === 'image'}
            <ImageMessageContent attachment={att} {onImageClick} />
          {:else if att.mediaType === 'sticker'}
            <StickerMessageContent attachment={att} />
          {:else if att.mediaType === 'video'}
            <VideoMessageContent attachment={att} />
          {:else if att.mediaType === 'audio' || att.mediaType === 'voice'}
            <AudioMessageContent attachment={att} />
          {:else}
            <FileMessageContent attachment={att} />
          {/if}
        {/each}
      </div>
    {:else if isImageContentType}
      {#if safeLegacyUrl}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="overflow-hidden rounded-lg border border-border/30 my-1 cursor-pointer transition-transform hover:scale-[1.02]"
          onclick={() => safeLegacyUrl && onImageClick(safeLegacyUrl)}
        >
          <img
            src={safeLegacyUrl}
            alt={displayText || 'Hình ảnh'}
            class="max-h-64 max-w-full object-cover rounded-lg"
            loading="lazy"
          />
        </div>
      {:else if isUnsafeLegacyImage}
        <div class="flex items-center gap-2 p-2 rounded-md bg-destructive/10 text-destructive text-xs font-medium border border-destructive/20">
          <span>File đính kèm không hợp lệ</span>
        </div>
      {/if}
    {/if}

    {#if isLinkContentType}
      <LinkMessageContent content={message.content} textContent={message.textContent} />
    {/if}

    {#if displayText && !isLinkContentType}
      <div class="message-text-content" data-testid="message-text-content">
        <MessageMentionText text={displayText} mentions={message.mentions ?? []} />
      </div>
    {:else if !message.attachments?.length && !legacyMediaUrl && message.contentType !== 'text'}
      <div class="flex items-center gap-2 p-2 rounded-md bg-muted/40 text-muted-foreground text-xs italic">
        <span>Nội dung chưa được hỗ trợ</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .message-content-renderer {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .message-attachments-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .message-text-content {
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
