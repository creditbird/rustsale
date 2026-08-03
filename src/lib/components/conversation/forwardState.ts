import type {
  ConversationForwardTargetResult,
  ForwardConversationMessageResult,
  InboxItem
} from '$lib/services/conversationService';

export interface ForwardDialogSnapshot {
  sourceMessageId: number;
  selectedTargetIds: number[];
  clientOperationId: string;
  status: 'idle' | 'submitting' | 'submitted' | 'failed';
  result: ForwardConversationMessageResult | null;
  error: string | null;
}

export function createForwardSnapshot(
  sourceMessageId: number,
  targetConversationIds: number[] = [],
  clientOperationId?: string
): ForwardDialogSnapshot {
  return {
    sourceMessageId,
    selectedTargetIds: targetConversationIds,
    clientOperationId: clientOperationId || crypto.randomUUID(),
    status: 'idle',
    result: null,
    error: null
  };
}

export function canRetryForwardTarget(
  targetStatus: ConversationForwardTargetResult | string
): boolean {
  const status = typeof targetStatus === 'string' ? targetStatus : targetStatus.status;
  return status === 'failed' || status === 'delivery_unknown';
}

function calculateOverallStatus(
  targets: ConversationForwardTargetResult[]
): ForwardConversationMessageResult['status'] {
  if (targets.length === 0) return 'queued';
  const statuses = targets.map((t) => t.status);
  if (statuses.every((s) => s === 'sent')) return 'sent';
  if (statuses.every((s) => s === 'failed')) return 'failed';
  if (statuses.some((s) => s === 'sending')) return 'sending';
  if (statuses.some((s) => s === 'queued')) return 'queued';
  return 'partial';
}

export function mergeForwardResult(
  snapshot: ForwardDialogSnapshot,
  result: ForwardConversationMessageResult | ConversationForwardTargetResult
): ForwardDialogSnapshot {
  if ('targets' in result) {
    return {
      ...snapshot,
      status: 'submitted',
      result,
      error: null
    };
  }

  const existingResult = snapshot.result;
  let updatedTargets: ConversationForwardTargetResult[];

  if (existingResult) {
    const targetIdx = existingResult.targets.findIndex(
      (t) => t.targetConversationId === result.targetConversationId
    );
    if (targetIdx >= 0) {
      updatedTargets = [...existingResult.targets];
      updatedTargets[targetIdx] = result;
    } else {
      updatedTargets = [...existingResult.targets, result];
    }
  } else {
    updatedTargets = [result];
  }

  const overallStatus = calculateOverallStatus(updatedTargets);

  return {
    ...snapshot,
    status: 'submitted',
    result: {
      clientOperationId: existingResult?.clientOperationId || snapshot.clientOperationId,
      sourceMessageId: existingResult?.sourceMessageId || snapshot.sourceMessageId,
      status: overallStatus,
      targets: updatedTargets
    },
    error: null
  };
}

export function filterForwardTargets(
  inbox: InboxItem[],
  sourceChannelAccountId: number,
  sourceConversationId: number | null,
  query: string
): InboxItem[] {
  const normalized = query.trim().toLowerCase();
  return inbox.filter((item) => {
    // Must match source channel account ID
    const matchesAccount = item.replyTargets.some(
      (rt) => rt.channelAccountId === sourceChannelAccountId
    );
    if (!matchesAccount) return false;

    // Exclude source conversation if specified
    if (sourceConversationId !== null && item.conversationId === sourceConversationId) {
      return false;
    }

    if (!normalized) return true;

    const titleMatch = item.title.toLowerCase().includes(normalized);
    const participantMatch = item.participants?.some((p) =>
      p.displayName?.toLowerCase().includes(normalized)
    );

    return titleMatch || Boolean(participantMatch);
  });
}
