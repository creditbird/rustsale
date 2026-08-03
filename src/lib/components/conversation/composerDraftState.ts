import type { ConversationReplyTarget } from '$lib/services/conversationService';
import type { ComposerMentionRange } from './mentionState';

export interface ConversationReplySelection {
  messageId: number;
  conversationId: number;
  channelAccountId: number;
  preview: ConversationReplyTarget;
}

export type ComposerAttachmentStatus = 'staging' | 'ready' | 'error' | 'removing';

export interface ComposerAttachmentItem {
  localId: string;
  stagedAttachmentId: string | null;
  fileName: string;
  mimeType: string;
  sizeBytes: number;
  attachmentType: string;
  previewDataUrl: string | null;
  status: ComposerAttachmentStatus;
  errorMessage: string | null;
}

export interface ComposerDraft {
  text: string;
  reply: ConversationReplySelection | null;
  attachments: ComposerAttachmentItem[];
  mentions: ComposerMentionRange[];
}

export interface ComposerDraftSnapshot {
  text: string;
  stagedAttachmentIds: string[];
  mentions: ComposerMentionRange[];
}

export function composerDraftKey(conversationId: number, channelAccountId: number): string {
  return `${conversationId}:${channelAccountId}`;
}

export function createEmptyComposerDraft(): ComposerDraft {
  return {
    text: '',
    reply: null,
    attachments: [],
    mentions: []
  };
}

export class ComposerDraftCache {
  private cache = new Map<string, ComposerDraft>();

  get(key: string): ComposerDraft {
    const existing = this.cache.get(key);
    if (existing) {
      return existing;
    }
    const fresh = createEmptyComposerDraft();
    this.cache.set(key, fresh);
    return fresh;
  }

  update(key: string, patch: Partial<ComposerDraft>): ComposerDraft {
    const current = this.get(key);
    const updated: ComposerDraft = {
      ...current,
      ...patch,
    };
    this.cache.set(key, updated);
    return updated;
  }

  takeSnapshot(key: string): ComposerDraftSnapshot {
    const draft = this.get(key);
    const stagedAttachmentIds = draft.attachments
      .map((att) => att.stagedAttachmentId)
      .filter((id): id is string => id !== null);
    return {
      text: draft.text,
      stagedAttachmentIds,
      mentions: draft.mentions ? [...draft.mentions] : []
    };
  }

  clearSnapshot(key: string, snapshot: ComposerDraftSnapshot): void {
    const current = this.get(key);
    const newText = current.text === snapshot.text ? '' : current.text;
    const newMentions = current.text === snapshot.text ? [] : current.mentions;
    const newAttachments = current.attachments.filter(
      (att) => !att.stagedAttachmentId || !snapshot.stagedAttachmentIds.includes(att.stagedAttachmentId)
    );
    this.cache.set(key, {
      ...current,
      text: newText,
      attachments: newAttachments,
      reply: null,
      mentions: newMentions
    });
  }

  clear(key: string): void {
    this.cache.delete(key);
  }
}

export function insertTextAtSelection(
  fullText: string,
  toInsert: string,
  start: number,
  end: number
): { value: string; caret: number } {
  const safeStart = Math.max(0, Math.min(start, fullText.length));
  const safeEnd = Math.max(safeStart, Math.min(end, fullText.length));

  const before = fullText.slice(0, safeStart);
  const after = fullText.slice(safeEnd);

  const value = before + toInsert + after;
  const caret = safeStart + toInsert.length;

  return { value, caret };
}
