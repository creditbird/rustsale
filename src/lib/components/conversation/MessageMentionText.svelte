<script lang="ts">
  import type {
    ConversationMention,
    ConversationMentionInput
  } from '$lib/services/conversationService';
  import { sliceUtf16 } from './mentionState';

  let {
    text,
    mentions = []
  }: {
    text: string;
    mentions?: (ConversationMention | ConversationMentionInput)[];
  } = $props();

  type MentionChunk =
    | { type: 'text'; text: string }
    | { type: 'mention'; text: string; identityId: number | null };

  let chunks = $derived.by<MentionChunk[]>(() => {
    if (!mentions || mentions.length === 0 || !text) {
      return [{ type: 'text', text }];
    }

    const sorted = [...mentions].sort((a, b) => a.startUtf16 - b.startUtf16);
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
          identityId: m.identityId
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
</script>

<span class="message-mention-text" data-testid="message-mention-text">
  {#each chunks as chunk, i (i)}
    {#if chunk.type === 'mention'}
      <span
        class="mention-span font-semibold text-primary bg-primary/10 rounded px-1 py-0.5"
        data-testid="mention-span"
        data-mention-id={chunk.identityId ?? ''}
      >{chunk.text}</span>
    {:else}
      <span>{chunk.text}</span>
    {/if}
  {/each}
</span>
