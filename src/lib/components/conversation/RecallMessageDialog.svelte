<script lang="ts">
  import type { ConversationMessage } from '$lib/services/conversationService';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import * as m from '$lib/paraglide/messages';

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
        <Dialog.Title>{m.conversation_recall_title()}</Dialog.Title>
        <Dialog.Description>
          {m.conversation_recall_desc()}
        </Dialog.Description>
      </Dialog.Header>

      {#if error}
        <div class="rounded-md bg-destructive/10 p-2 text-xs text-destructive" role="alert">
          {error}
        </div>
      {/if}

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>
          {m.common_cancel()}
        </Button>
        <Button type="button" variant="destructive" onclick={onConfirm} disabled={busy}>
          {busy ? m.conversation_recall_in_progress() : m.conversation_recall_confirm()}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
{/if}
