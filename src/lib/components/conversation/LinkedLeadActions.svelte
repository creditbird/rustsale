<script lang="ts">
  import { linkConversationIdentity, unlinkConversationIdentity, type InboxItem } from '$lib/services/conversationService';
  import * as crmService from '$lib/services/crmService';
  import type { Lead } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import { RefreshCw, Unlink, Search, AlertCircle } from 'lucide-svelte';

  let {
    item,
    onRelinked,
    onUnlinked
  }: {
    item: InboxItem;
    onRelinked?: (leadId: number) => void;
    onUnlinked?: () => void;
  } = $props();

  let isRelinkModalOpen = $state(false);
  let isUnlinkModalOpen = $state(false);

  // Relink state
  let leads = $state<Lead[]>([]);
  let isLoadingLeads = $state(false);
  let leadSearchText = $state('');
  let selectedLeadId = $state<number | null>(null);
  let isRelinking = $state(false);
  let relinkError = $state<string | null>(null);

  // Unlink state
  let isUnlinking = $state(false);
  let unlinkError = $state<string | null>(null);

  let filteredLeads = $derived.by(() => {
    const q = leadSearchText.trim().toLowerCase();
    if (!q) return leads;
    return leads.filter(
      (l) =>
        l.name.toLowerCase().includes(q) ||
        l.company.toLowerCase().includes(q) ||
        (l.email && l.email.toLowerCase().includes(q)) ||
        (l.phone && l.phone.includes(q))
    );
  });

  async function openRelinkModal() {
    relinkError = null;
    selectedLeadId = null;
    leadSearchText = '';
    isRelinkModalOpen = true;
    isLoadingLeads = true;
    try {
      leads = await crmService.getLeads();
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isLoadingLeads = false;
    }
  }

  async function handleConfirmRelink() {
    if (!selectedLeadId || !item.identityId) return;
    isRelinking = true;
    relinkError = null;
    try {
      await linkConversationIdentity(item.identityId, selectedLeadId);
      isRelinkModalOpen = false;
      onRelinked?.(selectedLeadId);
    } catch (err) {
      relinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isRelinking = false;
    }
  }

  async function handleConfirmUnlink() {
    if (!item.identityId) return;
    isUnlinking = true;
    unlinkError = null;
    try {
      await unlinkConversationIdentity(item.identityId);
      isUnlinkModalOpen = false;
      onUnlinked?.();
    } catch (err) {
      unlinkError = err instanceof Error ? err.message : String(err);
    } finally {
      isUnlinking = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId}
    onclick={openRelinkModal}
    class="flex items-center gap-1.5 text-xs"
  >
    <RefreshCw class="size-3.5" data-icon="inline-start" />
    <span>Đổi lead</span>
  </Button>

  <Button
    type="button"
    variant="outline"
    size="sm"
    disabled={!item.identityId}
    onclick={() => (isUnlinkModalOpen = true)}
    class="flex items-center gap-1.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
  >
    <Unlink class="size-3.5 text-destructive" data-icon="inline-start" />
    <span>Hủy liên kết</span>
  </Button>
</div>

<!-- Modal 1: Đổi lead -->
<Dialog.Root bind:open={isRelinkModalOpen}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>Đổi liên kết lead</Dialog.Title>
      <Dialog.Description>
        Chọn lead mới trong CRM để thay thế cho cuộc trò chuyện này.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-3 py-3">
      {#if relinkError}
        <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5" role="alert">
          <AlertCircle class="size-4 shrink-0" />
          <span>{relinkError}</span>
        </div>
      {/if}

      <div class="relative">
        <Search class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          bind:value={leadSearchText}
          placeholder="Tìm theo tên, email, sđt..."
          class="pl-8 text-sm"
        />
      </div>

      <div class="max-h-[220px] overflow-y-auto border rounded-md p-1 flex flex-col gap-1">
        {#if isLoadingLeads}
          <div class="text-xs text-center text-muted-foreground py-4">Đang tải danh sách lead...</div>
        {:else if filteredLeads.length === 0}
          <div class="text-xs text-center text-muted-foreground py-4">Không tìm thấy lead phù hợp.</div>
        {:else}
          {#each filteredLeads as lead (lead.id)}
            <button
              type="button"
              onclick={() => (selectedLeadId = lead.id ?? null)}
              class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors flex items-center justify-between {selectedLeadId === lead.id ? 'bg-accent text-accent-foreground font-medium' : 'hover:bg-muted/50'}"
            >
              <div>
                <div class="font-medium">{lead.name}</div>
                <div class="text-xs text-muted-foreground">{lead.company || 'Cá nhân'} {lead.phone ? `• ${lead.phone}` : ''}</div>
              </div>
              {#if selectedLeadId === lead.id}
                <span class="text-xs font-semibold text-primary">Đã chọn</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isRelinkModalOpen = false)}>Hủy</Button>
      <Button
        disabled={!selectedLeadId || isRelinking}
        onclick={handleConfirmRelink}
      >
        {isRelinking ? 'Đang cập nhật...' : 'Xác nhận đổi'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Modal 2: Xác nhận Hủy liên kết -->
<Dialog.Root bind:open={isUnlinkModalOpen}>
  <Dialog.Content class="sm:max-w-[420px]">
    <Dialog.Header>
      <Dialog.Title>Xác nhận hủy liên kết</Dialog.Title>
      <Dialog.Description>
        Bạn có chắc chắn muốn hủy liên kết giữa cuộc trò chuyện này và lead <strong>{item.title}</strong>? Cuộc trò chuyện sẽ quay về danh mục Chưa liên kết (Unknown).
      </Dialog.Description>
    </Dialog.Header>

    {#if unlinkError}
      <div class="text-xs text-destructive bg-destructive/10 p-2.5 rounded-md flex items-center gap-1.5 my-2" role="alert">
        <AlertCircle class="size-4 shrink-0" />
        <span>{unlinkError}</span>
      </div>
    {/if}

    <Dialog.Footer class="mt-4">
      <Button variant="outline" onclick={() => (isUnlinkModalOpen = false)}>Bỏ qua</Button>
      <Button
        variant="destructive"
        disabled={isUnlinking}
        onclick={handleConfirmUnlink}
      >
        {isUnlinking ? 'Đang hủy...' : 'Xác nhận hủy'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
