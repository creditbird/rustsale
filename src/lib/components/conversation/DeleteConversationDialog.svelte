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
    type InboxItem
  } from '$lib/services/conversationService';

  let {
    open = $bindable(false),
    item,
    onDeleted
  }: {
    open: boolean;
    item: InboxItem | null;
    onDeleted?: () => void;
  } = $props();

  let submitting = $state(false);
  let errorMessage = $state<string | null>(null);

  async function handleConfirm() {
    if (!item) return;
    submitting = true;
    errorMessage = null;

    try {
      await deleteConversation({
        conversationId: item.conversationId,
        identityId: item.identityId,
        leadId: item.leadId
      });
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
      <DialogTitle class="text-destructive">Xóa cuộc trò chuyện</DialogTitle>
      <DialogDescription class="pt-1 text-sm">
        {#if item}
          Bạn có chắc chắn muốn xóa cuộc trò chuyện với <strong>{item.title}</strong>? Lịch sử tin nhắn của cuộc trò chuyện này sẽ bị xóa khỏi ứng dụng.
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
        {submitting ? 'Đang xóa...' : 'Xóa trò chuyện'}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
