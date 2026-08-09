import type { ConversationReactionCode } from '$lib/services/conversationService';

const REACTION_ALIASES: Record<ConversationReactionCode, Set<string>> = {
  heart: new Set(['❤️', '❤', '/-heart', '/:heart', 'heart']),
  like: new Set(['👍', '/-strong', '/:strong', '/-like', '/:like', 'like']),
  haha: new Set(['😆', '😄', ':>', '/-haha', '/:haha', 'haha']),
  wow: new Set(['😮', ':o', '/-wow', '/:wow', 'wow']),
  cry: new Set(['😭', '😢', ':-(', ':-[', ':-((', ':((', '--b', '/-cry', '/:cry', '/-sad', '/:sad', 'cry', 'sad']),
  angry: new Set(['😡', ':-h', '/-angry', '/:angry', 'angry'])
};

const REACTION_EMOJI: Record<ConversationReactionCode, string> = {
  heart: '❤️',
  like: '👍',
  haha: '😆',
  wow: '😮',
  cry: '😭',
  angry: '😡'
};

export function reactionIconToCode(icon: string): ConversationReactionCode | null {
  const normalized = icon.trim().toLowerCase();
  for (const [code, aliases] of Object.entries(REACTION_ALIASES)) {
    if (aliases.has(normalized)) return code as ConversationReactionCode;
  }
  return null;
}

export function normalizeReactionIcon(icon: string): string {
  const code = reactionIconToCode(icon);
  if (code === null) return icon;
  const normalized = icon.trim().toLowerCase();
  return REACTION_ALIASES[code].has(normalized) ? REACTION_EMOJI[code] : icon;
}
