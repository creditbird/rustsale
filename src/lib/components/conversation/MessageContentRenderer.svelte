<script lang="ts">
  import type { ConversationMessage } from '$lib/services/conversationService';
  import ImageMessageContent from './ImageMessageContent.svelte';
  import FileMessageContent from './FileMessageContent.svelte';
  import StickerMessageContent from './StickerMessageContent.svelte';
  import VideoMessageContent from './VideoMessageContent.svelte';
  import AudioMessageContent from './AudioMessageContent.svelte';
  import LinkMessageContent from './LinkMessageContent.svelte';
  import MessageMentionText from './MessageMentionText.svelte';
  import { safeConversationUrl, extractLegacyImageUrl, extractStickerUrl, isImageUrl, isVideoUrl } from './messageMedia';

  let {
    message,
    onImageClick = () => {},
    onMentionClick = () => {}
  }: {
    message: ConversationMessage;
    onImageClick?: (url: string) => void;
    onMentionClick?: (identityId: number | null, mentionText: string, externalId?: string, targetEl?: HTMLElement) => void;
  } = $props();

  let stickerAttachment = $derived(
    message.attachments?.find((att) => att.mediaType === 'sticker') ?? message.attachments?.[0] ?? null
  );

  let isStickerMessage = $derived(
    message.contentType === 'sticker' ||
      message.textContent === '[Sticker]' ||
      (typeof message.content === 'object' &&
        message.content !== null &&
        ('stickerId' in message.content || 'cateId' in message.content))
  );

  let rawLegacyUrl = $derived(
    typeof message.content?.url === 'string'
      ? message.content.url
      : typeof message.content?.href === 'string'
        ? message.content.href
        : typeof message.content?.link === 'string'
          ? message.content.link
          : typeof message.content?.path === 'string'
            ? message.content.path
            : null
  );

  let legacyMediaUrl = $derived(rawLegacyUrl);
  let safeLegacyUrl = $derived(extractLegacyImageUrl(message.content));

  let rawDisplayText = $derived(
    typeof message.textContent === 'string' && message.textContent.trim() !== '' && message.textContent.trim() !== '[Sticker]'
      ? message.textContent
      : typeof message.content?.caption === 'string' && message.content.caption.trim() !== ''
        ? message.content.caption
        : typeof message.content?.description === 'string' && message.content.description.trim() !== ''
          ? message.content.description
          : typeof message.content?.title === 'string' && message.content.title.trim() !== ''
            ? message.content.title
            : typeof message.content?.text === 'string' && message.content.text.trim() !== '' && message.content.text.trim() !== '[Sticker]'
              ? message.content.text
              : typeof message.content?.message === 'string' && message.content.message.trim() !== '' && message.content.message.trim() !== '[Sticker]'
                ? message.content.message
                : typeof message.content?.subTitle === 'string' && message.content.subTitle.trim() !== ''
                  ? message.content.subTitle
                  : null
  );

  let isImageLink = $derived(
    isImageUrl(safeLegacyUrl) || isImageUrl(rawLegacyUrl) || isImageUrl(rawDisplayText)
  );

  let isImageContentType = $derived(
    message.contentType === 'image' ||
      message.contentType?.startsWith('image/') ||
      isImageLink
  );

  let isVideoLink = $derived(
    isVideoUrl(safeLegacyUrl) ||
      isVideoUrl(rawLegacyUrl) ||
      (typeof message.content?.mediaType === 'string' && message.content.mediaType.startsWith('video')) ||
      (typeof message.content?.type === 'string' && message.content.type.startsWith('video')) ||
      (isVideoUrl(rawDisplayText) && message.attachments?.length === 0)
  );

  let hasVideoAttachment = $derived(
    Boolean(message.attachments?.some((att) => att.mediaType === 'video'))
  );

  let isVideoContentType = $derived(
    message.contentType === 'video' ||
      message.contentType?.startsWith('video/') ||
      hasVideoAttachment ||
      isVideoLink
  );

  let legacyVideoUrl = $derived(
    safeLegacyUrl || (isVideoUrl(rawDisplayText) ? safeConversationUrl(rawDisplayText) : null)
  );

  let syntheticVideoAttachment = $derived.by(() => {
    if (hasVideoAttachment || !isVideoContentType || !legacyVideoUrl) return null;
    return {
      id: 0,
      mediaType: 'video',
      mimeType: 'video/mp4',
      fileName: null,
      sizeBytes: null,
      remoteUrl: legacyVideoUrl,
      thumbnailUrl:
        typeof message.content?.thumbUrl === 'string'
          ? message.content.thumbUrl
          : typeof message.content?.thumbnailUrl === 'string'
            ? message.content.thumbnailUrl
            : typeof message.content?.thumb === 'string'
              ? message.content.thumb
              : typeof message.content?.thumbnail === 'string'
                ? message.content.thumbnail
                : null,
      width: null,
      height: null,
      durationSeconds: null
    };
  });

  let isVideoLinkPreview = $derived(
    message.contentType === 'video' ||
      message.contentType?.startsWith('video/') ||
      isVideoLink
  );

  let hasLinkPreviewData = $derived(
    !isImageContentType &&
      !isVideoLinkPreview &&
      typeof message.content === 'object' &&
      message.content !== null &&
      (typeof message.content.url === 'string' ||
        typeof message.content.href === 'string' ||
        typeof message.content.link === 'string') &&
      (typeof message.content.title === 'string' ||
        typeof message.content.description === 'string' ||
        typeof message.content.thumbnailUrl === 'string' ||
        typeof message.content.thumbnail === 'string' ||
        typeof message.content.thumb === 'string')
  );

  let isLinkContentType = $derived(
    !isImageContentType &&
      !isVideoLinkPreview &&
      (message.contentType === 'link' || hasLinkPreviewData)
  );

  let isUnsafeLegacyImage = $derived(
    isImageContentType && legacyMediaUrl !== null && safeLegacyUrl === null
  );

  let shouldSuppressVideoText = $derived(
    isVideoContentType &&
      (rawDisplayText === legacyVideoUrl ||
        rawDisplayText === rawLegacyUrl ||
        isVideoUrl(rawDisplayText) ||
        (message.textContent === null && (rawDisplayText === message.content?.title || rawDisplayText === message.content?.description)))
  );

  let displayText = $derived(shouldSuppressVideoText ? null : rawDisplayText);
