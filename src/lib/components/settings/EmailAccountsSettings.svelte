<script lang="ts">
  import { onMount } from 'svelte';
  import {
    deleteSmtpAccount,
    listManagedSmtpAccounts,
    saveSmtpAccount,
    testSmtpConnection,
    type ManagedSmtpAccount,
    type SmtpServerInput
  } from '$lib/services/accountSettingsService';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import { AlertCircle, Edit3, LoaderCircle, Mail, Plus, RefreshCw, Server, Trash2 } from 'lucide-svelte';

  let accounts = $state<ManagedSmtpAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let formError = $state<string | null>(null);
  let testResult = $state<{ success: boolean; message: string } | null>(null);
  let testing = $state(false);
  let saving = $state(false);
  let dialogOpen = $state(false);

  let editingId = $state<number | null>(null);
  let name = $state('');
  let host = $state('');
  let port = $state(587);
  let username = $state('');
  let password = $state('');
  let fromEmail = $state('');
  let fromName = $state('');
  let security = $state('STARTTLS');

  async function loadAccounts() {
    loading = true;
    loadError = null;
    try {
      accounts = await listManagedSmtpAccounts();
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    } finally {
      loading = false;
    }
  }

  function resetForm() {
    editingId = null;
    name = '';
    host = '';
    port = 587;
    username = '';
    password = '';
    fromEmail = '';
    fromName = '';
    security = 'STARTTLS';
    formError = null;
    testResult = null;
  }

  function openAddDialog() {
    resetForm();
    dialogOpen = true;
  }

  function openEditDialog(account: ManagedSmtpAccount) {
    editingId = account.id;
    name = account.name;
    host = account.host;
    port = account.port;
    username = account.username;
    password = '';
    fromEmail = account.fromEmail;
    fromName = account.fromName;
    security = account.security;
    formError = null;
    testResult = null;
    dialogOpen = true;
  }

  function payload(): SmtpServerInput {
    return {
      id: editingId ?? undefined,
      name: name.trim(),
      host: host.trim(),
      port: Number(port),
      username: username.trim(),
      password,
      from_email: fromEmail.trim(),
      from_name: fromName.trim(),
      security,
      created_at: new Date().toISOString()
    };
  }

  async function saveAccount() {
    if (!name.trim() || !host.trim()) {
      formError = 'Account name and SMTP host are required.';
      return;
    }

    saving = true;
    formError = null;
    try {
      await saveSmtpAccount(payload());
      await loadAccounts();
      dialogOpen = false;
    } catch (error) {
      formError = error instanceof Error ? error.message : String(error);
    } finally {
      saving = false;
    }
  }

  async function testConnection() {
    testing = true;
    testResult = null;
    try {
      testResult = { success: true, message: await testSmtpConnection(host.trim(), Number(port)) };
    } catch (error) {
      testResult = { success: false, message: error instanceof Error ? error.message : String(error) };
    } finally {
      testing = false;
    }
  }

  async function removeAccount(account: ManagedSmtpAccount) {
    try {
      await deleteSmtpAccount(account.id);
      await loadAccounts();
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    }
  }

  onMount(loadAccounts);
</script>

