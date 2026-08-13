<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle
  } from '$lib/components/ui/dialog';
  import {
    deleteConversation,
    deleteConversationsBatch,
    type InboxItem
  } from '$lib/services/conversationService';

  let {
    open = $bindable(false),
    item = null,
    items = [],
    onDeleted
  }: {
    open: boolean;
    item?: InboxItem | null;
    items?: InboxItem[];
    onDeleted?: () => void;
  } = $props();

  let submitting = $state(false);
  let errorMessage = $state<string | null>(null);

  let targetList = $derived(items && items.length > 0 ? items : item ? [item] : []);
  let count = $derived(targetList.length);
  let singleItem = $derived(count === 1 ? targetList[0] : null);

  function buildDeleteRequest(target: InboxItem) {
    if (target.conversationId && target.conversationId > 0) {
      return {
        conversationId: target.conversationId,
        identityId: null,
        leadId: null
      };
    }
    if (target.identityId && target.identityId > 0) {
      return {
        conversationId: null,
        identityId: target.identityId,
        leadId: null
      };
    }
    return {
      conversationId: null,
      identityId: null,
      leadId: target.leadId
    };
  }

  async function handleConfirm() {
    if (count === 0) return;
    submitting = true;
    errorMessage = null;

    try {
      if (count === 1 && singleItem) {
        await deleteConversation(buildDeleteRequest(singleItem));
      } else {
        await deleteConversationsBatch(targetList.map(buildDeleteRequest));
      }
      open = false;
      onDeleted?.();
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-[400px]">
    <DialogHeader>
      <DialogTitle class="text-destructive">
        {count > 1 ? `Xóa ${count} cuộc trò chuyện` : 'Xóa cuộc trò chuyện'}
      </DialogTitle>
      <DialogDescription class="pt-1 text-sm">
        {#if count > 1}
          Bạn có chắc chắn muốn xóa <strong>{count}</strong> cuộc trò chuyện đã chọn? Lịch sử tin nhắn của các cuộc trò chuyện này sẽ bị xóa khỏi ứng dụng.
        {:else if singleItem}
          Bạn có chắc chắn muốn xóa cuộc trò chuyện với <strong>{singleItem.title}</strong>? Lịch sử tin nhắn của cuộc trò chuyện này sẽ bị xóa khỏi ứng dụng.
        {:else}
          Bạn có chắc chắn muốn xóa cuộc trò chuyện này?
        {/if}
      </DialogDescription>
    </DialogHeader>

    {#if errorMessage}
      <div class="p-2.5 rounded bg-destructive/10 text-destructive text-xs font-medium my-2">
        {errorMessage}
      </div>
    {/if}

    <DialogFooter class="pt-3">
      <Button variant="outline" onclick={() => (open = false)} disabled={submitting}>
        Hủy
      </Button>
      <Button variant="destructive" onclick={handleConfirm} disabled={submitting}>
        {submitting ? 'Đang xóa...' : count > 1 ? `Xóa (${count})` : 'Xóa trò chuyện'}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
