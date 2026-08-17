<script lang="ts">
  import { tick } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import { linkConversationIdentity } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import { UserPlus, AlertCircle, Loader2 } from 'lucide-svelte';
  import { m } from '$lib/i18n';

  let {
    open = false,
    initialName = '',
    initialPhoneOrUid = '',
    identityId = null,
    onClose = () => {},
    onSuccess = () => {}
  }: {
    open?: boolean;
    initialName?: string;
    initialPhoneOrUid?: string;
    identityId?: number | null;
    onClose?: () => void;
    onSuccess?: (leadId: number) => void;
  } = $props();

  let name = $state('');
  let company = $state('Individual');
  let phone = $state('');
  let email = $state('');
  let status = $state('New');

  let isSubmitting = $state(false);
  let error = $state<string | null>(null);
  let validationError = $state<string | null>(null);

  $effect(() => {
    if (open) {
      name = initialName || '';
      phone = initialPhoneOrUid || '';
      company = 'Individual';
      email = '';
      status = 'New';
      error = null;
      validationError = null;
    }
  });

  async function handleCreateLead() {
    validationError = null;
    error = null;

    const trimmedName = name.trim();
    if (!trimmedName) {
      validationError = m.create_lead_name_required();
      return;
    }

    isSubmitting = true;
    try {
      const leadId = await crmService.addLead(
        trimmedName,
        company.trim() || 'Individual',
        status,
        0,
        'Self',
        email.trim(),
        phone.trim(),
        m.create_lead_created_from_zalo({ info: phone.trim() || 'No UID' })
      );

      if (identityId) {
        await linkConversationIdentity(identityId, leadId);
      }

      onSuccess(leadId);
      onClose();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      isSubmitting = false;
    }
  }
</script>

<Dialog.Root bind:open onOpenChange={(v) => !v && onClose()}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title class="flex items-center gap-2">
        <UserPlus class="h-5 w-5 text-primary" />
        <span>{m.leads_modal_add_title({ entity: 'CRM Lead' })}</span>
      </Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        {m.tab_leads_subtitle()}
      </Dialog.Description>
    </Dialog.Header>

    {#if error}
      <div class="flex items-center gap-2 rounded-lg bg-destructive/10 p-3 text-xs text-destructive border border-destructive/20">
        <AlertCircle class="h-4 w-4 shrink-0" />
        <span>{error}</span>
      </div>
    {/if}

    {#if validationError}
      <div class="flex items-center gap-2 rounded-lg bg-amber-500/10 p-3 text-xs text-amber-600 dark:text-amber-400 border border-amber-500/20">
        <AlertCircle class="h-4 w-4 shrink-0" />
        <span>{validationError}</span>
      </div>
    {/if}

    <form onsubmit={(e) => { e.preventDefault(); void handleCreateLead(); }} class="space-y-4 py-2">
      <div class="space-y-1.5">
        <label for="create-lead-name" class="text-xs font-semibold text-foreground">
          {m.leads_modal_name_label()} <span class="text-destructive">*</span>
        </label>
        <Input
          id="create-lead-name"
          bind:value={name}
          placeholder={m.create_lead_name_placeholder()}
          class="h-9 text-xs"
        />
      </div>

      <div class="space-y-1.5">
        <label for="create-lead-phone" class="text-xs font-semibold text-foreground">
          {m.leads_modal_phone_label()}
        </label>
        <Input
          id="create-lead-phone"
          bind:value={phone}
          placeholder={m.create_lead_phone_placeholder()}
          class="h-9 text-xs font-mono"
        />
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1.5">
          <label for="create-lead-company" class="text-xs font-semibold text-foreground">
            {m.leads_modal_company_label()}
          </label>
          <Input
            id="create-lead-company"
            bind:value={company}
            placeholder={m.create_lead_company_placeholder()}
            class="h-9 text-xs"
          />
        </div>

        <div class="space-y-1.5">
          <label for="create-lead-email" class="text-xs font-semibold text-foreground">
            {m.leads_modal_email_label()}
          </label>
          <Input
            id="create-lead-email"
            type="email"
            bind:value={email}
            placeholder={m.create_lead_email_placeholder()}
            class="h-9 text-xs"
          />
        </div>
      </div>

      <Dialog.Footer class="pt-2 flex justify-end gap-2">
        <Button type="button" variant="outline" size="sm" onclick={onClose} disabled={isSubmitting}>
          {m.common_cancel()}
        </Button>
        <Button
          type="submit"
          size="sm"
          disabled={isSubmitting}
          class="bg-[#00a884] hover:bg-[#008f70] text-white gap-1.5"
        >
          {#if isSubmitting}
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            <span>{m.common_loading()}</span>
          {:else}
            <UserPlus class="h-3.5 w-3.5" />
            <span>{m.leads_modal_save_btn({ entity: 'CRM Lead' })}</span>
          {/if}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
