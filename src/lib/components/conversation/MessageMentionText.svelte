<script lang="ts">
  import type {
    ConversationMention,
    ConversationMentionInput
  } from '$lib/services/conversationService';
  import { sliceUtf16 } from './mentionState';
  import { parseEmoticons } from './emoticonUtils';
  import { openExternalUrl } from './messageMedia';

  let {
    text,
    mentions = [],
    onMentionClick = () => {}
  }: {
    text: string;
    mentions?: (ConversationMention | ConversationMentionInput)[];
    onMentionClick?: (identityId: number | null, mentionText: string) => void;
  } = $props();

  type MentionChunk =
    | { type: 'text'; text: string }
    | { type: 'mention'; text: string; identityId: number | null };

  const MENTION_REGEX = /@(\p{Lu}[\p{L}0-9._]*(?:\s\p{Lu}[\p{L}0-9._]*){0,2}(?:\s[-–—]\s\p{Lu}[\p{L}0-9._]*(?:\s\p{Lu}[\p{L}0-9._]*){0,2})?)/gu;
  const URL_REGEX = /(https?:\/\/[^\s<]+)/g;

  let chunks = $derived.by<MentionChunk[]>(() => {
    if (!text) return [];

    let mentionItems = [...mentions];

    // Fallback regex if structured mentions array is empty
    if (mentionItems.length === 0) {
      let match: RegExpExecArray | null;
      MENTION_REGEX.lastIndex = 0;
      while ((match = MENTION_REGEX.exec(text)) !== null) {
        mentionItems.push({
          identityId: null,
          externalId: null,
          displayText: match[0],
          startUtf16: match.index,
          lengthUtf16: match[0].length
        });
      }
    }

    if (mentionItems.length === 0) {
      return [{ type: 'text', text }];
    }

    const sorted = [...mentionItems].sort((a, b) => a.startUtf16 - b.startUtf16);
    const result: MentionChunk[] = [];
    let cursor = 0;

    for (const m of sorted) {
      const start = Math.max(cursor, Math.min(m.startUtf16, text.length));
      const len = Math.max(0, Math.min(m.lengthUtf16, text.length - start));
      const end = start + len;

      if (start > cursor) {
        result.push({
          type: 'text',
          text: sliceUtf16(text, cursor, start - cursor)
        });
      }

      if (len > 0) {
        result.push({
          type: 'mention',
          text: sliceUtf16(text, start, len),
          identityId: 'identityId' in m ? m.identityId : null
        });
      }

      cursor = Math.max(cursor, end);
    }

    if (cursor < text.length) {
      result.push({
        type: 'text',
        text: sliceUtf16(text, cursor, text.length - cursor)
      });
    }

    return result;
  });

  function parseTextWithUrls(str: string) {
    if (!str) return [];
    const parts: { type: 'plain' | 'url'; value: string }[] = [];
    let lastIdx = 0;
    URL_REGEX.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = URL_REGEX.exec(str)) !== null) {
      if (m.index > lastIdx) {
        parts.push({ type: 'plain', value: str.substring(lastIdx, m.index) });
      }
      parts.push({ type: 'url', value: m[0] });
      lastIdx = m.index + m[0].length;
    }
    if (lastIdx < str.length) {
      parts.push({ type: 'plain', value: str.substring(lastIdx) });
    }
    return parts;
  }
</script>

<span class="message-mention-text" data-testid="message-mention-text">
  {#each chunks as chunk, i (i)}
    {#if chunk.type === 'mention'}
      <span
        role="button"
        tabindex="0"
        class="mention-span font-semibold text-blue-600 dark:text-blue-400 bg-blue-500/10 dark:bg-blue-400/15 rounded px-1.5 py-0.5 cursor-pointer hover:underline hover:bg-blue-500/20 transition-colors inline-flex items-center gap-0.5"
        data-testid="mention-span"
        data-mention-id={chunk.identityId ?? ''}
        onclick={(e) => {
          e.stopPropagation();
          onMentionClick(chunk.identityId, chunk.text);
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.stopPropagation();
            onMentionClick(chunk.identityId, chunk.text);
          }
        }}
        title={`Tag ${chunk.text}`}
      >{chunk.text}</span>
    {:else}
      {#each parseTextWithUrls(chunk.text) as subPart, j (j)}
        {#if subPart.type === 'url'}
          <a
            href={subPart.value}
            target="_blank"
            rel="noopener noreferrer"
            class="text-blue-600 dark:text-blue-400 underline font-medium hover:opacity-80 break-all"
            onclick={(e) => {
              e.stopPropagation();
              e.preventDefault();
              void openExternalUrl(subPart.value);
            }}
          >{subPart.value}</a>
        {:else}
          <span>{parseEmoticons(subPart.value)}</span>
        {/if}
      {/each}
    {/if}
  {/each}
</span>
