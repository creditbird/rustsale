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
    initialPhone = '',
    initialDisplayName = '',
    onSuccess
  }: {
    open: boolean;
    initialPhone?: string;
    initialDisplayName?: string;
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
      phoneOrExternalId = (initialPhone || '').trim();
      displayName = initialDisplayName || '';
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
      errorMessage = 'Please enter Phone Number or Zalo ID';
      return;
    }
    if (!text) {
      errorMessage = 'Please enter initial message';
      return;
    }
    if (!selectedAccountId) {
      errorMessage = 'Please select a sender Zalo account';
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
      const rawMsg = err instanceof Error ? err.message : String(err);
      if (rawMsg.includes('114') || rawMsg.includes('Tham số không hợp lệ') || rawMsg.includes('invalid parameter')) {
        errorMessage = 'Zalo account not found for this Phone Number / Zalo ID. Please enter a valid phone number or Zalo UID.';
      } else {
        errorMessage = rawMsg;
      }
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-[425px]">
    <DialogHeader>
      <DialogTitle>New Chat via Phone / Zalo ID</DialogTitle>
      <DialogDescription>
        Start a direct conversation and send the first message to customer.
      </DialogDescription>
    </DialogHeader>

    <form onsubmit={handleSubmit} class="space-y-4 py-2">
      {#if errorMessage}
        <div class="p-2.5 rounded-lg bg-destructive/10 text-destructive text-xs font-medium border border-destructive/20">
          {errorMessage}
        </div>
      {/if}

      <div class="space-y-1.5">
        <Label for="phone">Phone Number / Zalo ID <span class="text-destructive">*</span></Label>
        <Input
          id="phone"
          type="text"
          placeholder="e.g. 0987654321 or Zalo UID"
          bind:value={phoneOrExternalId}
          disabled={submitting}
          required
        />
        <p class="text-[11px] text-muted-foreground">
          {#if displayName}
            💡 Please enter Phone Number (09x/08x...) or Zalo UID for <strong>{displayName}</strong>.
          {:else}
            💡 Enter customer's Zalo Phone Number (09x/08x/03x...) or Zalo UID.
          {/if}
        </p>
      </div>

      <div class="space-y-1.5">
        <Label for="display-name">Display Name / Contact Name (Optional)</Label>
        <Input
          id="display-name"
          type="text"
          placeholder="e.g. John Doe"
          bind:value={displayName}
          disabled={submitting}
        />
      </div>

      <div class="space-y-1.5">
        <Label for="account-select">Sender Zalo Account <span class="text-destructive">*</span></Label>
        {#if loadingAccounts}
          <div class="text-xs text-muted-foreground italic">Loading account list...</div>
        {:else if accounts.length === 0}
          <div class="text-xs text-destructive">No Zalo account connected. Please go to Settings to add an account.</div>
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
        <Label for="initial-msg">Initial Message <span class="text-destructive">*</span></Label>
        <Textarea
          id="initial-msg"
          placeholder="Enter initial message to send to customer..."
          bind:value={initialMessage}
          rows={3}
          disabled={submitting}
          required
        />
      </div>

      <DialogFooter class="pt-2">
        <Button type="button" variant="outline" onclick={() => (open = false)} disabled={submitting}>
          Cancel
        </Button>
        <Button type="submit" disabled={submitting || accounts.length === 0}>
          {submitting ? 'Sending...' : 'Start Chat'}
        </Button>
      </DialogFooter>
    </form>
  </DialogContent>
</Dialog>
