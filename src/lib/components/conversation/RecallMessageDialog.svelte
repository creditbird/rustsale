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
        <Dialog.Title>Recall Message</Dialog.Title>
        <Dialog.Description>
          The message will be recalled on Zalo once confirmed by the provider.
        </Dialog.Description>
      </Dialog.Header>

      {#if error}
        <div class="rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
          {error}
        </div>
      {/if}

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>Cancel</Button>
        <Button type="button" variant="destructive" onclick={onConfirm} disabled={busy}>
          {busy ? 'Recalling...' : 'Confirm Recall'}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
{/if}
