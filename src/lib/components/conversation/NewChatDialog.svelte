<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Textarea } from '$lib/components/ui/textarea';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle
  } from '$lib/components/ui/dialog';
  import {
    getConversationRuntimeStatus,
    startDirectConversation,
    type ConversationAccountRuntime,
    type ConversationMessage
  } from '$lib/services/conversationService';

  let {
    open = $bindable(false),
    onSuccess
  }: {
    open: boolean;
    onSuccess?: (message: ConversationMessage) => void;
  } = $props();

  let phoneOrExternalId = $state('');
  let displayName = $state('');
  let initialMessage = $state('');
  let selectedAccountId = $state<number | null>(null);
  let accounts = $state<ConversationAccountRuntime[]>([]);
  let loadingAccounts = $state(false);
  let submitting = $state(false);
  let errorMessage = $state<string | null>(null);

  $effect(() => {
    if (open) {
      phoneOrExternalId = '';
      displayName = '';
      initialMessage = '';
      errorMessage = null;
      loadAccounts();
    }
  });

  async function loadAccounts() {
    loadingAccounts = true;
    try {
      const runtime = await getConversationRuntimeStatus();
      accounts = runtime.accounts.filter((a) => a.channel === 'zalo');
      if (accounts.length > 0) {
        selectedAccountId = accounts[0].channelAccountId;
      }
    } catch (e: unknown) {
      console.error('Failed to load accounts', e);
    } finally {
      loadingAccounts = false;
    }
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const phone = phoneOrExternalId.trim();
    const text = initialMessage.trim();

    if (!phone) {
      errorMessage = 'Vui lòng nhập Số điện thoại hoặc Zalo ID';
      return;
    }
    if (!text) {
      errorMessage = 'Vui lòng nhập tin nhắn ban đầu';
      return;
    }
    if (!selectedAccountId) {
      errorMessage = 'Vui lòng chọn tài khoản Zalo gửi';
      return;
    }

    submitting = true;
    errorMessage = null;

    try {
      const msg = await startDirectConversation({
        channelAccountId: selectedAccountId,
        phoneOrExternalId: phone,
        displayName: displayName.trim() || null,
        initialMessage: text
      });
      open = false;
      onSuccess?.(msg);
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-[425px]">
    <DialogHeader>
      <DialogTitle>Chat với Số điện thoại / Zalo ID mới</DialogTitle>
      <DialogDescription>
        Khởi tạo cuộc trò chuyện trực tiếp và gửi tin nhắn đầu tiên cho khách hàng.
      </DialogDescription>
    </DialogHeader>

    <form onsubmit={handleSubmit} class="space-y-4 py-2">
      {#if errorMessage}
        <div class="p-2.5 rounded bg-destructive/10 text-destructive text-xs font-medium">
          {errorMessage}
        </div>
      {/if}

      <div class="space-y-1.5">
        <Label for="phone">Số điện thoại / Zalo ID <span class="text-destructive">*</span></Label>
        <Input
          id="phone"
          type="text"
          placeholder="Ví dụ: 0987654321"
          bind:value={phoneOrExternalId}
          disabled={submitting}
          required
        />
      </div>

      <div class="space-y-1.5">
        <Label for="display-name">Tên hiển thị / Tên khách (Tùy chọn)</Label>
        <Input
          id="display-name"
          type="text"
          placeholder="Ví dụ: Anh Nam"
          bind:value={displayName}
          disabled={submitting}
        />
      </div>

      <div class="space-y-1.5">
        <Label for="account-select">Tài khoản Zalo gửi <span class="text-destructive">*</span></Label>
        {#if loadingAccounts}
          <div class="text-xs text-muted-foreground italic">Đang tải danh sách tài khoản...</div>
        {:else if accounts.length === 0}
          <div class="text-xs text-destructive">Chưa có tài khoản Zalo nào được kết nối. Vui lòng vào Cài đặt để thêm tài khoản.</div>
        {:else}
          <select
            id="account-select"
            class="w-full h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs transition-colors focus-visible:outline-hidden focus-visible:ring-1 focus-visible:ring-ring"
            bind:value={selectedAccountId}
            disabled={submitting}
          >
            {#each accounts as acc (acc.channelAccountId)}
              <option value={acc.channelAccountId}>
                {acc.displayName || acc.providerAccountKey} ({acc.status})
              </option>
            {/each}
          </select>
        {/if}
      </div>

      <div class="space-y-1.5">
        <Label for="initial-msg">Tin nhắn khởi tạo <span class="text-destructive">*</span></Label>
        <Textarea
          id="initial-msg"
          placeholder="Nhập nội dung tin nhắn gửi cho khách hàng..."
          bind:value={initialMessage}
          rows={3}
          disabled={submitting}
          required
        />
      </div>

      <DialogFooter class="pt-2">
        <Button type="button" variant="outline" onclick={() => (open = false)} disabled={submitting}>
          Hủy
        </Button>
        <Button type="submit" disabled={submitting || accounts.length === 0}>
          {submitting ? 'Đang gửi...' : 'Bắt đầu chat'}
        </Button>
      </DialogFooter>
    </form>
  </DialogContent>
</Dialog>
