import type { ConversationMentionInput } from '$lib/services/conversationService';

export interface ComposerMentionRange {
  identityId: number;
  displayText: string;
  startUtf16: number;
  lengthUtf16: number;
}

export interface MentionToken {
  start: number;
  end: number;
  query: string;
}

export function utf16Length(s: string): number {
  return s.length;
}

export function sliceUtf16(s: string, start: number, len: number): string {
  return s.slice(start, start + len);
}

export function findMentionToken(value: string, caret: number): MentionToken | null {
  if (caret <= 0 || caret > value.length) return null;

  // Search backward from caret - 1 for '@'
  for (let i = caret - 1; i >= 0; i -= 1) {
    const char = value[i];
    if (char === '@') {
      const isStart = i === 0 || /\s/.test(value[i - 1]);
      if (!isStart) return null;

      const query = value.slice(i + 1, caret);
      if (/\s/.test(query)) return null;

      return {
        start: i,
        end: caret,
        query
      };
    } else if (/\s/.test(char)) {
      // Mention token query cannot cross whitespace
      return null;
    }
  }

  return null;
}

export function applyTextEditToMentions(
  _previousValue: string,
  _nextValue: string,
  edit: { start: number; deleteCount: number; insertedText: string },
  mentions: ComposerMentionRange[]
): ComposerMentionRange[] {
  const delta = edit.insertedText.length - edit.deleteCount;
  const editEnd = edit.start + edit.deleteCount;

  const result: ComposerMentionRange[] = [];

  for (const m of mentions) {
    const mEnd = m.startUtf16 + m.lengthUtf16;

    if (editEnd <= m.startUtf16) {
      // Edit is strictly before mention: shift mention
      result.push({
        ...m,
        startUtf16: m.startUtf16 + delta
      });
    } else if (edit.start >= mEnd) {
      // Edit is strictly after mention: leave unchanged
      result.push(m);
    } else {
      // Edit overlaps mention range: mention is invalidated and removed
    }
  }

  return result;
}

export function insertMention(
  value: string,
  token: MentionToken,
  candidate: { identityId: number; displayName: string },
  currentMentions: ComposerMentionRange[] = []
): { value: string; caret: number; mentions: ComposerMentionRange[] } {
  const mentionText = `@${candidate.displayName}`;
  const inserted = `${mentionText} `;
  const before = value.slice(0, token.start);
  const after = value.slice(token.end);

  const newValue = before + inserted + after;
  const caret = token.start + inserted.length;

  const tokenLength = token.end - token.start;
  const delta = inserted.length - tokenLength;

  const newMention: ComposerMentionRange = {
    identityId: candidate.identityId,
    displayText: mentionText,
    startUtf16: token.start,
    lengthUtf16: mentionText.length
  };

  const updatedMentions: ComposerMentionRange[] = [];

  for (const m of currentMentions) {
    const mEnd = m.startUtf16 + m.lengthUtf16;
    if (token.end <= m.startUtf16) {
      // Mention is after token edit point
      updatedMentions.push({
        ...m,
        startUtf16: m.startUtf16 + delta
      });
    } else if (token.start >= mEnd) {
      // Mention is before token edit point
      updatedMentions.push(m);
    }
    // Overlapping mentions are discarded
  }

  updatedMentions.push(newMention);
  updatedMentions.sort((a, b) => a.startUtf16 - b.startUtf16);

  return {
    value: newValue,
    caret,
    mentions: updatedMentions
  };
}

export function toMentionInputs(mentions: ComposerMentionRange[]): ConversationMentionInput[] {
  return mentions
    .slice()
    .sort((a, b) => a.startUtf16 - b.startUtf16)
    .map(({ identityId, displayText, startUtf16, lengthUtf16 }) => ({
      identityId,
      displayText,
      startUtf16,
      lengthUtf16
    }));
}
