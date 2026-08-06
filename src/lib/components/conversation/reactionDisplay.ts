import type { ConversationReactionCode } from '$lib/services/conversationService';

const REACTION_ALIASES: Record<ConversationReactionCode, Set<string>> = {
  heart: new Set(['❤️', '❤', '/-heart', '/:heart', 'heart']),
  like: new Set(['👍', '/-strong', '/-like', 'like']),
  haha: new Set(['😆', '😄', ':>', '/-haha', 'haha']),
  wow: new Set(['😮', ':o', '/-wow', 'wow']),
  cry: new Set(['😭', '😢', ':-(', ':-[', ':-((', ':((', '--b', '/-cry', 'cry', 'sad']),
  angry: new Set(['😡', ':-h', '/-angry', 'angry'])
};

const REACTION_EMOJI: Record<ConversationReactionCode, string> = {
  heart: '❤️',
  like: '👍',
  haha: '😆',
  wow: '😮',
  cry: '😭',
  angry: '😡'
};

export function reactionIconToCode(icon: string): ConversationReactionCode {
  const normalized = icon.trim().toLowerCase();
  for (const [code, aliases] of Object.entries(REACTION_ALIASES)) {
    if (aliases.has(normalized)) return code as ConversationReactionCode;
  }
  return 'like';
}

export function normalizeReactionIcon(icon: string): string {
  const code = reactionIconToCode(icon);
  const normalized = icon.trim().toLowerCase();
  return REACTION_ALIASES[code].has(normalized) ? REACTION_EMOJI[code] : icon;
}
