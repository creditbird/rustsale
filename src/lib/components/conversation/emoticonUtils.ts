export const EMOTICON_MAP: Record<string, string> = {
  ":')": '🥲',
  ":'(": '😭',
  "O:-)": '😇',
  "o:-)": '😇',
  "O:)": '😇',
  "o:)": '😇',
  "B-)": '😎',
  "b-)": '😎',
  "B)": '😎',
  "b)": '😎',
  "3:-)": '😈',
  "3:)": '😈',
  "</3": '💔',
  "<3": '❤️',
  "(y)": '👍',
  "(Y)": '👍',
  ":-)": '😊',
  ":)": '😊',
  ":-D": '😀',
  ":D": '😀',
  ":d": '😀',
  ":-(": '🙁',
  ":(": '🙁',
  ";-)": '😉',
  ";)": '😉',
  ":-P": '😛',
  ":P": '😛',
  ":-p": '😛',
  ":p": '😛',
  ":-O": '😮',
  ":O": '😮',
  ":-o": '😮',
  ":o": '😮',
  ":-/": '😕',
  ":/": '😕',
  ":-\\": '😕',
  ":\\": '😕',
  ":-*": '😗',
  ":*": '😗',
  ":-|": '😐',
  ":|": '😐',
  ":v": '✌️',
  ":V": '✌️',
  "xD": '😆',
  "XD": '😆',
  "^_^": '😊',
  "^^": '😊',
  "T_T": '😭',
  "T-T": '😭',
  ">.<": '😣',
  ">_<": '😣'
};

const escapeRegExp = (str: string) => str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const emoticonKeys = Object.keys(EMOTICON_MAP).sort((a, b) => b.length - a.length);
const emoticonPattern = emoticonKeys.map(escapeRegExp).join('|');

const EMOTICON_REGEX = new RegExp(
  `(^|\\s)(${emoticonPattern})(?=\\s|$|[.,!?;:])`,
  'g'
);

/**
 * Parses ASCII text emoticons in text and replaces them with Unicode emojis.
 */
export function parseEmoticons(text: string): string {
  if (!text) return text;
  return text.replace(EMOTICON_REGEX, (match, prefix, emoticon) => {
    return prefix + (EMOTICON_MAP[emoticon] || emoticon);
  });
}
