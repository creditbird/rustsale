<script lang="ts">
  import { tick } from 'svelte';
  import {
    discardConversationStagedAttachment,
    listConversationQuickTemplates,
    markConversationQuickTemplateUsed,
    restartConversationAccount,
    sendConversationBatch,
    stageConversationAttachmentPaths,
    stageConversationClipboardImage,
    type ConversationAccountRuntime,
    type ConversationCapabilities,
    type ConversationMentionCandidate,
    type ConversationQuickTemplate,
    type ConversationRuntimeStatus,
    type InboxItem,
    type InboxReplyTarget,
    type SendConversationBatchResult,
    type SendPartRequest,
    type StagedConversationAttachment,
    type ZaloStickerReference
  } from '$lib/services/conversationService';
  import {
    onDragDropEvent,
    openFilePicker,
    readClipboardImage
  } from '$lib/services/conversationPlatform';
  import { Textarea } from '$lib/components/ui/textarea';
  import { Button } from '$lib/components/ui/button';
  import * as Select from '$lib/components/ui/select';
  import { Badge } from '$lib/components/ui/badge';
  import { AlertCircle, RefreshCw, Send, Settings2 } from 'lucide-svelte';
  import AttachmentDropOverlay from './AttachmentDropOverlay.svelte';
  import AttachmentPreviewQueue from './AttachmentPreviewQueue.svelte';
  import ComposerReplyBar from './ComposerReplyBar.svelte';
  import ComposerToolbar from './ComposerToolbar.svelte';
  import EmojiPicker from './EmojiPicker.svelte';
  import MentionCandidatePopover from './MentionCandidatePopover.svelte';
  import QuickTemplateManagerDialog from './QuickTemplateManagerDialog.svelte';
  import QuickTemplatePopover from './QuickTemplatePopover.svelte';
  import ZaloStickerPicker from './ZaloStickerPicker.svelte';
  import {
    ComposerDraftCache,
    composerDraftKey,
    insertTextAtSelection,
    type ComposerAttachmentItem,
    type ConversationReplySelection
  } from './composerDraftState';
  import {
    applyTextEditToMentions,
    findMentionToken,
    insertMention,
    toMentionInputs,
    type ComposerMentionRange,
    type MentionToken
  } from './mentionState';
  import {
    findSlashToken,
    rankQuickTemplates,
    replaceSlashToken,
    type SlashToken
  } from './quickTemplateState';
  import { safeImagePreviewDataUrl } from './messageMedia';

  const MAX_BATCH_PARTS = 10;
  const MAX_BATCH_BYTES = 100 * 1024 * 1024;
  const TEXT_ONLY_CAPABILITIES: ConversationCapabilities = {
    maxTextLength: 2000,
    maxAttachmentsPerBatch: 0,
    maxFileSizeBytes: 0,
    supportedImageTypes: [],
    supportedVideoTypes: [],
    supportedAudioTypes: [],
    supportedDocTypes: [],
    supportsStickers: false,
    supportsQuoteReply: false,
    supportsRecall: false,
    recallWindowSeconds: null,
    supportsReactions: false
  };

  let fallbackId = 0;

  let {
    item,
    runtime,
    disabled = false,
    replyTarget = null,
    onCancelReply = () => {},
    onTargetResolved,
    onBatchQueued,
    onOpenSettings = () => {},
    onRuntimeRefresh = () => {}
  }: {
    item: InboxItem;
    runtime?: ConversationRuntimeStatus;
    disabled?: boolean;
    replyTarget?: ConversationReplySelection | null;
    onCancelReply?: () => void;
    onTargetResolved?: (target: InboxReplyTarget) => void;
    onBatchQueued?: (result: SendConversationBatchResult) => void;
    onOpenSettings?: () => void;
    onRuntimeRefresh?: () => void | Promise<void>;
  } = $props();

  const draftCache = new ComposerDraftCache();
  let text = $state('');
  let attachments = $state<ComposerAttachmentItem[]>([]);
  let activeDraftKey = $state<string | null>(null);
  let activePopover = $state<'emoji' | 'sticker' | null>(null);
  let isSending = $state(false);
  let sendError = $state<string | null>(null);
  let reconnecting = $state(false);
  let recoveryError = $state<string | null>(null);
  let isDragActive = $state(false);
  let textareaElement = $state<HTMLTextAreaElement | null>(null);

  let quickTemplates = $state<ConversationQuickTemplate[]>([]);
  let showQuickTemplatePopover = $state(false);
  let quickTemplateSlashToken = $state<SlashToken | null>(null);
  let quickTemplateSelectedIndex = $state(0);
  let showQuickTemplateManager = $state(false);

  let mentions = $state<ComposerMentionRange[]>([]);
  let showMentionPopover = $state(false);
  let mentionToken = $state<MentionToken | null>(null);
  let mentionSelectedIndex = $state(0);
  let mentionCandidates = $state<ConversationMentionCandidate[]>([]);

  $effect(() => {
    if (selectedCapabilities.quickTemplates?.enabled) {
      void loadQuickTemplates();
    }
  });

  async function loadQuickTemplates(): Promise<void> {
    try {
      quickTemplates = await listConversationQuickTemplates({ includeInactive: false });
    } catch {
      quickTemplates = [];
    }
  }

  let selectedChannelAccountId = $state<number | null>(null);
  let selectedAccountIdString = $state('');
  let selectionTouched = $state(false);
  let currentItemKey = $state('');
  let lastResolvedTargetKey = $state('');

  function newClientId(prefix: string): string {
    const randomUUID = globalThis.crypto?.randomUUID;
    if (randomUUID) return `${prefix}-${randomUUID.call(globalThis.crypto)}`;
    fallbackId += 1;
    return `${prefix}-${Date.now()}-${fallbackId}`;
  }

  function getAccountRuntime(channelAccountId: number): ConversationAccountRuntime | null {
    return runtime?.accounts.find((account) => account.channelAccountId === channelAccountId) ?? null;
  }

  type ComposerOption = {
    target: InboxReplyTarget;
    account: ConversationAccountRuntime | null;
    id: number;
    label: string;
    status: string;
    lastError: string | null;
    isEligible: boolean;
  };

  let options = $derived.by<ComposerOption[]>(() =>
    item.replyTargets.map((target) => {
      const account = getAccountRuntime(target.channelAccountId);
      const status = account?.status ?? 'connected';
      const accountName = target.accountDisplayName || account?.displayName;
      return {
        target,
        account,
        id: target.channelAccountId,
        label: `${target.channel} #${target.channelAccountId}${accountName ? ` (${accountName})` : ''}`,
        status,
        lastError: account?.lastError ?? null,
        isEligible: status === 'connected'
      };
    })
  );

  let isGroupMode = $derived(
    item.kind === 'group' || item.replyTargets.some((target) => target.conversationKind === 'group')
  );

  let pinnedOption = $derived(
    replyTarget
      ? options.find(
          (option) =>
            option.target.conversationId === replyTarget.conversationId &&
            option.id === replyTarget.channelAccountId
        ) ?? null
      : null
  );

  let selectedOption = $derived(
    pinnedOption ?? options.find((option) => option.id === selectedChannelAccountId) ?? options[0] ?? null
  );

  let targetConversationId = $derived(
    replyTarget?.conversationId ?? selectedOption?.target.conversationId ?? item.conversationId ?? null
  );
  let selectedCapabilities = $derived(
    selectedOption?.account?.capabilities ?? TEXT_ONLY_CAPABILITIES
  );
  let maxMentions = $derived(
    selectedCapabilities.mentions?.maxMentionsPerMessage ??
      selectedCapabilities.mentions?.maxPerMessage ??
      20
  );
  let isTargetLocked = $derived(Boolean(replyTarget) || isGroupMode);
  let nextDraftKey = $derived(
    selectedOption && targetConversationId !== null
      ? composerDraftKey(targetConversationId, selectedOption.id)
      : null
  );

  $effect(() => {
    if (item.key !== currentItemKey) {
      currentItemKey = item.key;
      selectionTouched = false;
      selectedChannelAccountId = null;
      activePopover = null;
      sendError = null;
    }
  });

  $effect(() => {
    if (replyTarget) {
      selectedChannelAccountId = replyTarget.channelAccountId;
      return;
    }
    if (selectionTouched) return;
    if (options.length === 0) {
      selectedChannelAccountId = null;
      return;
    }

    if (isGroupMode) {
      selectedChannelAccountId =
        options.find((option) => option.target.conversationKind === 'group')?.id ?? options[0].id;
      return;
    }

    const candidates = options.some((option) => option.isEligible)
      ? options.filter((option) => option.isEligible)
      : options;
    selectedChannelAccountId = [...candidates].sort((left, right) => {
      const leftTime = left.target.lastInboundAt ? new Date(left.target.lastInboundAt).getTime() : 0;
      const rightTime = right.target.lastInboundAt ? new Date(right.target.lastInboundAt).getTime() : 0;
      return rightTime - leftTime;
    })[0].id;
  });

  $effect(() => {
    if (selectedAccountIdString) {
      const id = Number(selectedAccountIdString);
      if (Number.isFinite(id) && id !== selectedChannelAccountId) {
        selectedChannelAccountId = id;
        selectionTouched = true;
      }
    }
  });

  $effect(() => {
    const nextValue = selectedChannelAccountId === null ? '' : String(selectedChannelAccountId);
    if (selectedAccountIdString !== nextValue) selectedAccountIdString = nextValue;
  });

  $effect(() => {
    if (nextDraftKey === activeDraftKey) return;
    if (activeDraftKey) {
      draftCache.update(activeDraftKey, { text, attachments: [...attachments], mentions: [...mentions] });
    }
    activeDraftKey = nextDraftKey;
    if (!activeDraftKey) {
      text = '';
      attachments = [];
      mentions = [];
      return;
    }
    const draft = draftCache.get(activeDraftKey);
    text = draft.text;
    attachments = [...draft.attachments];
    mentions = [...draft.mentions];
  });

  $effect(() => {
    if (!selectedOption) return;
    const resolvedKey = `${selectedOption.target.conversationId}:${selectedOption.id}`;
    if (resolvedKey !== lastResolvedTargetKey) {
      lastResolvedTargetKey = resolvedKey;
      onTargetResolved?.(selectedOption.target);
    }
  });

  $effect(() =>
    onDragDropEvent(
      (paths) => {
        isDragActive = false;
        void stagePaths(paths);
      },
      (active) => {
        isDragActive = active && !disabled && selectedCapabilities.maxAttachmentsPerBatch > 0;
      }
    )
  );

  function persistDraft(): void {
    if (!activeDraftKey) return;
    draftCache.update(activeDraftKey, { text, attachments: [...attachments], mentions: [...mentions] });
  }

  function updateAttachments(nextItems: ComposerAttachmentItem[]): void {
    attachments = nextItems;
    persistDraft();
  }

  function formatError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() || 'Tệp đính kèm';
  }

  function supportsAttachment(staged: StagedConversationAttachment): boolean {
    const supportedTypes =
      staged.mediaType === 'image'
        ? selectedCapabilities.supportedImageTypes
        : staged.mediaType === 'video'
          ? selectedCapabilities.supportedVideoTypes
          : staged.mediaType === 'audio'
            ? selectedCapabilities.supportedAudioTypes
            : selectedCapabilities.supportedDocTypes;
    return supportedTypes.includes(staged.mimeType);
  }

  function validateStagedAttachment(staged: StagedConversationAttachment): string | null {
    if (staged.fileSizeBytes > selectedCapabilities.maxFileSizeBytes) {
      return `Tệp vượt quá giới hạn ${selectedCapabilities.maxFileSizeBytes} byte`;
    }
    if (!supportsAttachment(staged)) {
      return `Định dạng ${staged.mimeType} không được hỗ trợ`;
    }
    return null;
  }

  async function applyStagedResults(
    placeholders: ComposerAttachmentItem[],
    stagedItems: StagedConversationAttachment[],
    previewDataUrl: string | null = null
  ): Promise<void> {
    const currentIds = new Set(attachments.map((item) => item.localId));
    const replacements = new Map<string, ComposerAttachmentItem>();

    for (let index = 0; index < placeholders.length; index += 1) {
      const placeholder = placeholders[index];
      const staged = stagedItems[index];
      if (!staged) {
        replacements.set(placeholder.localId, {
          ...placeholder,
          status: 'error',
          errorMessage: 'Không nhận được metadata staging'
        });
        continue;
      }

      if (!currentIds.has(placeholder.localId)) {
        await discardConversationStagedAttachment(staged.stagedAttachmentId).catch(() => {});
        continue;
      }

      const validationError = validateStagedAttachment(staged);
      if (validationError) {
        await discardConversationStagedAttachment(staged.stagedAttachmentId).catch(() => {});
        replacements.set(placeholder.localId, {
          ...placeholder,
          fileName: staged.fileName,
          mimeType: staged.mimeType,
          sizeBytes: staged.fileSizeBytes,
          attachmentType: staged.mediaType,
          previewDataUrl: null,
          status: 'error',
          errorMessage: validationError
        });
        continue;
      }

      replacements.set(placeholder.localId, {
        localId: placeholder.localId,
        stagedAttachmentId: staged.stagedAttachmentId,
        fileName: staged.fileName,
        mimeType: staged.mimeType,
        sizeBytes: staged.fileSizeBytes,
        attachmentType: staged.mediaType,
        previewDataUrl: staged.mediaType === 'image' ? previewDataUrl : null,
        status: 'ready',
        errorMessage: null
      });
    }

    updateAttachments(
      attachments.map((item) => replacements.get(item.localId) ?? item)
    );
  }

  async function stagePaths(paths: string[]): Promise<void> {
    if (!selectedOption || targetConversationId === null || paths.length === 0) return;
    sendError = null;
    const remaining = selectedCapabilities.maxAttachmentsPerBatch - attachments.length;
    if (remaining <= 0 || paths.length > remaining) {
      sendError = `Chỉ được đính kèm tối đa ${selectedCapabilities.maxAttachmentsPerBatch} tệp`;
      return;
    }

    const placeholders = paths.map<ComposerAttachmentItem>((path) => ({
      localId: newClientId('local-attachment'),
      stagedAttachmentId: null,
      fileName: fileNameFromPath(path),
      mimeType: 'application/octet-stream',
      sizeBytes: 0,
      attachmentType: 'file',
      previewDataUrl: null,
      status: 'staging',
      errorMessage: null
    }));
    updateAttachments([...attachments, ...placeholders]);

    try {
      const stagedItems = await stageConversationAttachmentPaths(targetConversationId, paths);
      await applyStagedResults(placeholders, stagedItems);
    } catch (error) {
      const message = formatError(error);
      const placeholderIds = new Set(placeholders.map((item) => item.localId));
      updateAttachments(
        attachments.map((item) =>
          placeholderIds.has(item.localId)
            ? { ...item, status: 'error', errorMessage: message }
            : item
        )
      );
      sendError = message;
    }
  }

  async function handlePickAttachment(): Promise<void> {
    try {
      const paths = await openFilePicker({ multiple: true });
      await stagePaths(paths);
    } catch (error) {
      sendError = formatError(error);
    }
  }

  async function handlePaste(): Promise<void> {
    if (!selectedOption || targetConversationId === null) return;
    try {
      const clipboardImage = await readClipboardImage();
      if (!clipboardImage) return;
      if (attachments.length >= selectedCapabilities.maxAttachmentsPerBatch) {
        sendError = `Chỉ được đính kèm tối đa ${selectedCapabilities.maxAttachmentsPerBatch} tệp`;
        return;
      }
      if (!selectedCapabilities.supportedImageTypes.includes(clipboardImage.mimeType)) {
        sendError = `Định dạng ${clipboardImage.mimeType} không được hỗ trợ`;
        return;
      }

      const previewDataUrl = safeImagePreviewDataUrl(
        `data:${clipboardImage.mimeType};base64,${clipboardImage.base64Data}`
      );
      const placeholder: ComposerAttachmentItem = {
        localId: newClientId('clipboard-attachment'),
        stagedAttachmentId: null,
        fileName: clipboardImage.fileName,
        mimeType: clipboardImage.mimeType,
        sizeBytes: 0,
        attachmentType: 'image',
        previewDataUrl,
        status: 'staging',
        errorMessage: null
      };
      updateAttachments([...attachments, placeholder]);
      const staged = await stageConversationClipboardImage({
        conversationId: targetConversationId,
        ...clipboardImage
      });
      await applyStagedResults([placeholder], [staged], previewDataUrl);
    } catch (error) {
      const message = formatError(error);
      updateAttachments(
        attachments.map((item) =>
          item.status === 'staging' && item.attachmentType === 'image'
            ? { ...item, status: 'error', errorMessage: message }
            : item
        )
      );
      sendError = message;
    }
  }

  async function handleRemoveAttachment(item: ComposerAttachmentItem): Promise<void> {
    if (!item.stagedAttachmentId) {
      updateAttachments(attachments.filter((candidate) => candidate.localId !== item.localId));
      return;
    }

    updateAttachments(
      attachments.map((candidate) =>
        candidate.localId === item.localId ? { ...candidate, status: 'removing' } : candidate
      )
    );
    try {
      await discardConversationStagedAttachment(item.stagedAttachmentId);
      updateAttachments(attachments.filter((candidate) => candidate.localId !== item.localId));
    } catch (error) {
      updateAttachments(
        attachments.map((candidate) =>
          candidate.localId === item.localId
            ? { ...candidate, status: 'error', errorMessage: formatError(error) }
            : candidate
        )
      );
    }
  }

  function checkQuickTemplates(): void {
    if (!selectedCapabilities.quickTemplates?.enabled || !textareaElement) {
      showQuickTemplatePopover = false;
      quickTemplateSlashToken = null;
      return;
    }

    const caret = textareaElement.selectionStart || text.length;
    const token = findSlashToken(text, caret);
    if (token) {
      quickTemplateSlashToken = token;
      const ranked = rankQuickTemplates(quickTemplates, token.query);
      if (ranked.length > 0) {
        showQuickTemplatePopover = true;
        quickTemplateSelectedIndex = 0;
      } else {
        showQuickTemplatePopover = false;
      }
    } else {
      showQuickTemplatePopover = false;
      quickTemplateSlashToken = null;
    }
  }

  function applyQuickTemplate(template: ConversationQuickTemplate): void {
    if (!quickTemplateSlashToken) return;

    const result = replaceSlashToken(text, quickTemplateSlashToken, template.bodyText);
    text = result.value;
    persistDraft();
    showQuickTemplatePopover = false;
    quickTemplateSlashToken = null;

    void markConversationQuickTemplateUsed(template.id);

    void tick().then(() => {
      if (textareaElement) {
        textareaElement.focus();
        textareaElement.setSelectionRange(result.caret, result.caret);
      }
    });
  }

  function checkMentions(): void {
    if (!selectedCapabilities.mentions?.enabled || !isGroupMode || !textareaElement) {
      showMentionPopover = false;
      mentionToken = null;
      return;
    }

    const caret = textareaElement.selectionStart || text.length;
    const token = findMentionToken(text, caret);
    if (token) {
      mentionToken = token;
      showMentionPopover = true;
      mentionSelectedIndex = 0;
    } else {
      showMentionPopover = false;
      mentionToken = null;
    }
  }

  function applyMention(candidate: ConversationMentionCandidate): void {
    if (!mentionToken) return;

    const result = insertMention(text, mentionToken, candidate, mentions);
    text = result.value;
    mentions = result.mentions;
    persistDraft();
    showMentionPopover = false;
    mentionToken = null;

    void tick().then(() => {
      if (textareaElement) {
        textareaElement.focus();
        textareaElement.setSelectionRange(result.caret, result.caret);
      }
    });
  }

  function handleTextInput(event: Event): void {
    const el = event.currentTarget as HTMLTextAreaElement;
    const nextText = el.value;
    const caret = el.selectionStart || nextText.length;

    const prevText = text;
    const lenDiff = nextText.length - prevText.length;
    const edit = {
      start: lenDiff < 0 ? caret : Math.max(0, caret - lenDiff),
      deleteCount: lenDiff < 0 ? Math.abs(lenDiff) : 0,
      insertedText: lenDiff > 0 ? nextText.slice(Math.max(0, caret - lenDiff), caret) : ''
    };

    mentions = applyTextEditToMentions(prevText, nextText, edit, mentions);
    text = nextText;
    persistDraft();
    sendError = null;
    checkQuickTemplates();
    checkMentions();
  }

  async function handleEmojiSelect(emoji: string): Promise<void> {
    const start = textareaElement?.selectionStart ?? text.length;
    const end = textareaElement?.selectionEnd ?? start;
    const inserted = insertTextAtSelection(text, emoji, start, end);
    text = inserted.value;
    persistDraft();
    await tick();
    textareaElement?.focus();
    textareaElement?.setSelectionRange(inserted.caret, inserted.caret);
  }

  let statusReason = $derived.by(() => {
    if (!selectedOption) return 'Chưa chọn tài khoản gửi';
    if (selectedOption.status === 'connected') return null;
    const reasons: Record<string, string> = {
      disconnected: 'Tài khoản chưa kết nối',
      connecting: 'Tài khoản đang kết nối...',
      degraded: 'Tài khoản bị gián đoạn/suy giảm kết nối',
      invalid: 'Tài khoản không hợp lệ hoặc hết hạn xác thực'
    };
    const baseReason = reasons[selectedOption.status] ?? `Trạng thái tài khoản: ${selectedOption.status}`;
    return selectedOption.lastError ? `${baseReason} (${selectedOption.lastError})` : baseReason;
  });

  let readyAttachments = $derived(
    attachments.filter(
      (attachment): attachment is ComposerAttachmentItem & { stagedAttachmentId: string } =>
        attachment.status === 'ready' && attachment.stagedAttachmentId !== null
    )
  );
  let hasBlockingAttachment = $derived(
    attachments.some((attachment) => attachment.status !== 'ready')
  );
  let totalBatchBytes = $derived(
    readyAttachments.reduce((sum, attachment) => sum + attachment.sizeBytes, 0)
  );
  let partCount = $derived(readyAttachments.length + (text.trim() ? 1 : 0));
  let validationReason = $derived.by(() => {
    if (replyTarget && !selectedCapabilities.supportsQuoteReply) {
      return 'Tài khoản này không hỗ trợ trả lời trích dẫn';
    }
    if (text.length > selectedCapabilities.maxTextLength) {
      return `Tin nhắn vượt quá ${selectedCapabilities.maxTextLength} ký tự`;
    }
    if (readyAttachments.length > selectedCapabilities.maxAttachmentsPerBatch) {
      return `Chỉ được đính kèm tối đa ${selectedCapabilities.maxAttachmentsPerBatch} tệp`;
    }
    if (partCount > MAX_BATCH_PARTS) {
      return `Một lần gửi chỉ hỗ trợ tối đa ${MAX_BATCH_PARTS} phần`;
    }
    if (totalBatchBytes > MAX_BATCH_BYTES) {
      return 'Tổng dung lượng đính kèm vượt quá 100 MiB';
    }
    return null;
  });
  let canSend = $derived(
    !disabled &&
      !isSending &&
      selectedOption !== null &&
      selectedOption.isEligible &&
      targetConversationId !== null &&
      partCount > 0 &&
      !hasBlockingAttachment &&
      validationReason === null
  );

  function buildParts(
    snapshotText: string,
    stagedAttachmentIds: string[],
    snapshotMentions: ComposerMentionRange[]
  ): SendPartRequest[] {
    const parts: SendPartRequest[] = [];
    const trimmed = snapshotText.trim();
    if (trimmed) {
      const mentionInputs = toMentionInputs(snapshotMentions);
      parts.push({
        clientPartId: newClientId('part'),
        type: 'text',
        text: trimmed,
        ...(mentionInputs.length > 0 ? { mentions: mentionInputs } : {})
      });
    }
    for (const stagedAttachmentId of stagedAttachmentIds) {
      parts.push({
        clientPartId: newClientId('part'),
        type: 'attachment',
        stagedAttachmentId
      });
    }
    return parts;
  }

  async function handleSend(): Promise<void> {
    if (!canSend || !activeDraftKey || targetConversationId === null) return;
    persistDraft();
    const snapshot = draftCache.takeSnapshot(activeDraftKey);
    const parts = buildParts(snapshot.text, snapshot.stagedAttachmentIds, snapshot.mentions);
    if (parts.length === 0) return;

    isSending = true;
    sendError = null;
    try {
      const result = await sendConversationBatch({
        conversationId: targetConversationId,
        clientBatchId: newClientId('batch'),
        replyToMessageId: replyTarget?.messageId ?? null,
        parts
      });
      draftCache.clearSnapshot(activeDraftKey, snapshot);
      const nextDraft = draftCache.get(activeDraftKey);
      text = nextDraft.text;
      attachments = [...nextDraft.attachments];
      mentions = [...nextDraft.mentions];
      onBatchQueued?.(result);
      if (replyTarget) onCancelReply();
    } catch (error) {
      sendError = formatError(error);
    } finally {
      isSending = false;
    }
  }

  async function handleStickerSelect(sticker: ZaloStickerReference): Promise<void> {
    if (
      disabled ||
      isSending ||
      !selectedOption?.isEligible ||
      targetConversationId === null ||
      !selectedCapabilities.supportsStickers
    ) {
      return;
    }

    isSending = true;
    sendError = null;
    try {
      const result = await sendConversationBatch({
        conversationId: targetConversationId,
        clientBatchId: newClientId('batch'),
        replyToMessageId: replyTarget?.messageId ?? null,
        parts: [
          {
            clientPartId: newClientId('part'),
            type: 'sticker',
            stickerId: sticker.stickerId,
            cateId: sticker.cateId,
            stickerType: sticker.stickerType
          }
        ]
      });
      activePopover = null;
      onBatchQueued?.(result);
      if (replyTarget) onCancelReply();
    } catch (error) {
      sendError = formatError(error);
    } finally {
      isSending = false;
    }
  }

  async function handleReconnect(): Promise<void> {
    if (!selectedOption?.account?.providerAccountKey || reconnecting) return;
    reconnecting = true;
    recoveryError = null;
    try {
      await restartConversationAccount(selectedOption.account.providerAccountKey);
      await onRuntimeRefresh();
    } catch (error) {
      recoveryError = formatError(error);
    } finally {
      reconnecting = false;
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (showMentionPopover && mentionToken) {
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        if (mentionCandidates.length > 0) {
          mentionSelectedIndex = (mentionSelectedIndex + 1) % mentionCandidates.length;
        }
        return;
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        if (mentionCandidates.length > 0) {
          mentionSelectedIndex =
            (mentionSelectedIndex - 1 + mentionCandidates.length) % mentionCandidates.length;
        }
        return;
      } else if (event.key === 'Enter' || event.key === 'Tab') {
        event.preventDefault();
        if (mentionCandidates[mentionSelectedIndex]) {
          applyMention(mentionCandidates[mentionSelectedIndex]);
        }
        return;
      } else if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        showMentionPopover = false;
        return;
      }
    }

    if (showQuickTemplatePopover && quickTemplateSlashToken) {
      const ranked = rankQuickTemplates(quickTemplates, quickTemplateSlashToken.query);
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        if (ranked.length > 0) {
          quickTemplateSelectedIndex = (quickTemplateSelectedIndex + 1) % ranked.length;
        }
        return;
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        if (ranked.length > 0) {
          quickTemplateSelectedIndex =
            (quickTemplateSelectedIndex - 1 + ranked.length) % ranked.length;
        }
        return;
      } else if (event.key === 'Enter' || event.key === 'Tab') {
        event.preventDefault();
        if (ranked[quickTemplateSelectedIndex]) {
          applyQuickTemplate(ranked[quickTemplateSelectedIndex]);
        }
        return;
      } else if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        showQuickTemplatePopover = false;
        return;
      }
    }

    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      void handleSend();
    }
  }
