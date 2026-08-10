<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import {
    getGroupMembersUnrestricted,
    syncAndDeepScanGroupMembers,
    batchImportGroupMembersToLeads,
    type GroupMemberDetail,
    type GroupMemberScanResult,
    type ImportMemberItem
  } from '$lib/services/conversationService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Badge } from '$lib/components/ui/badge';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Dialog as DialogPrimitive } from 'bits-ui';
  import { buildGroupMembersCsv } from './groupMembersCsv';
  import {
    Users,
    Search,
    ShieldAlert,
    ShieldCheck,
    Download,
    UserPlus,
    Tag,
    X,
    Check,
    Loader2,
    Crown,
    CheckCircle2,
    Scan
  } from 'lucide-svelte';

  let {
    open = false,
    conversationId = null,
    groupTitle = '',
    onClose = () => {},
    onLeadsImported = (_count: number) => {}
  }: {
    open?: boolean;
    conversationId?: number | null;
    groupTitle?: string;
    onClose?: () => void;
    onLeadsImported?: (count: number) => void;
  } = $props();

  let members = $state<GroupMemberDetail[]>([]);
  let loading = $state(false);
  let isScanning = $state(false);
  let error = $state<string | null>(null);
  let scanResult = $state<GroupMemberScanResult | null>(null);
  let announcement = $state('');

  let searchQuery = $state('');
  let activeFilter = $state<'all' | 'no_lead' | 'has_lead' | 'admin'>('all');
  let selectedKeys = $state<Set<string>>(new Set());

  // Category & Import Wizard state
  let wizardOpen = $state(false);
  let existingCategories = $state<{ id: number; name: string }[]>([]);
  let selectedCategoryIds = $state<Set<number>>(new Set());
  let newCategoryName = $state('');
  let isImporting = $state(false);
  let importSuccessMessage = $state<string | null>(null);
  let requestGeneration = 0;
  let activeRequestContext: { generation: number; conversationId: number } | null = null;
  let wizardCloseTimer: ReturnType<typeof setTimeout> | null = null;

  const membershipComplete = $derived(scanResult?.membership_complete ?? false);

  const diagnosticWarnings = $derived.by(() => {
    if (!scanResult) return [];
    const warnings = [
      scanResult.diagnostics.direct_snapshot.warning,
      scanResult.diagnostics.group_link?.warning,
      ...scanResult.diagnostics.board.warnings.map((warning) => ({
        stage: warning.source,
        code: warning.code,
        context: warning.context,
        message: warning.message
      })),
      ...scanResult.diagnostics.profile_hydration.warnings,
      ...scanResult.diagnostics.warnings
    ].filter((warning): warning is NonNullable<typeof warning> => Boolean(warning));
    const seen = new Set<string>();
    return warnings.filter((warning) => {
      const key = `${warning.stage}:${warning.code}:${warning.message}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  });

  const diagnosticSummary = $derived.by(() => {
    if (!scanResult) {
      return 'Đang hiển thị UID đã lưu cục bộ. Nhấn Deep Scan để bổ sung dữ liệu từ Zalo.';
    }
    if (scanResult.diagnostics.direct_snapshot.complete) {
      return `Danh sách thành viên đầy đủ từ snapshot nhà cung cấp: ${scanResult.discovered_count} thành viên.`;
    }
    const diagnostics = scanResult.diagnostics;
    return [
      `Snapshot chưa đầy đủ (${diagnostics.direct_snapshot.member_count} UID)`,
      diagnostics.group_link?.attempted
        ? `Link/Invite ${diagnostics.group_link.committed_uid_count} UID`
        : null,
      `Board/Poll ${diagnostics.board.pages_succeeded}/${diagnostics.board.pages_attempted} trang`,
      `Lịch sử WebSocket toàn tài khoản: ${diagnostics.history_backfill.outcome}`,
      `Local ${diagnostics.local_evidence.messages_scanned} tin nhắn`,
      `Hồ sơ ${diagnostics.profile_hydration.succeeded}/${diagnostics.profile_hydration.attempted}`
    ]
      .filter((item): item is string => Boolean(item))
      .join(' · ');
  });

  function getMemberKey(m: GroupMemberDetail): string {
    return m.external_id || (m.identity_id ? String(m.identity_id) : m.display_name);
  }

  $effect(() => {
    const activeOpen = open;
    const activeConversationId = conversationId;
    const generation =
      activeOpen && activeConversationId != null
        ? beginRequest(activeConversationId)
        : ++requestGeneration;
    if (!activeOpen || activeConversationId == null) activeRequestContext = null;

    if (wizardCloseTimer) {
      clearTimeout(wizardCloseTimer);
      wizardCloseTimer = null;
    }

    resetModalState();
    if (activeOpen && activeConversationId != null) {
      void loadMembers(activeConversationId, generation);
      void loadCategories(generation, activeConversationId);
    }

    return () => {
      if (activeRequestContext?.generation === generation) {
        activeRequestContext = null;
      }
      requestGeneration += 1;
      if (wizardCloseTimer) {
        clearTimeout(wizardCloseTimer);
        wizardCloseTimer = null;
      }
    };
  });

  function resetModalState() {
    members = [];
    loading = false;
    isScanning = false;
    error = null;
    scanResult = null;
    announcement = '';
    selectedKeys = new Set();
    searchQuery = '';
    activeFilter = 'all';
    wizardOpen = false;
    existingCategories = [];
    selectedCategoryIds = new Set();
    newCategoryName = '';
    isImporting = false;
    importSuccessMessage = null;
  }

  function isCurrentRequest(generation: number, id: number): boolean {
    return (
      activeRequestContext?.generation === generation &&
      activeRequestContext.conversationId === id &&
      requestGeneration === generation
    );
  }

  function beginRequest(id: number): number {
    const generation = ++requestGeneration;
    activeRequestContext = { generation, conversationId: id };
    return generation;
  }

  function errorMessage(err: unknown, fallback: string): string {
    if (err instanceof Error && err.message) return err.message;
    if (typeof err === 'string' && err) return err;
    return fallback;
  }

  function replaceMembers(nextMembers: GroupMemberDetail[]) {
    members = nextMembers;
    const nonLeads = members.filter((member) => !member.lead_id).map((member) => getMemberKey(member));
    selectedKeys = new Set(nonLeads);
  }

  function applyScanResult(result: GroupMemberScanResult) {
    replaceMembers(result.members);
    scanResult = result;
  }

  async function loadMembers(id: number, generation: number) {
    if (!isCurrentRequest(generation, id)) return;
    loading = true;
    error = null;
    announcement = 'Đang tải thành viên đã lưu cục bộ.';
    try {
      const nextMembers = await getGroupMembersUnrestricted(id);
      if (!isCurrentRequest(generation, id)) return;
      replaceMembers(nextMembers);
      announcement = `Đã tải ${nextMembers.length} thành viên đã lưu cục bộ.`;
    } catch (err: unknown) {
      if (!isCurrentRequest(generation, id)) return;
      error = errorMessage(err, 'Không thể tải danh sách thành viên');
      announcement = error;
    } finally {
      if (isCurrentRequest(generation, id)) {
        loading = false;
      }
    }
  }

  async function triggerDeepScan() {
    const targetConversationId = conversationId;
    if (targetConversationId == null || isScanning) return;
    const generation = beginRequest(targetConversationId);
    isScanning = true;
    error = null;
    announcement = 'Đang Deep Scan UID và chờ lịch sử WebSocket toàn tài khoản.';
    try {
      const result = await syncAndDeepScanGroupMembers(targetConversationId);
      if (!isCurrentRequest(generation, targetConversationId)) return;
      applyScanResult(result);
      announcement = result.diagnostics.partial_result
        ? `Deep Scan hoàn tất một phần với ${result.discovered_count} UID.`
        : `Deep Scan hoàn tất với ${result.discovered_count} thành viên.`;
    } catch (err: unknown) {
      if (!isCurrentRequest(generation, targetConversationId)) return;
      error = errorMessage(err, 'Không thể đồng bộ thành viên nhóm');
      announcement = error;
    } finally {
      if (isCurrentRequest(generation, targetConversationId)) {
        isScanning = false;
      }
    }
  }

  async function loadCategories(generation: number, id: number) {
    try {
      const cats = await invoke<{ id: number; name: string }[]>('get_categories');
      if (!isCurrentRequest(generation, id)) return;
      existingCategories = cats || [];
    } catch (err) {
      console.error('Failed to load categories:', err);
    }
  }

  const filteredMembers = $derived.by(() => {
    return members.filter((m) => {
      // Filter by search query
      const query = searchQuery.trim().toLowerCase();
      if (query) {
        const nameMatch = m.display_name.toLowerCase().includes(query);
        const uidMatch = m.external_id?.toLowerCase().includes(query) ?? false;
        const leadMatch = m.lead_name?.toLowerCase().includes(query) ?? false;
        if (!nameMatch && !uidMatch && !leadMatch) return false;
      }

      // Filter by category tab
      if (activeFilter === 'no_lead') return !m.lead_id;
      if (activeFilter === 'has_lead') return Boolean(m.lead_id);
      if (activeFilter === 'admin') return m.participant_role === 'admin' || m.participant_role === 'deputy';
      return true;
    });
  });

  const totalCount = $derived(members.length);
  const nonLeadCount = $derived(members.filter((m) => !m.lead_id).length);
  const leadCount = $derived(members.filter((m) => Boolean(m.lead_id)).length);
  const adminCount = $derived(members.filter((m) => m.participant_role === 'admin' || m.participant_role === 'deputy').length);

  const isAllFilteredSelected = $derived.by(() => {
    if (filteredMembers.length === 0) return false;
    return filteredMembers.every((m) => selectedKeys.has(getMemberKey(m)));
  });

  function toggleSelectAllFiltered() {
    const next = new Set(selectedKeys);
    if (isAllFilteredSelected) {
      filteredMembers.forEach((m) => next.delete(getMemberKey(m)));
    } else {
      filteredMembers.forEach((m) => next.add(getMemberKey(m)));
    }
    selectedKeys = next;
  }

  function toggleMemberSelection(key: string) {
    const next = new Set(selectedKeys);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selectedKeys = next;
  }

  function toggleCategorySelection(catId: number) {
    const next = new Set(selectedCategoryIds);
    if (next.has(catId)) {
      next.delete(catId);
    } else {
      next.add(catId);
    }
    selectedCategoryIds = next;
  }

  function exportCSV() {
    const selectedMembers = members.filter((m) => selectedKeys.has(getMemberKey(m)));
    if (selectedMembers.length === 0) return;

    const csvContent = buildGroupMembersCsv(
      selectedMembers.map((member) => ({
        displayName: member.display_name,
        zaloUid: member.external_id ?? '',
        role: member.participant_role,
        crmStatus: member.lead_id ? 'Existing Lead' : 'New',
        categories: member.categories.map((category) => category.name)
      }))
    );
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `GroupMembers_${groupTitle.replace(/[^a-zA-Z0-9]/g, '_')}_${Date.now()}.csv`;
    link.click();
    URL.revokeObjectURL(url);
  }

  function openImportWizard() {
    if (selectedKeys.size === 0) return;
    if (!newCategoryName) {
      newCategoryName = `Lead Nhóm: ${groupTitle}`;
    }
    wizardOpen = true;
  }

  async function executeBatchImport() {
    const targetConversationId = conversationId;
    if (targetConversationId == null || isImporting) return;
    const selectedMembers = members.filter((m) => selectedKeys.has(getMemberKey(m)));
    if (selectedMembers.length === 0) return;

    const generation = beginRequest(targetConversationId);
    isImporting = true;
    error = null;
    announcement = 'Đang import thành viên đã chọn vào CRM.';
    try {
      const items: ImportMemberItem[] = selectedMembers.map((m) => ({
        identity_id: m.identity_id ?? undefined,
        external_id: m.external_id ?? undefined,
        display_name: m.display_name,
        avatar_url: m.avatar_url ?? undefined
      }));

      const res = await batchImportGroupMembersToLeads({
        conversation_id: targetConversationId,
        members: items,
        category_ids: Array.from(selectedCategoryIds),
        new_category_name: newCategoryName.trim() || undefined,
        group_title: groupTitle,
        skip_existing: true
      });
      if (!isCurrentRequest(generation, targetConversationId)) return;

      importSuccessMessage = `Đã thêm ${res.imported_count} lead mới và cập nhật ${res.updated_count} lead sẵn có vào Categories!`;
      onLeadsImported(res.imported_count);
      announcement = importSuccessMessage;

      await loadMembers(targetConversationId, generation);
      if (!isCurrentRequest(generation, targetConversationId)) return;
      wizardCloseTimer = setTimeout(() => {
        if (isCurrentRequest(generation, targetConversationId)) {
          wizardOpen = false;
          importSuccessMessage = null;
        }
      }, 1800);
    } catch (err: unknown) {
      if (!isCurrentRequest(generation, targetConversationId)) return;
      error = errorMessage(err, 'Không thể import thành viên vào CRM');
      announcement = error;
    } finally {
      if (isCurrentRequest(generation, targetConversationId)) {
        isImporting = false;
      }
    }
  }

  function handleDialogOpenChange(nextOpen: boolean) {
    if (!nextOpen && open) onClose();
  }

  function handleWizardOpenChange(nextOpen: boolean) {
    if (!nextOpen && !isImporting) wizardOpen = false;
  }
</script>

<p class="sr-only" role="status" aria-live="polite">{announcement}</p>

<Dialog.Root {open} onOpenChange={handleDialogOpenChange}>
  <Dialog.Portal>
    <Dialog.Overlay
      class="z-50 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4"
      data-testid="group-members-overlay"
    />
    <DialogPrimitive.Content
      trapFocus={true}
      class="bg-background border border-border rounded-xl shadow-2xl w-[calc(100%-2rem)] max-w-4xl max-h-[90vh] flex flex-col p-0 overflow-hidden animate-in fade-in zoom-in-95 duration-200 fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 outline-none"
      data-testid="group-members-dialog"
    >
      <!-- Header -->
      <div class="p-4 border-b border-border flex items-center justify-between bg-muted/30" data-testid="group-members-header">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-full bg-primary/10 text-primary flex items-center justify-center shrink-0">
            <Users class="w-5 h-5" />
          </div>
          <div>
            <div class="flex items-center gap-2">
              <Dialog.Title level={2} class="font-semibold text-base leading-6 truncate max-w-md">{groupTitle}</Dialog.Title>
              {#if membershipComplete}
                <Badge variant="outline" class="bg-emerald-500/10 text-emerald-600 border-emerald-500/30 text-xs py-0.5 px-2 flex items-center gap-1">
                  <ShieldCheck class="w-3 h-3 text-emerald-500" />
                  <span>Danh sách đầy đủ</span>
                </Badge>
              {:else}
                <Badge variant="outline" class="bg-amber-500/10 text-amber-700 border-amber-500/30 text-xs py-0.5 px-2 flex items-center gap-1">
                  <ShieldAlert class="w-3 h-3 text-amber-600" />
                  <span>{totalCount} UID đã quan sát</span>
                </Badge>
              {/if}
              {#if diagnosticWarnings.length > 0}
                <Badge
                  variant="outline"
                  class="text-[10px] py-0 px-1.5"
                  title={diagnosticWarnings.map((warning) => warning.message).join('\n')}
                >
                  {diagnosticWarnings.length} cảnh báo
                </Badge>
              {/if}
            </div>
            <Dialog.Description
              class="text-xs leading-4 text-muted-foreground mt-0.5 max-w-2xl truncate"
              title={diagnosticSummary}
            >
              {diagnosticSummary}
            </Dialog.Description>
            {#each diagnosticWarnings as warning (`${warning.stage}:${warning.code}:${warning.message}`)}
              <span class="sr-only">{warning.message}</span>
            {/each}
          </div>
        </div>

        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            class="h-8 gap-1.5 text-xs border-emerald-500/30 text-emerald-600 hover:bg-emerald-500/10"
            disabled={isScanning || loading}
            onclick={triggerDeepScan}
          >
            {#if isScanning}
              <Loader2 class="w-3.5 h-3.5 animate-spin text-emerald-500" />
              <span>Đang Deep Scan...</span>
            {:else}
              <Scan class="w-3.5 h-3.5 text-emerald-500" />
              <span>Deep Scan UID</span>
            {/if}
          </Button>

          <Button
            variant="ghost"
            size="icon"
            class="h-8 w-8 rounded-full"
            aria-label="Đóng danh sách thành viên"
            onclick={onClose}
          >
            <X class="w-4 h-4" />
          </Button>
        </div>

      </div>

      <!-- Filters & Toolbar -->
      <div class="p-3 border-b border-border bg-background flex flex-wrap items-center justify-between gap-3" data-testid="group-members-toolbar">
        <!-- Tabs -->
        <div class="flex items-center gap-1 bg-muted p-1 rounded-lg text-xs">
          <button
            type="button"
            class="px-2.5 py-1 rounded-md transition-all font-medium {activeFilter === 'all' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => (activeFilter = 'all')}
          >
            Tất cả ({totalCount})
          </button>
          <button
            type="button"
            class="px-2.5 py-1 rounded-md transition-all font-medium {activeFilter === 'no_lead' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => (activeFilter = 'no_lead')}
          >
            Chưa có trong CRM ({nonLeadCount})
          </button>
          <button
            type="button"
            class="px-2.5 py-1 rounded-md transition-all font-medium {activeFilter === 'has_lead' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => (activeFilter = 'has_lead')}
          >
            Đã là Lead ({leadCount})
          </button>
          <button
            type="button"
            class="px-2.5 py-1 rounded-md transition-all font-medium {activeFilter === 'admin' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => (activeFilter = 'admin')}
          >
            Ban Quản Trị ({adminCount})
          </button>
        </div>

        <!-- Search input -->
        <div class="relative w-full sm:w-64">
          <Search class="w-3.5 h-3.5 absolute left-2.5 top-2.5 text-muted-foreground" />
          <Input
            type="text"
            bind:value={searchQuery}
            placeholder="Tìm tên, Zalo ID..."
            class="h-8 pl-8 text-xs w-full"
          />
        </div>
      </div>

      <!-- Member List Table -->
      <div class="flex-1 overflow-y-auto p-0 relative" data-testid="group-members-table-region">
        {#if loading}
          <div class="flex flex-col items-center justify-center p-12 text-muted-foreground gap-2">
            <Loader2 class="w-6 h-6 animate-spin text-primary" />
            <span class="text-xs">Đang tải các thành viên đã nhận diện...</span>
          </div>
        {:else if error}
          <div class="flex flex-col items-center justify-center p-12 text-destructive gap-2" role="alert" aria-live="assertive">
            <ShieldAlert class="w-8 h-8" />
            <span class="text-xs">{error}</span>
          </div>
        {:else if filteredMembers.length === 0}
          <div class="flex flex-col items-center justify-center p-12 text-muted-foreground gap-2">
            <Users class="w-8 h-8 opacity-40" />
            <span class="text-xs">Không tìm thấy thành viên phù hợp với bộ lọc.</span>
          </div>
        {:else}
          <table class="w-full text-left text-xs border-collapse">
            <thead class="bg-muted/50 sticky top-0 z-10 border-b border-border text-muted-foreground font-medium">
              <tr>
                <th class="p-3 w-10 text-center">
                  <Checkbox checked={isAllFilteredSelected} onCheckedChange={toggleSelectAllFiltered} />
                </th>
                <th class="p-3">Thành viên</th>
                <th class="p-3 w-28">Vai trò</th>
                <th class="p-3 w-56">Trạng thái CRM & Category</th>
                <th class="p-3 w-28 text-right">Zalo ID</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border/60">
              {#each filteredMembers as member (getMemberKey(member))}
                {@const key = getMemberKey(member)}
                {@const isSelected = selectedKeys.has(key)}
                <tr class="hover:bg-muted/30 transition-colors {isSelected ? 'bg-primary/5' : ''}">
                  <td class="p-3 text-center">
                    <Checkbox checked={isSelected} onCheckedChange={() => toggleMemberSelection(key)} />
                  </td>
                  <td class="p-3">
                    <div class="flex items-center gap-2.5">
                      {#if member.avatar_url}
                        <img
                          src={member.avatar_url}
                          alt={member.display_name}
                          class="w-7 h-7 rounded-full object-cover shrink-0"
                        />
                      {:else}
                        <div class="w-7 h-7 rounded-full bg-primary/10 text-primary font-semibold flex items-center justify-center shrink-0 text-[10px]">
                          {member.display_name.slice(0, 2).toUpperCase()}
                        </div>
                      {/if}
                      <span class="font-medium text-foreground text-xs truncate max-w-[200px]" title={member.display_name}>
                        {member.display_name}
                      </span>
                    </div>
                  </td>
                  <td class="p-3">
                    {#if member.participant_role === 'admin'}
                      <Badge variant="secondary" class="bg-amber-500/10 text-amber-600 border-amber-500/30 text-[10px] py-0 px-1.5 flex items-center gap-1 w-fit">
                        <Crown class="w-3 h-3 text-amber-500" />
                        <span>Trưởng nhóm</span>
                      </Badge>
                    {:else if member.participant_role === 'deputy'}
                      <Badge variant="secondary" class="bg-blue-500/10 text-blue-600 border-blue-500/30 text-[10px] py-0 px-1.5 w-fit">
                        Phó nhóm
                      </Badge>
                    {:else}
                      <span class="text-muted-foreground text-[11px]">Thành viên</span>
                    {/if}
                  </td>
                  <td class="p-3">
                    {#if member.lead_id}
                      <div class="flex items-center gap-1.5 flex-wrap">
                        <Badge variant="outline" class="bg-emerald-500/10 text-emerald-600 border-emerald-500/30 text-[10px] py-0 px-1.5 flex items-center gap-1">
                          <ShieldCheck class="w-3 h-3 text-emerald-500" />
                          <span>Đã là Lead</span>
                        </Badge>
                        {#each member.categories as cat}
                          <Badge variant="secondary" class="text-[10px] py-0 px-1.5">
                            {cat.name}
                          </Badge>
                        {/each}
                      </div>
                    {:else}
                      <span class="text-muted-foreground/70 text-[11px] italic">Chưa thêm vào CRM</span>
                    {/if}
                  </td>
                  <td class="p-3 text-right font-mono text-[11px] text-muted-foreground">
                    {member.external_id || '—'}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>

      <!-- Footer Action Bar -->
      <div class="p-3 border-t border-border bg-muted/30 flex items-center justify-between" data-testid="group-members-footer">
        <div class="text-xs text-muted-foreground flex items-center gap-2">
          <span>Đã chọn <strong class="text-foreground">{selectedKeys.size}</strong> / {totalCount} thành viên</span>
          {#if selectedKeys.size > 0}
            <Button
              variant="ghost"
              size="sm"
              class="h-6 text-[11px] text-muted-foreground hover:text-foreground px-1.5"
              onclick={() => (selectedKeys = new Set())}
            >
              Bỏ chọn tất cả
            </Button>
          {/if}
        </div>

        <div class="flex items-center gap-2">
          <Button variant="outline" size="sm" onclick={exportCSV} disabled={selectedKeys.size === 0} class="h-8 text-xs gap-1.5">
            <Download class="w-3.5 h-3.5" />
            <span>Xuất CSV</span>
          </Button>

          <Button size="sm" onclick={openImportWizard} disabled={selectedKeys.size === 0} class="h-8 text-xs gap-1.5 bg-primary text-primary-foreground">
            <UserPlus class="w-3.5 h-3.5" />
            <span>Thêm vào Leads & Categories ({selectedKeys.size})</span>
          </Button>
        </div>
      </div>
    </DialogPrimitive.Content>
  </Dialog.Portal>
</Dialog.Root>

<!-- Step 2: Import & Category Wizard Modal -->
<Dialog.Root open={wizardOpen} onOpenChange={handleWizardOpenChange}>
  <Dialog.Portal>
    <Dialog.Overlay
      class="z-60 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4"
      data-testid="group-members-wizard-overlay"
    />
    <DialogPrimitive.Content
      trapFocus={true}
      class="bg-background border border-border rounded-xl shadow-2xl w-[calc(100%-2rem)] max-w-md p-5 flex flex-col gap-4 animate-in fade-in zoom-in-95 duration-150 fixed left-1/2 top-1/2 z-60 -translate-x-1/2 -translate-y-1/2 outline-none"
      data-testid="group-members-wizard"
    >
      <div class="flex items-center justify-between border-b border-border pb-3">
        <div class="flex items-center gap-2">
          <div class="w-8 h-8 rounded-full bg-primary/10 text-primary flex items-center justify-center shrink-0">
            <Tag class="w-4 h-4" />
          </div>
          <div>
            <Dialog.Title level={3} class="font-semibold text-sm leading-5">Cấu Hình Import Lead & Category</Dialog.Title>
            <Dialog.Description class="text-xs leading-4 text-muted-foreground">Thêm {selectedKeys.size} thành viên nhóm vào CRM</Dialog.Description>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7"
          aria-label="Đóng cấu hình import"
          onclick={() => (wizardOpen = false)}
        >
          <X class="w-3.5 h-3.5" />
        </Button>
      </div>

      {#if importSuccessMessage}
        <div class="p-3 bg-emerald-500/10 border border-emerald-500/30 rounded-lg text-emerald-600 text-xs flex items-center gap-2">
          <CheckCircle2 class="w-5 h-5 text-emerald-500 shrink-0" />
          <span>{importSuccessMessage}</span>
        </div>
      {:else}
        <!-- New Category Input -->
        <div class="space-y-1.5">
          <label for="new-group-category" class="text-xs font-medium text-foreground flex items-center gap-1.5 font-sans">
            Tạo Category mới cho nhóm này:
          </label>
          <Input
            id="new-group-category"
            type="text"
            bind:value={newCategoryName}
            placeholder="VD: Lead Nhóm - Vibe Coding"
            class="h-8 text-xs"
          />
        </div>

        <!-- Existing Categories Selection -->
        {#if existingCategories.length > 0}
          <div class="space-y-1.5">
            <p class="text-xs font-medium text-foreground">
              Hoặc chọn Categories sẵn có trong CRM:
            </p>
            <div class="max-h-36 overflow-y-auto border border-border rounded-md p-2 space-y-1.5 bg-muted/20">
              {#each existingCategories as cat (cat.id)}
                <label class="flex items-center gap-2 text-xs cursor-pointer hover:bg-muted/40 p-1 rounded transition-colors">
                  <Checkbox
                    checked={selectedCategoryIds.has(cat.id)}
                    onCheckedChange={() => toggleCategorySelection(cat.id)}
                  />
                  <span>{cat.name}</span>
                </label>
              {/each}
            </div>
          </div>
        {/if}

        <div class="p-2.5 bg-muted/40 rounded-lg text-[11px] text-muted-foreground space-y-1">
          <p class="font-medium text-foreground">ℹ️ Quy tắc tự động:</p>
          <ul class="list-disc pl-4 space-y-0.5">
            <li>Những ai <strong>chưa là Lead</strong> sẽ được tự động chèn mới vào CRM.</li>
            <li>Những ai <strong>đã là Lead</strong> sẽ tự động được gán thêm vào các Category vừa chọn.</li>
            <li>Ghi chú Lead tự động lưu vết nguồn: <code>Zalo Group Lead: {groupTitle}</code>.</li>
          </ul>
        </div>

        <div class="flex items-center justify-end gap-2 pt-2 border-t border-border">
          <Button variant="outline" size="sm" onclick={() => (wizardOpen = false)} class="h-8 text-xs">
            Hủy
          </Button>
          <Button
            size="sm"
            onclick={executeBatchImport}
            disabled={isImporting}
            class="h-8 text-xs gap-1.5 bg-primary text-primary-foreground"
          >
            {#if isImporting}
              <Loader2 class="w-3.5 h-3.5 animate-spin" />
              <span>Đang xử lý...</span>
            {:else}
              <Check class="w-3.5 h-3.5" />
              <span>Xác Nhận Import</span>
            {/if}
          </Button>
        </div>
      {/if}
    </DialogPrimitive.Content>
  </Dialog.Portal>
</Dialog.Root>
