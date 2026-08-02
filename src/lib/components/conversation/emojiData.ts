export type EmojiCategory = {
  id: string;
  label: string;
  emojis: string[];
};

export const EMOJI_CATEGORIES: EmojiCategory[] = [
  {
    id: 'smileys',
    label: 'Cảm xúc',
    emojis: ['😀', '😃', '😄', '😁', '😆', '😅', '😂', '🤣', '😊', '😍', '🥰', '😘', '😎', '🤔', '😭', '😡'],
  },
  {
    id: 'gestures',
    label: 'Cử chỉ',
    emojis: ['👍', '👎', '👌', '✌️', '🤞', '👏', '🙏', '💪', '👋', '🤝', '🫶', '🤟'],
  },
  {
    id: 'hearts',
    label: 'Trái tim',
    emojis: ['❤️', '🧡', '💛', '💚', '💙', '💜', '🖤', '🤍', '🤎', '💔', '💕', '💖'],
  },
  {
    id: 'people',
    label: 'Con người',
    emojis: ['👨', '👩', '👦', '👧', '👶', '👴', '👵', '👮', '👷', '💁', '🙋', '🙆'],
  },
  {
    id: 'objects',
    label: 'Đồ vật',
    emojis: ['🎉', '🎁', '🔥', '⭐', '✨', '💡', '📌', '📞', '📅', '✅', '❌', '⚠️'],
  },
  {
    id: 'symbols',
    label: 'Ký hiệu',
    emojis: ['✔️', '☑️', '➡️', '⬅️', '⬆️', '⬇️', '➕', '➖', '❓', '❗', '💯', '♻️'],
  },
];