</script>

<div class="relative flex flex-col gap-2 border-t p-3 bg-background">
  <AttachmentDropOverlay active={isDragActive} />

  <div class="flex items-center justify-between gap-2">
    {#if isTargetLocked && selectedOption}
      <div class="flex min-w-0 items-center gap-2 text-sm font-medium text-muted-foreground" data-testid="locked-target">
        <Badge variant="outline">Đã khóa kênh gửi</Badge>
        <span class="truncate">{selectedOption.label}</span>
      </div>
    {:else if options.length > 0}
      <Select.Root type="single" bind:value={selectedAccountIdString} disabled={disabled || isSending}>
        <Select.Trigger class="w-[260px]" data-testid="channel-account-select">
          <span>{selectedOption?.label ?? 'Chọn tài khoản gửi...'}</span>
        </Select.Trigger>
        <Select.Content>
          <Select.Group>
            {#each options as option (option.id)}
              <Select.Item
                value={String(option.id)}
                label={option.label}
                disabled={!option.isEligible}
                data-testid="account-option-{option.id}"
              >
                <div class="flex w-full items-center justify-between gap-2">
                  <span>{option.label}</span>
                  {#if option.status !== 'connected'}
                    <span class="text-xs text-destructive">({option.status})</span>
                  {/if}
                </div>
              </Select.Item>
            {/each}
          </Select.Group>
        </Select.Content>
      </Select.Root>
    {:else}
      <span></span>
    {/if}

    <div class="relative ml-auto shrink-0">
      <ComposerToolbar
        capabilities={selectedCapabilities}
        disabled={disabled || isSending || !selectedOption?.isEligible}
        onToggleEmoji={() => (activePopover = activePopover === 'emoji' ? null : 'emoji')}
        onToggleSticker={() => (activePopover = activePopover === 'sticker' ? null : 'sticker')}
        onPickAttachment={handlePickAttachment}
        onToggleQuickTemplates={() => (showQuickTemplateManager = true)}
      />
      {#if activePopover === 'emoji'}
        <div class="absolute right-0 top-full z-50 mt-2">
          <EmojiPicker onSelect={handleEmojiSelect} />
        </div>
      {:else if activePopover === 'sticker' && targetConversationId !== null}
        <div class="absolute right-0 top-full z-50 mt-2">
          <ZaloStickerPicker
            conversationId={targetConversationId}
            disabled={isSending}
            onSelect={handleStickerSelect}
          />
        </div>
      {/if}
    </div>
  </div>

  {#if statusReason}
    <div class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
      <div class="flex min-w-0 items-center gap-1.5">
        <AlertCircle class="size-3.5 shrink-0" />
        <span>{statusReason}</span>
      </div>
      <div class="flex shrink-0 items-center gap-1.5">
        {#if selectedOption?.account?.providerAccountKey}
          <Button
            type="button"
            variant="outline"
            size="sm"
            class="h-7 gap-1 text-xs"
            aria-label={`Reconnect ${selectedOption.account.displayName || selectedOption.label}`}
            onclick={handleReconnect}
            disabled={reconnecting}
          >
            <RefreshCw class="size-3 {reconnecting ? 'animate-spin' : ''}" />
            Reconnect
          </Button>
        {/if}
        <Button
          type="button"
          variant="outline"
          size="sm"
          class="h-7 gap-1 text-xs"
          aria-label="Open account settings"
          onclick={onOpenSettings}
        >
          <Settings2 class="size-3" /> Settings
        </Button>
      </div>
    </div>
  {/if}

  {#if recoveryError}
    <div class="flex items-center gap-1.5 rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
      <AlertCircle class="size-3.5 shrink-0" />
      <span>{recoveryError}</span>
    </div>
  {/if}

  {#if sendError || validationReason}
    <div class="flex items-center gap-1.5 rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
      <AlertCircle class="size-3.5 shrink-0" />
      <span>{sendError || validationReason}</span>
    </div>
  {/if}

  <ComposerReplyBar reply={replyTarget} onCancel={onCancelReply} />
  <AttachmentPreviewQueue items={attachments} onRemove={handleRemoveAttachment} />

  {#if showQuickTemplatePopover}
    <div class="relative z-50 mb-1" data-testid="quick-template-popover-wrapper">
      <QuickTemplatePopover
        templates={quickTemplates}
        query={quickTemplateSlashToken?.query ?? ''}
        selectedIndex={quickTemplateSelectedIndex}
        onSelect={applyQuickTemplate}
        onOpenManager={() => {
          showQuickTemplatePopover = false;
          showQuickTemplateManager = true;
        }}
      />
    </div>
  {/if}

  {#if showMentionPopover && mentionToken && targetConversationId !== null && selectedCapabilities.mentions?.enabled}
    <div class="relative z-50 mb-1" data-testid="mention-candidate-popover-wrapper">
      <MentionCandidatePopover
        conversationId={targetConversationId}
        query={mentionToken.query}
        selectedIndex={mentionSelectedIndex}
        maxMentions={maxMentions}
        currentMentionCount={mentions.length}
        onCandidatesLoaded={(list) => {
          mentionCandidates = list;
        }}
        onSelect={applyMention}
      />
    </div>
  {/if}

  <div class="flex items-end gap-2">
    <Textarea
      bind:ref={textareaElement}
      bind:value={text}
      oninput={handleTextInput}
      onkeydown={handleKeydown}
      onpaste={() => void handlePaste()}
      maxlength={selectedCapabilities.maxTextLength > 0 ? selectedCapabilities.maxTextLength + 1 : undefined}
      placeholder="Nhập tin nhắn..."
      disabled={disabled || isSending || (selectedOption !== null && !selectedOption.isEligible)}
      class="min-h-[70px] resize-none"
    />
    <Button
      type="button"
      onclick={handleSend}
      disabled={!canSend}
      class="h-[70px] px-4 shrink-0 flex items-center justify-center gap-1.5"
    >
      <Send class="size-4" />
      <span>Gửi</span>
    </Button>
  </div>
</div>

{#if showQuickTemplateManager}
  <QuickTemplateManagerDialog
    bind:open={showQuickTemplateManager}
    onTemplatesChanged={loadQuickTemplates}
  />
{/if}