<div class="flex flex-col gap-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h3 class="text-base font-semibold text-foreground">Email accounts</h3>
      <p class="text-xs text-muted-foreground">Manage reusable SMTP senders. Stored passwords never return to this screen.</p>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={loadAccounts} disabled={loading}>
        <RefreshCw class="size-4 {loading ? 'animate-spin' : ''}" /> Refresh
      </Button>
      <Button size="sm" onclick={openAddDialog}><Plus class="size-4" /> Add email account</Button>
    </div>
  </div>

  {#if loadError}
    <div role="alert" class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{loadError}</div>
  {/if}

  {#if loading && accounts.length === 0}
    <Card.Root>
      <Card.Content class="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
        <LoaderCircle class="size-4 animate-spin" /> Loading email accounts...
      </Card.Content>
    </Card.Root>
  {:else if accounts.length === 0}
    <Card.Root class="border-dashed">
      <Card.Content class="flex flex-col items-center gap-3 py-10 text-center">
        <div class="rounded-full bg-sky-500/10 p-3 text-sky-600"><Mail class="size-6" /></div>
        <div>
          <p class="text-sm font-semibold">No email account configured</p>
          <p class="text-xs text-muted-foreground">Add SMTP credentials here before launching an email campaign.</p>
        </div>
        <Button size="sm" onclick={openAddDialog}>Add email account</Button>
      </Card.Content>
    </Card.Root>
  {:else}
    <div class="grid gap-3 xl:grid-cols-2">
      {#each accounts as account (account.id)}
        <Card.Root>
          <Card.Header class="border-b border-border/70 pb-4">
            <div class="flex items-start justify-between gap-3">
              <div class="flex min-w-0 items-center gap-3">
                <div class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-sky-500/10 text-sky-700"><Server class="size-5" /></div>
                <div class="min-w-0">
                  <Card.Title class="truncate text-sm">{account.name}</Card.Title>
                  <Card.Description class="truncate text-xs">{account.host}:{account.port}</Card.Description>
                </div>
              </div>
              <Badge variant="secondary">SMTP</Badge>
            </div>
          </Card.Header>
          <Card.Content class="flex flex-col gap-4 pt-4">
            <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
              <dt class="text-muted-foreground">Sender</dt><dd class="truncate text-right font-medium">{account.fromName || account.fromEmail}</dd>
              <dt class="text-muted-foreground">Address</dt><dd class="truncate text-right">{account.fromEmail}</dd>
              <dt class="text-muted-foreground">Security</dt><dd class="text-right">{account.security}</dd>
            </dl>
            <div class="flex justify-end gap-2">
              <Button variant="outline" size="sm" aria-label={`Edit ${account.name}`} onclick={() => openEditDialog(account)}>
                <Edit3 class="size-4" /> Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Delete ${account.name}`}
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                onclick={() => removeAccount(account)}
              >
                <Trash2 class="size-4" /> Delete
              </Button>
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="sm:max-w-xl">
    <Dialog.Header>
      <Dialog.Title>{editingId ? 'Edit email account' : 'Add email account'}</Dialog.Title>
      <Dialog.Description>
        {editingId ? 'Leave the replacement password empty to keep the stored password.' : 'Configure an SMTP sender for outreach campaigns.'}
      </Dialog.Description>
    </Dialog.Header>

    <div class="grid gap-3 py-2 text-xs">
      {#if formError}
        <div role="alert" class="rounded-lg bg-destructive/10 px-3 py-2 text-destructive">{formError}</div>
      {/if}
      <div class="grid gap-1.5">
        <label for="settings-smtp-name" class="font-medium">Account name</label>
        <Input id="settings-smtp-name" bind:value={name} placeholder="Primary SMTP" />
      </div>
      <div class="grid grid-cols-[1fr_110px] gap-3">
        <div class="grid gap-1.5">
          <label for="settings-smtp-host" class="font-medium">SMTP host</label>
          <Input id="settings-smtp-host" bind:value={host} placeholder="smtp.example.com" />
        </div>
        <div class="grid gap-1.5">
          <label for="settings-smtp-port" class="font-medium">Port</label>
          <Input id="settings-smtp-port" type="number" bind:value={port} />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div class="grid gap-1.5">
          <label for="settings-smtp-username" class="font-medium">Username</label>
          <Input id="settings-smtp-username" bind:value={username} />
        </div>
        <div class="grid gap-1.5">
          <label for="settings-smtp-password" class="font-medium">Replacement password</label>
          <Input id="settings-smtp-password" type="password" bind:value={password} placeholder={editingId ? 'Keep current password' : 'Password'} />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div class="grid gap-1.5">
          <label for="settings-smtp-from-name" class="font-medium">From name</label>
          <Input id="settings-smtp-from-name" bind:value={fromName} />
        </div>
        <div class="grid gap-1.5">
          <label for="settings-smtp-from-email" class="font-medium">From email</label>
          <Input id="settings-smtp-from-email" type="email" bind:value={fromEmail} />
        </div>
      </div>
      <div class="grid gap-1.5">
        <label for="settings-smtp-security" class="font-medium">Security</label>
        <Select.Root type="single" bind:value={security}>
          <Select.Trigger id="settings-smtp-security" class="w-full"><span>{security}</span></Select.Trigger>
          <Select.Content>
            <Select.Item value="STARTTLS">STARTTLS</Select.Item>
            <Select.Item value="TLS">TLS</Select.Item>
            <Select.Item value="NONE">None</Select.Item>
          </Select.Content>
        </Select.Root>
      </div>
      <div class="flex items-center justify-between gap-3 border-t pt-3">
        <Button variant="outline" size="sm" onclick={testConnection} disabled={testing || !host.trim()}>
          <RefreshCw class="size-4 {testing ? 'animate-spin' : ''}" /> Test connection
        </Button>
        {#if testResult}
          <p class="text-xs {testResult.success ? 'text-emerald-700' : 'text-destructive'}">{testResult.message}</p>
        {/if}
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (dialogOpen = false)}>Cancel</Button>
      <Button onclick={saveAccount} disabled={saving}>{saving ? 'Saving...' : 'Save account'}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
