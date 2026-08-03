import type { ConversationQuickTemplate } from '$lib/services/conversationService';

export type SlashToken = {
  start: number;
  end: number;
  query: string;
};

export function findSlashToken(value: string, caret: number): SlashToken | null {
  if (caret < 0 || caret > value.length) return null;

  // Search backwards from caret to find a `/` without encountering whitespace
  for (let i = caret - 1; i >= 0; i--) {
    const char = value[i];

    if (/\s/.test(char)) {
      // Encountered whitespace before finding `/` -> no active slash token
      return null;
    }

    if (char === '/') {
      // Check character preceding `/`
      const isStartOfLineOrWhitespace = i === 0 || /\s/.test(value[i - 1]);
      if (isStartOfLineOrWhitespace) {
        return {
          start: i,
          end: caret,
          query: value.slice(i + 1, caret)
        };
      } else {
        // Preceded by non-whitespace (e.g. `http://` or `abc/def`)
        return null;
      }
    }
  }

  return null;
}

export function replaceSlashToken(
  value: string,
  token: SlashToken,
  body: string
): { value: string; caret: number } {
  const valueBefore = value.slice(0, token.start);
  const valueAfter = value.slice(token.end);
  const newValue = valueBefore + body + valueAfter;
  const newCaret = token.start + body.length;

  return {
    value: newValue,
    caret: newCaret
  };
}

export function rankQuickTemplates(
  items: ConversationQuickTemplate[],
  query: string
): ConversationQuickTemplate[] {
  const activeItems = items.filter((item) => item.isActive);
  const q = query.trim().toLowerCase();

  if (!q) {
    return activeItems.sort((a, b) => {
      if (b.usageCount !== a.usageCount) {
        return b.usageCount - a.usageCount;
      }
      return a.shortcut.localeCompare(b.shortcut);
    });
  }

  type RankedItem = {
    item: ConversationQuickTemplate;
    tier: number;
  };

  const ranked: RankedItem[] = [];

  for (const item of activeItems) {
    const shortcutLower = item.shortcut.toLowerCase();
    const titleLower = item.title.toLowerCase();
    const categoryLower = item.category?.toLowerCase() ?? '';
    const bodyLower = item.bodyText.toLowerCase();

    let tier = 0;
    if (shortcutLower === q) {
      tier = 1;
    } else if (shortcutLower.startsWith(q)) {
      tier = 2;
    } else if (shortcutLower.includes(q)) {
      tier = 3;
    } else if (
      titleLower.includes(q) ||
      categoryLower.includes(q) ||
      bodyLower.includes(q)
    ) {
      tier = 4;
    }

    if (tier > 0) {
      ranked.push({ item, tier });
    }
  }

  ranked.sort((a, b) => {
    if (a.tier !== b.tier) {
      return a.tier - b.tier;
    }
    if (b.item.usageCount !== a.item.usageCount) {
      return b.item.usageCount - a.item.usageCount;
    }
    return a.item.shortcut.localeCompare(b.item.shortcut);
  });

  return ranked.map((r) => r.item);
}
