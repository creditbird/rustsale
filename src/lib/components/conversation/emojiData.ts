export type EmojiCategory = {
  id: string;
  label: string;
  emojis: string[];
};

export const EMOJI_CATEGORIES: EmojiCategory[] = [
  {
    id: 'smileys',
    label: 'Smileys & Emotion',
    emojis: ['😀', '😃', '😄', '😁', '😆', '😅', '😂', '🤣', '😊', '😍', '🥰', '😘', '😎', '🤔', '😭', '😡'],
  },
  {
    id: 'gestures',
    label: 'Gestures',
    emojis: ['👍', '👎', '👌', '✌️', '🤞', '👏', '🙏', '💪', '👋', '🤝', '🫶', '🤟'],
  },
  {
    id: 'hearts',
    label: 'Hearts',
    emojis: ['❤️', '🧡', '💛', '💚', '💙', '💜', '🖤', '🤍', '🤎', '💔', '💕', '💖'],
  },
  {
    id: 'people',
    label: 'People',
    emojis: ['👨', '👩', '👦', '👧', '👶', '👴', '👵', '👮', '👷', '💁', '🙋', '🙆'],
  },
  {
    id: 'objects',
    label: 'Objects',
    emojis: ['🎉', '🎁', '🔥', '⭐', '✨', '💡', '📌', '📞', '📅', '✅', '❌', '⚠️'],
  },
  {
    id: 'symbols',
    label: 'Symbols',
    emojis: ['✔️', '☑️', '➡️', '⬅️', '⬆️', '⬇️', '➕', '➖', '❓', '❗', '💯', '♻️'],
  },
];