</script>

<div class="message-content-renderer flex flex-col gap-1" data-testid="message-content-renderer">
  {#if isStickerMessage}
    <StickerMessageContent attachment={stickerAttachment} content={message.content} conversationId={message.conversationId} />
  {:else}
    {#if message.attachments && message.attachments.length > 0}
      <div class="message-attachments-list flex flex-col gap-[6px]">
        {#each message.attachments as att (att.id)}
          {#if att.mediaType === 'image'}
            <ImageMessageContent attachment={att} {onImageClick} />
          {:else if att.mediaType === 'sticker'}
            <StickerMessageContent attachment={att} content={message.content} conversationId={message.conversationId} />
          {:else if att.mediaType === 'video'}
            <VideoMessageContent attachment={att} />
          {:else if att.mediaType === 'audio' || att.mediaType === 'voice'}
            <AudioMessageContent attachment={att} />
          {:else}
            <FileMessageContent attachment={att} />
          {/if}
        {/each}
      </div>
    {:else if syntheticVideoAttachment}
      <div class="message-attachments-list flex flex-col gap-[6px]">
        <VideoMessageContent attachment={syntheticVideoAttachment} />
      </div>
    {:else if isImageContentType}
      {@const imageUrl = safeLegacyUrl || safeConversationUrl(rawDisplayText)}
      {#if imageUrl}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="overflow-hidden rounded-lg border border-border/30 my-1 cursor-pointer transition-transform hover:scale-[1.02]"
          onclick={() => imageUrl && onImageClick(imageUrl)}
        >
          <img
            src={imageUrl}
            alt={displayText || 'Image'}
            class="max-h-64 max-w-full object-cover rounded-lg"
            loading="lazy"
          />
        </div>
      {:else if isUnsafeLegacyImage}
        <div class="flex items-center gap-2 p-2 rounded-md bg-destructive/10 text-destructive text-xs font-medium border border-destructive/20">
          <span>Invalid attachment file</span>
        </div>
      {/if}
    {/if}

    {#if isLinkContentType}
      <LinkMessageContent content={message.content} textContent={message.textContent} />
    {/if}

    {#if displayText}
      <div class="message-text-content whitespace-pre-wrap [word-break:break-word]" data-testid="message-text-content">
        <MessageMentionText text={displayText} mentions={message.mentions ?? []} {onMentionClick} />
      </div>
    {:else if !message.attachments?.length && !syntheticVideoAttachment && !legacyMediaUrl && message.contentType !== 'text' && !isLinkContentType}
      <div class="flex items-center gap-2 p-2 rounded-md bg-muted/40 text-muted-foreground text-xs italic">
        <span>Unsupported content</span>
      </div>
    {/if}
  {/if}
</div>

