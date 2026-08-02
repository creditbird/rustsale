<script lang="ts">
  import { safeConversationUrl } from './messageMedia';

  let {
    content,
    textContent = null
  }: {
    content: Record<string, unknown>;
    textContent?: string | null;
  } = $props();

  let rawUrl = $derived(
    typeof content.url === 'string'
      ? content.url
      : typeof content.href === 'string'
        ? content.href
        : null
  );
  let safeUrl = $derived(safeConversationUrl(rawUrl));
  let title = $derived(
    typeof content.title === 'string' && content.title.trim()
      ? content.title
      : textContent || safeUrl || 'Liên kết'
  );
  let description = $derived(
    typeof content.description === 'string' && content.description.trim()
      ? content.description
      : null
  );
  let thumbnailUrl = $derived(
    safeConversationUrl(
      typeof content.thumbnailUrl === 'string'
        ? content.thumbnailUrl
        : typeof content.thumbnail === 'string'
          ? content.thumbnail
          : null
    )
  );
</script>

{#if safeUrl}
  <a
    href={safeUrl}
    target="_blank"
    rel="noopener noreferrer"
    class="flex overflow-hidden rounded-lg border border-border/40 bg-background/30 text-foreground no-underline hover:bg-background/50"
    data-testid="link-message-content"
  >
    {#if thumbnailUrl}
      <img src={thumbnailUrl} alt="" class="h-20 w-20 shrink-0 object-cover" loading="lazy" />
    {/if}
    <span class="min-w-0 p-2">
      <span class="block truncate text-sm font-semibold">{title}</span>
      {#if description}
        <span class="mt-1 line-clamp-2 block text-xs opacity-75">{description}</span>
      {/if}
      <span class="mt-1 block truncate text-[11px] opacity-60">{safeUrl}</span>
    </span>
  </a>
{:else}
  <div class="rounded-md border border-destructive/20 bg-destructive/10 p-2 text-xs font-medium text-destructive" data-testid="link-message-content">
    Liên kết không hợp lệ
  </div>
{/if}
