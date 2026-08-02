<script lang="ts">
  import type { ConversationMessage } from '$lib/services/conversationService';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';

  let {
    message = null,
    busy = false,
    error = null,
    onConfirm = () => {},
    onCancel = () => {}
  }: {
    message?: ConversationMessage | null;
    busy?: boolean;
    error?: string | null;
    onConfirm?: () => void;
    onCancel?: () => void;
  } = $props();
</script>

{#if message}
  <Dialog.Root open={true} onOpenChange={(open) => !open && !busy && onCancel()}>
    <Dialog.Content class="sm:max-w-[420px]">
      <Dialog.Header>
        <Dialog.Title>Thu hồi tin nhắn</Dialog.Title>
        <Dialog.Description>
          Tin nhắn sẽ được thu hồi trên Zalo sau khi nhà cung cấp xác nhận thành công.
        </Dialog.Description>
      </Dialog.Header>

      {#if error}
        <div class="rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
          {error}
        </div>
      {/if}

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>Hủy</Button>
        <Button type="button" variant="destructive" onclick={onConfirm} disabled={busy}>
          {busy ? 'Đang thu hồi...' : 'Xác nhận thu hồi'}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
{/if}
