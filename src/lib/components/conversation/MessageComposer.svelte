<script lang="ts">
  import { tick } from 'svelte';
  import {
    sendConversationMessage,
    type InboxItem,
    type ConversationRuntimeStatus,
    type ConversationMessage,
    type ConversationReplyTarget,
    type ConversationAccountRuntime
  } from '$lib/services/conversationService';
  import { Textarea } from '$lib/components/ui/textarea';
  import { Button } from '$lib/components/ui/button';
  import * as Select from '$lib/components/ui/select';
  import { Badge } from '$lib/components/ui/badge';
  import { Send, AlertCircle } from 'lucide-svelte';

  let {
    item,
    runtime,
    disabled = false,
    onQueued
  }: {
    item: InboxItem;
    runtime?: ConversationRuntimeStatus;
    disabled?: boolean;
    onQueued?: (message: ConversationMessage) => void;
  } = $props();

  let text = $state('');
  let isSending = $state(false);
  let sendError = $state<string | null>(null);

  let selectedChannelAccountId = $state<number | null>(null);
  let selectedAccountIdString = $state<string>('');
  let selectionTouched = $state(false);
  let currentItemKey = $state('');

  $effect(() => {
    if (item.key !== currentItemKey) {
      currentItemKey = item.key;
      selectionTouched = false;
      selectedChannelAccountId = null;
    }
  });

  $effect(() => {
    if (selectedAccountIdString) {
      const id = Number(selectedAccountIdString);
      if (!isNaN(id) && id !== selectedChannelAccountId) {
        selectedChannelAccountId = id;
        selectionTouched = true;
      }
    }
  });

  $effect(() => {
    if (selectedChannelAccountId !== null && selectedChannelAccountId !== undefined) {
      const str = String(selectedChannelAccountId);
      if (selectedAccountIdString !== str) {
        selectedAccountIdString = str;
      }
    } else if (selectedAccountIdString !== '') {
      selectedAccountIdString = '';
    }
  });

  function getAccountRuntime(channelAccountId: number): ConversationAccountRuntime | null {
    if (!runtime || !runtime.accounts) return null;
    return runtime.accounts.find((a) => a.channelAccountId === channelAccountId) || null;
  }

  type ComposerOption = {
    target: ConversationReplyTarget;
    account: ConversationAccountRuntime | null;
    id: number;
    label: string;
    status: string;
    lastError: string | null;
    isEligible: boolean;
  };

  let options = $derived.by<ComposerOption[]>(() => {
    if (!item.replyTargets || item.replyTargets.length === 0) return [];
    return item.replyTargets.map((target) => {
      const account = getAccountRuntime(target.channelAccountId);
      const status = account ? account.status : 'connected';
      const label = `${target.channel} #${target.channelAccountId}${target.accountDisplayName || account?.displayName ? ` (${target.accountDisplayName || account?.displayName})` : ''}`;
      const isEligible = status === 'connected';

      return {
        target,
        account,
        id: target.channelAccountId,
        label,
        status,
        lastError: account?.lastError || null,
        isEligible
      };
    });
  });

  let isGroupMode = $derived(
    item.kind === 'group' ||
      Boolean(item.replyTargets && item.replyTargets.some((t) => t.conversationKind === 'group'))
  );

  $effect(() => {
    if (selectionTouched) return;
    if (options.length === 0) {
      selectedChannelAccountId = null;
      return;
    }

    if (isGroupMode) {
      const zaloTarget = options.find((o) => o.target.channel === 'zalo') || options[0];
      selectedChannelAccountId = zaloTarget.id;
      return;
    }

    const connectedOptions = options.filter((o) => o.isEligible);
    const candidateList = connectedOptions.length > 0 ? connectedOptions : options;

    const sorted = [...candidateList].sort((a, b) => {
      const timeA = a.target.lastInboundAt ? new Date(a.target.lastInboundAt).getTime() : 0;
      const timeB = b.target.lastInboundAt ? new Date(b.target.lastInboundAt).getTime() : 0;
      return timeB - timeA;
    });

    selectedChannelAccountId = sorted[0].id;
  });

  let selectedOption = $derived(
    options.find((o) => o.id === selectedChannelAccountId) || options[0] || null
  );

  let statusReason = $derived.by(() => {
    if (!selectedOption) return 'Chưa chọn tài khoản gửi';
    const st = selectedOption.status;
    if (st === 'connected') return null;

    const reasonMap: Record<string, string> = {
      disconnected: 'Tài khoản chưa kết nối',
      connecting: 'Tài khoản đang kết nối...',
      degraded: 'Tài khoản bị gián đoạn/suy giảm kết nối (Cần khởi động lại)',
      invalid: 'Tài khoản không hợp lệ hoặc hết hạn xác thực'
    };

    const baseReason = reasonMap[st] || `Trạng thái tài khoản: ${st}`;
    if (selectedOption.lastError) {
      return `${baseReason} (${selectedOption.lastError})`;
    }
    return baseReason;
  });

  let targetConversationId = $derived(
    selectedOption?.target.conversationId ?? item.conversationId ?? null
  );

  let canSend = $derived(
    !disabled &&
      !isSending &&
      Boolean(text.trim()) &&
      selectedOption !== null &&
      selectedOption.isEligible &&
      targetConversationId !== null
  );

  async function handleSend() {
    const trimmed = text.trim();
    if (!trimmed || !canSend || !selectedOption || targetConversationId === null) return;

    isSending = true;
    sendError = null;

    try {
      const msg = await sendConversationMessage({
        conversationId: targetConversationId,
        channelAccountId: selectedOption.id,
        text: trimmed,
        replyToMessageId: null
      });
      text = '';
      sendError = null;
      onQueued?.(msg);
    } catch (err) {
      sendError = err instanceof Error ? err.message : String(err);
    } finally {
      isSending = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }
</script>

<div class="flex flex-col gap-2 border-t p-3 bg-background">
  <div class="flex items-center justify-between gap-2">
    {#if isGroupMode}
      <div class="flex items-center gap-2 text-sm font-medium text-muted-foreground" data-testid="locked-zalo-target">
        <Badge variant="outline">Zalo (Group Locked)</Badge>
        <span>{selectedOption?.label ?? 'Zalo Account'}</span>
      </div>
    {:else if options.length > 0}
      <div class="flex items-center gap-2">
        <Select.Root
          type="single"
          bind:value={selectedAccountIdString}
          disabled={disabled || isSending}
        >
          <Select.Trigger class="w-[260px]" data-testid="channel-account-select">
            <span>{selectedOption?.label ?? 'Chọn tài khoản gửi...'}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              {#each options as opt (opt.id)}
                <Select.Item
                  value={String(opt.id)}
                  label={opt.label}
                  disabled={!opt.isEligible}
                  data-testid="account-option-{opt.id}"
                >
                  <div class="flex items-center justify-between w-full gap-2">
                    <span>{opt.label}</span>
                    {#if opt.status !== 'connected'}
                      <span class="text-xs text-destructive">({opt.status})</span>
                    {/if}
                  </div>
                </Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>
      </div>
    {/if}
  </div>

  {#if statusReason}
    <div class="text-xs text-destructive bg-destructive/10 p-2 rounded-md flex items-center gap-1.5" role="alert">
      <AlertCircle class="size-3.5 shrink-0" />
      <span>{statusReason}</span>
    </div>
  {/if}

  {#if sendError}
    <div class="text-xs text-destructive bg-destructive/10 p-2 rounded-md flex items-center gap-1.5" role="alert">
      <AlertCircle class="size-3.5 shrink-0" />
      <span>{sendError}</span>
    </div>
  {/if}

  <div class="flex items-end gap-2">
    <Textarea
      bind:value={text}
      onkeydown={handleKeydown}
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
