import type { ConversationReactionCode } from '$lib/services/conversationService';

const REACTION_ALIASES: Record<string, Set<string>> = {
  heart: new Set(['❤️', '❤', '/-heart', '/:heart', 'heart']),
  like: new Set(['👍', '/-strong', '/:strong', '/-like', '/:like', 'like']),
  haha: new Set(['😆', '😄', '😂', ':>', '/-haha', '/:haha', 'haha']),
  wow: new Set(['😮', '😲', ':o', '/-wow', '/:wow', 'wow']),
  cry: new Set(['😭', '😢', '🥺', ':-(', ':-[', ':-((', ':((', '--b', '/-cry', '/:cry', '/-sad', '/:sad', 'cry', 'sad']),
  angry: new Set(['😡', '😠', '👿', ':-h', '/-angry', '/:angry', 'angry'])
};

const REACTION_EMOJI: Record<string, string> = {
  heart: '❤️',
  like: '👍',
  haha: '😆',
  wow: '😮',
  cry: '😭',
  angry: '😡'
};

export function reactionCodeToIcon(code: ConversationReactionCode): string {
  return REACTION_EMOJI[code as string] ?? (code as string);
}

export function reactionIconToCode(icon: string): ConversationReactionCode | null {
  if (!icon) return null;
  const normalized = icon.trim().toLowerCase();
  for (const [code, aliases] of Object.entries(REACTION_ALIASES)) {
    if (aliases.has(normalized)) return code as ConversationReactionCode;
  }
  if (normalized.startsWith('/-') || normalized.startsWith('/:') || normalized.startsWith(':')) {
    return null;
  }
  if (normalized.length > 0) {
    return icon.trim() as ConversationReactionCode;
  }
  return null;
}

export function normalizeReactionIcon(icon: string): string {
  if (!icon) return '';
  const code = reactionIconToCode(icon);
  if (code === null) return icon;
  const normalized = icon.trim().toLowerCase();
  if (REACTION_ALIASES[code as string]?.has(normalized)) {
    return REACTION_EMOJI[code as string];
  }
  return icon;
}
