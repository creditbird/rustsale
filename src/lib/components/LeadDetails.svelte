<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { toast } from 'svelte-sonner';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Activity, Deal } from '$lib/services/crmService';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import * as Dialog from '$lib/components/ui/dialog';
  import {
    X, Edit2, Trash2, Save, Phone, Mail, Users, CheckCircle2, XCircle,
    AlertCircle, Calendar, Clock, MapPin, Building, Briefcase, Plus,
    DollarSign, Layers, Send, MessageCircle
  } from 'lucide-svelte';

  let { leadId, onClose, onRefresh } = $props<{ leadId: number; onClose: () => void; onRefresh?: () => void }>();

  let lead = $state<Lead | null>(null);
  let activities = $state<Activity[]>([]);
  let deals = $state<Deal[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  // Edit mode states
  let isEditing = $state(false);
  let editName = $state('');
  let editCompany = $state('');
  let editStatus = $state('');
  let editValue = $state(0);
  let editOwner = $state('');
  let editEmail = $state('');
  let editPhone = $state('');
  let editAbout = $state('');
  let editAddress = $state('');
  let savingLead = $state(false);
  let deletingLead = $state(false);

  // Create Deal Modal State for this lead
  let showCreateDealModal = $state(false);
  let newDealTitle = $state('');
  let newDealValue = $state(10000);
  let newDealStage = $state('Prospecting');
  let creatingDeal = $state(false);
  let createDealErrorMsg = $state<string | null>(null);

  const presetValues = [5000, 10000, 25000, 50000, 100000];

  const stages = ['Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won'];
  const stageBadges: Record<string, { label: string; bg: string; text: string }> = {
    'Prospecting': { label: 'Prospecting (20%)', bg: 'bg-blue-500/10 border-blue-500/30', text: 'text-blue-600' },
    'Demo': { label: 'Demo (40%)', bg: 'bg-purple-500/10 border-purple-500/30', text: 'text-purple-600' },
    'Proposal': { label: 'Proposal (60%)', bg: 'bg-amber-500/10 border-amber-500/30', text: 'text-amber-600' },
    'Negotiation': { label: 'Negotiation (80%)', bg: 'bg-indigo-500/10 border-indigo-500/30', text: 'text-indigo-600' },
    'Won': { label: 'Won (100%)', bg: 'bg-emerald-500/10 border-emerald-500/30', text: 'text-emerald-600' }
  };

  function startEditing() {
    if (!lead) return;
    editName = lead.name;
    editCompany = lead.company;
    editStatus = lead.status;
    editValue = lead.value;
    editOwner = lead.owner || '';
    editEmail = lead.email;
    editPhone = lead.phone || '';
    editAbout = lead.about || '';
    editAddress = lead.address || '';
    isEditing = true;
  }

  function openCreateDealModal() {
    createDealErrorMsg = null;
    newDealTitle = `Deal - ${lead?.company || lead?.name || ''}`;
    newDealValue = 10000;
    newDealStage = 'Prospecting';
    showCreateDealModal = true;
  }

  async function handleCreateDeal(e: Event) {
    e.preventDefault();
    if (!newDealTitle.trim()) {
      createDealErrorMsg = 'Vui lòng nhập tên Deal.';
      return;
    }

    creatingDeal = true;
    createDealErrorMsg = null;
    try {
      const createdTitle = newDealTitle.trim();
      await crmService.createDeal(leadId, createdTitle, Number(newDealValue) || 0, newDealStage);
      showCreateDealModal = false;
      toast.success('Tạo Deal mới thành công!', {
        description: `Đã tạo cơ hội kinh doanh "${createdTitle}" cho ${lead?.name || 'khách hàng'}.`
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      createDealErrorMsg = err instanceof Error ? err.message : String(err);
      toast.error('Tạo Deal thất bại', {
        description: createDealErrorMsg
      });
    } finally {
      creatingDeal = false;
    }
  }

  // Delete Deal Modal State
  let showDeleteDealModal = $state(false);
  let dealToDelete = $state<{ id: number; title: string } | null>(null);
  let deletingDeal = $state(false);

  function promptDeleteDeal(id: number, title: string) {
    dealToDelete = { id, title };
    showDeleteDealModal = true;
  }

  async function confirmDeleteDeal() {
    if (!dealToDelete) return;
    deletingDeal = true;
    const deletedTitle = dealToDelete.title;
    try {
      await crmService.deleteDeal(dealToDelete.id);
      showDeleteDealModal = false;
      dealToDelete = null;
      toast.success('Xóa Deal thành công!', {
        description: `Đã xóa cơ hội kinh doanh "${deletedTitle}".`
      });
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
      toast.error('Xóa Deal thất bại', {
        description: errorMsg
      });
    } finally {
      deletingDeal = false;
    }
  }

  async function handleSaveLead(e: Event) {
    e.preventDefault();
    if (!lead || !editName.trim() || !editCompany.trim() || !editEmail.trim()) return;

    savingLead = true;
    errorMsg = null;
    try {
      await crmService.updateLead(
        leadId,
        editName.trim(),
        editCompany.trim(),
        editStatus,
        editValue,
        editOwner.trim(),
        editEmail.trim(),
        editPhone.trim(),
        editAbout.trim() || null,
        editAddress.trim() || null
      );
      isEditing = false;
      await loadLeadDetails(leadId);
      if (onRefresh) onRefresh();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      savingLead = false;
    }
  }

  let isDeleteLeadModalOpen = $state(false);

  function promptDeleteLead() {
    isDeleteLeadModalOpen = true;
  }

  async function confirmDeleteLead() {
    if (!lead) return;
    deletingLead = true;
    errorMsg = null;
    try {
      await crmService.deleteLead(leadId);
      isDeleteLeadModalOpen = false;
      if (onRefresh) onRefresh();
      onClose();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      deletingLead = false;
    }
  }

  // Form states
  let activityType = $state('Call');
  let activityContent = $state('');
  let logging = $state(false);

  $effect(() => {
    if (leadId) loadLeadDetails(leadId);
  });

  async function loadLeadDetails(id: number) {
    loading = true;
    errorMsg = null;
    try {
      const [leadData, activitiesData, allDealsData] = await Promise.all([
        crmService.getLeadDetails(id),
        crmService.getActivities(id),
        crmService.getDeals()
      ]);
      if (id !== leadId) return;
      lead = leadData;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
      deals = allDealsData.filter(d => d.lead_id === id);
    } catch (err) {
      if (id === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (id === leadId) loading = false;
    }
  }

  async function handleLogActivity(e: Event) {
    e.preventDefault();
    if (!activityContent.trim()) return;

    logging = true;
    errorMsg = null;
    const currentLeadId = leadId;
    try {
      const performer = lead?.owner || 'Self';
      await crmService.addActivity(currentLeadId, activityType, activityContent.trim(), performer);
      if (currentLeadId !== leadId) return;
      activityContent = '';
      const activitiesData = await crmService.getActivities(currentLeadId);
      if (currentLeadId !== leadId) return;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    } catch (err) {
      if (currentLeadId === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (currentLeadId === leadId) logging = false;
    }
  }

  function formatDateTime(dateStr: string) {
    if (!dateStr) return '';
    try {
      const date = new Date(dateStr);
      if (isNaN(date.getTime())) return dateStr;
      return date.toLocaleString('vi-VN', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      });
    } catch (e) {
      return dateStr;
    }
  }
</script>

<!-- Backdrop overlay -->
<div
  class="fixed inset-0 z-50 bg-background/80 backdrop-blur-xs"
  transition:fade={{ duration: 150 }}
  onclick={onClose}
  role="presentation"
></div>

<!-- Drawer container -->
<aside
  class="fixed top-0 right-0 bottom-0 z-50 w-full max-w-2xl bg-card border-l border-border shadow-2xl flex flex-col overflow-hidden text-card-foreground"
  transition:fly={{ x: 640, duration: 250 }}
>
  <!-- Drawer Header -->
  <header class="h-14 px-6 border-b border-border flex items-center justify-between shrink-0 bg-muted/20">
    <div class="flex items-center gap-2">
      <h3 class="text-sm font-bold text-foreground tracking-wide">Chi Tiết Hồ Sơ Khách Hàng</h3>
      {#if lead}
        <Badge variant="outline" class="text-[10px] uppercase font-mono">{lead.status}</Badge>
      {/if}
    </div>
    <Button variant="ghost" size="sm" onclick={onClose} class="h-8 w-8 p-0 text-muted-foreground hover:text-foreground">
      <X class="w-4 h-4" />
    </Button>
  </header>

  {#if loading && !lead}
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-muted-foreground text-xs gap-2">
      <div class="w-5 h-5 rounded-full border-2 border-primary border-t-transparent animate-spin"></div>
      <p>Đang tải chi tiết khách hàng...</p>
    </div>
  {:else if errorMsg && !lead}
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-destructive text-xs gap-2">
      <AlertCircle class="w-6 h-6" />
      <p>{errorMsg}</p>
    </div>
  {:else if lead}
    <div class="flex-1 overflow-y-auto p-6 space-y-6">
      {#if errorMsg}
        <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="w-4 h-4 shrink-0" />
          <span>{errorMsg}</span>
        </div>
      {/if}

      <!-- Profile Summary Card -->
      <div class="p-4 rounded-xl border border-border bg-card shadow-xs relative space-y-4">
        <div class="flex items-start gap-4">
          <div class="w-14 h-14 rounded-full bg-emerald-500/10 text-emerald-600 font-bold text-base flex items-center justify-center border border-emerald-500/20 shrink-0 shadow-2xs">
            {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
          </div>
          <div class="flex-1 min-w-0 pr-12">
            <h4 class="text-lg font-bold text-foreground truncate">{lead.name}</h4>
            <p class="text-xs text-muted-foreground font-medium">{lead.company}</p>
            <div class="flex flex-wrap gap-2 mt-2.5">
              <Badge variant="default" class="text-[10px] py-0.5 px-2.5">{lead.status}</Badge>
              {#if lead.has_zalo === 1}
                <Badge variant="outline" class="gap-1 border-sky-500/30 text-sky-600 bg-sky-500/10 text-[10px] py-0.5 px-2 font-semibold">
                  <CheckCircle2 class="w-3 h-3 text-sky-500" /> Zalo Verified
                </Badge>
              {:else if lead.has_zalo === 0}
                <Badge variant="outline" class="gap-1 border-slate-300 text-slate-500 bg-slate-100 text-[10px] py-0.5 px-2">
                  <XCircle class="w-3 h-3 text-slate-400" /> No Zalo
                </Badge>
              {/if}
            </div>
          </div>
          {#if !isEditing}
            <Button variant="outline" size="sm" onclick={startEditing} class="h-8 gap-1.5 absolute top-4 right-4 text-xs shadow-2xs">
              <Edit2 class="w-3.5 h-3.5" />
              Sửa
            </Button>
          {/if}
        </div>

        <!-- CRM Action Bar: Quick Deal Creation & Outreach Actions -->
        {#if !isEditing}
          <div class="pt-3 border-t border-border/60 flex items-center gap-2 flex-wrap">
            <Button
              size="sm"
              onclick={openCreateDealModal}
              class="gap-1.5 font-bold text-xs bg-emerald-600 hover:bg-emerald-700 text-white shadow-sm h-8 rounded-lg"
            >
              <Briefcase class="w-3.5 h-3.5" />
              <span>+ Tạo Deal Mới</span>
            </Button>

            <a
              href="/#zalo-outreach"
              class="inline-flex items-center gap-1.5 px-3 h-8 rounded-lg border border-sky-500/30 bg-sky-500/10 text-sky-600 hover:bg-sky-500/20 text-xs font-semibold transition-colors"
            >
              <MessageCircle class="w-3.5 h-3.5" />
              <span>Gửi Zalo</span>
            </a>

            <a
              href="mailto:{lead.email}"
              class="inline-flex items-center gap-1.5 px-3 h-8 rounded-lg border border-border bg-muted/40 hover:bg-muted text-foreground text-xs font-semibold transition-colors"
            >
              <Mail class="w-3.5 h-3.5" />
              <span>Gửi Email</span>
            </a>
          </div>
        {/if}
      </div>

      <!-- Deals List Section (Cơ Hội Bán Hàng) -->
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider flex items-center gap-1.5">
            <Briefcase class="w-3.5 h-3.5 text-emerald-600" />
            Cơ Hội Kinh Doanh (Deals) ({deals.length})
          </h5>
        </div>

        {#if deals.length === 0}
          <div class="p-4 rounded-xl border border-dashed border-border bg-card/50 text-center py-5">
            <p class="text-xs text-muted-foreground">Chưa có Deal / Hợp đồng nào cho khách hàng này.</p>
          </div>
        {:else}
          <div class="space-y-2.5">
            {#each deals as deal}
              {@const badge = stageBadges[deal.stage] || { label: deal.stage, bg: 'bg-muted', text: 'text-foreground' }}
              <div class="p-3.5 rounded-xl border border-border bg-card shadow-2xs hover:border-emerald-500/40 transition-colors flex items-center justify-between gap-3">
                <div class="min-w-0 space-y-1">
                  <div class="flex items-center gap-2">
                    <h6 class="text-xs font-bold text-foreground truncate">{deal.title}</h6>
                    <Badge variant="outline" class="text-[10px] font-semibold px-2 py-0.5 {badge.bg} {badge.text}">
                      {badge.label}
                    </Badge>
                  </div>
                  <div class="flex items-center gap-3 text-[11px]">
                    <span class="font-extrabold text-emerald-600 text-xs">${deal.value.toLocaleString()}</span>
                    {#if deal.created_at}
                      <span class="text-muted-foreground text-[10px] flex items-center gap-1 font-medium">
                        <Calendar class="w-3 h-3 text-muted-foreground/60" />
                        {new Date(deal.created_at).toLocaleDateString('vi-VN', { month: 'numeric', day: 'numeric' })}
                      </span>
                    {/if}
                  </div>
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  onclick={() => promptDeleteDeal(deal.id!, deal.title)}
                  class="h-7 w-7 p-0 text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-lg shrink-0"
                  title="Xóa deal này"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </Button>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      {#if isEditing}
        <!-- Edit Lead Form (2-column responsive layout) -->
        <form onsubmit={handleSaveLead} class="space-y-4 text-xs border border-border bg-card p-5 rounded-xl shadow-xs">
          <div class="flex items-center justify-between border-b border-border pb-3">
            <h5 class="text-xs font-bold text-foreground uppercase tracking-wider">Chỉnh Sửa Thông Tin Khách Hàng</h5>
            <span class="text-[10px] text-muted-foreground">* Trường bắt buộc</span>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1">
              <label for="edit-name" class="font-medium text-foreground">Họ và Tên *</label>
              <Input id="edit-name" bind:value={editName} required class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-company" class="font-medium text-foreground">Tên Công Ty / Đơn Vị *</label>
              <Input id="edit-company" bind:value={editCompany} required class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-email" class="font-medium text-foreground">Email *</label>
              <Input id="edit-email" type="email" bind:value={editEmail} required class="h-9 text-xs" />
            </div>
            <div class="space-y-1">
              <label for="edit-phone" class="font-medium text-foreground">Số Điện Thoại</label>
              <Input id="edit-phone" bind:value={editPhone} class="h-9 text-xs" />
            </div>

            <div class="space-y-1">
              <label for="edit-status" class="font-medium text-foreground">Trạng Thái</label>
              <select
                id="edit-status"
                bind:value={editStatus}
                class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
              >
                <option value="New">New</option>
                <option value="Contacted">Contacted</option>
                <option value="Qualified">Qualified</option>
                <option value="Lost">Lost</option>
                <option value="Converted">Converted (Contact)</option>
              </select>
            </div>
            <div class="space-y-1">
              <label for="edit-value" class="font-medium text-foreground">Giá Trị Dự Kiến ($)</label>
              <Input id="edit-value" type="number" bind:value={editValue} min="0" class="h-9 text-xs" />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-owner" class="font-medium text-foreground">Người Phụ Trách</label>
              <Input id="edit-owner" bind:value={editOwner} class="h-9 text-xs" />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-address" class="font-medium text-foreground">Địa Chỉ</label>
              <Input id="edit-address" bind:value={editAddress} class="h-9 text-xs" placeholder="Địa chỉ chi tiết..." />
            </div>

            <div class="space-y-1 sm:col-span-2">
              <label for="edit-about" class="font-medium text-foreground">Mô Tả / Ghi Chú</label>
              <textarea id="edit-about" bind:value={editAbout} rows="3" class="w-full p-3 rounded-md border border-input bg-background text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"></textarea>
            </div>
          </div>

          <div class="flex items-center justify-between pt-4 border-t border-border">
            <Button type="button" variant="destructive" size="sm" onclick={promptDeleteLead} disabled={deletingLead} class="gap-1.5 h-9 px-4">
              <Trash2 class="w-3.5 h-3.5" />
              {deletingLead ? 'Đang Xóa...' : 'Xóa Lead'}
            </Button>
            <div class="flex items-center gap-2">
              <Button type="button" variant="outline" size="sm" onclick={() => isEditing = false} class="h-9 px-4">Hủy Bỏ</Button>
              <Button type="submit" size="sm" disabled={savingLead} class="gap-1.5 h-9 px-5">
                <Save class="w-3.5 h-3.5" />
                {savingLead ? 'Đang Lưu...' : 'Lưu Thay Đổi'}
              </Button>
            </div>
          </div>
        </form>
      {:else}
        <!-- Contact Details Overview -->
        <div class="space-y-3">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Tổng Quan Liên Hệ</h5>
          <div class="rounded-xl border border-border bg-card p-4 space-y-3 text-xs shadow-xs">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Mail class="w-3.5 h-3.5 text-primary" /> Email</span>
                <a href="mailto:{lead.email}" class="text-primary font-semibold hover:underline break-all">{lead.email}</a>
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Phone class="w-3.5 h-3.5 text-primary" /> Số Điện Thoại</span>
                <span class="text-foreground font-semibold">{lead.phone || '—'}</span>
              </div>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-3 border-b border-border/60">
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Users class="w-3.5 h-3.5 text-primary" /> Phụ Trách</span>
                <span class="text-foreground font-semibold">{lead.owner || '—'}</span>
              </div>
              <div>
                <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><Building class="w-3.5 h-3.5 text-primary" /> Giá Trị Dự Kiến</span>
                <span class="text-emerald-600 font-bold text-sm">${(lead.value || 0).toLocaleString()}</span>
              </div>
            </div>

            <div class="pb-1">
              <span class="text-muted-foreground flex items-center gap-1.5 mb-1 font-medium"><MapPin class="w-3.5 h-3.5 text-primary" /> Địa Chỉ</span>
              <span class="text-foreground font-medium leading-relaxed break-words block bg-muted/30 p-2.5 rounded-lg border border-border/50">{lead.address || '—'}</span>
            </div>

            {#if lead.about}
              <div class="pt-2">
                <span class="text-muted-foreground block mb-1 font-medium">Giới Thiệu / Ghi Chú</span>
                <p class="p-3 rounded-lg bg-muted/30 border border-border/60 text-foreground leading-relaxed break-words">{lead.about}</p>
              </div>
            {/if}
          </div>
        </div>

        <!-- Log Activity Form -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Ghi Nhận Hoạt Động (Log Activity)</h5>
          <form onsubmit={handleLogActivity} class="space-y-3 text-xs rounded-xl border border-border bg-card p-4 shadow-xs">
            <select
              bind:value={activityType}
              class="w-full h-9 text-xs border border-input bg-background rounded-md px-3 py-1 text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              <option value="Call">Call (Cuộc gọi)</option>
              <option value="Email">Email</option>
              <option value="Meeting">Meeting (Cuộc họp)</option>
            </select>
            <textarea
              bind:value={activityContent}
              placeholder="Nhập nội dung trao đổi, ghi chú cuộc gọi/cuộc họp..."
              rows="3"
              required
              class="w-full p-3 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground text-xs leading-relaxed focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"
            ></textarea>
            <Button type="submit" size="sm" class="w-full h-9 text-xs font-medium" disabled={logging || !activityContent.trim()}>
              {logging ? 'Đang Ghi Nhận...' : 'Ghi Nhận Hoạt Động'}
            </Button>
          </form>
        </div>

        <!-- Activity Timeline -->
        <div class="space-y-3 pt-2">
          <h5 class="text-xs font-bold text-muted-foreground uppercase tracking-wider">Lịch Sử Hoạt Động</h5>
          {#if activities.length === 0}
            <p class="text-xs text-muted-foreground italic p-4 rounded-xl border border-border bg-card text-center">Chưa có hoạt động nào được ghi nhận.</p>
          {:else}
            <div class="space-y-3 relative before:absolute before:left-3 before:top-2 before:bottom-2 before:w-0.5 before:bg-border">
              {#each activities as activity}
                <div class="flex items-start gap-3 relative">
                  <div class="w-6.5 h-6.5 rounded-full bg-background border border-border flex items-center justify-center text-primary z-10 text-[10px] shrink-0">
                    <Clock class="w-3 h-3 text-muted-foreground" />
                  </div>
                  <div class="flex-1 p-3 rounded-xl border border-border/70 bg-card text-xs space-y-1.5 shadow-2xs">
                    <div class="flex items-center justify-between font-semibold">
                      <span class="text-foreground">{activity.activity_type}</span>
                      <span class="text-[10px] text-muted-foreground font-normal">{formatDateTime(activity.created_at)}</span>
                    </div>
                    <p class="text-muted-foreground leading-relaxed break-words">{activity.content}</p>
                    <span class="text-[10px] text-muted-foreground/70 block">Ghi bởi {activity.performed_by}</span>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</aside>

<!-- Create Deal Modal for this Specific Lead -->
{#if showCreateDealModal && lead}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-150">

      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between bg-muted/40">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-xl bg-emerald-500/10 text-emerald-600 flex items-center justify-center shadow-2xs">
            <Briefcase class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-base text-foreground">Tạo Deal Mới Cho Khách Hàng</h3>
            <p class="text-xs text-muted-foreground">{lead.name} — {lead.company}</p>
          </div>
        </div>
        <button
          type="button"
          onclick={() => showCreateDealModal = false}
          class="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-accent transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <form onsubmit={handleCreateDeal} class="p-6 space-y-4">
        {#if createDealErrorMsg}
          <div class="p-3 rounded-xl bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span>{createDealErrorMsg}</span>
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="lead-deal-title" class="text-xs font-bold text-foreground">Tên Deal / Hợp Đồng *</label>
          <Input
            id="lead-deal-title"
            bind:value={newDealTitle}
            placeholder="Ví dụ: Deal - {lead.company}"
            class="h-10 text-xs rounded-xl"
            required
          />
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <label for="lead-deal-value" class="text-xs font-bold text-foreground">Giá Trị Hợp Đồng ($) *</label>
            <Input
              id="lead-deal-value"
              type="number"
              bind:value={newDealValue}
              placeholder="10000"
              class="h-10 text-xs rounded-xl font-bold text-emerald-600"
              min="0"
              step="500"
              required
            />

            <!-- Quick Value Presets -->
            <div class="flex items-center gap-1 pt-1 flex-wrap">
              {#each presetValues as preset}
                <button
                  type="button"
                  onclick={() => newDealValue = preset}
                  class="text-[10px] font-semibold px-2 py-0.5 rounded-lg border border-border hover:border-emerald-500 hover:bg-emerald-500/10 hover:text-emerald-600 transition-colors"
                >
                  ${preset >= 1000 ? `${preset/1000}k` : preset}
                </button>
              {/each}
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="lead-deal-stage" class="text-xs font-bold text-foreground">Giai Đoạn Ban Đầu</label>
            <select
              id="lead-deal-stage"
              bind:value={newDealStage}
              class="w-full h-10 px-3 rounded-xl border border-input bg-background text-xs font-semibold text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
            >
              {#each stages as stage}
                <option value={stage}>{stageBadges[stage]?.label || stage}</option>
              {/each}
            </select>
          </div>
        </div>

        <div class="pt-4 border-t border-border flex items-center justify-end gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => showCreateDealModal = false}
            disabled={creatingDeal}
            class="rounded-xl h-10 px-5 text-xs font-semibold"
          >
            Hủy Bỏ
          </Button>
          <Button
            type="submit"
            size="default"
            disabled={creatingDeal}
            class="rounded-xl h-10 px-6 text-xs font-bold bg-emerald-600 text-white hover:bg-emerald-700 shadow-md"
          >
            {creatingDeal ? 'Đang Tạo...' : 'Tạo Deal Mới'}
          </Button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Delete Lead Confirmation Dialog -->
<Dialog.Root bind:open={isDeleteLeadModalOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Xóa Hồ Sơ Khách Hàng</Dialog.Title>
    </Dialog.Header>
    <div class="py-2 text-xs text-muted-foreground leading-relaxed">
      Bạn có chắc chắn muốn xóa khách hàng <strong class="text-foreground">{lead?.name}</strong>?
      Hành động này sẽ xóa khách hàng và toàn bộ lịch sử hoạt động liên quan vĩnh viễn.
    </div>
    <Dialog.Footer class="gap-2">
      <Button variant="outline" size="sm" onclick={() => isDeleteLeadModalOpen = false} disabled={deletingLead}>Hủy Bỏ</Button>
      <Button variant="destructive" size="sm" onclick={confirmDeleteLead} disabled={deletingLead}>
        {deletingLead ? 'Đang Xóa...' : 'Xóa Khách Hàng'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Custom Styled Delete Deal Confirmation Modal -->
{#if showDeleteDealModal && dealToDelete}
  <div class="fixed inset-0 z-[60] bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in-95 duration-150">
      <div class="p-6 text-center space-y-4">
        <div class="w-12 h-12 rounded-full bg-destructive/10 text-destructive flex items-center justify-center mx-auto shadow-2xs">
          <AlertCircle class="w-6 h-6" />
        </div>

        <div class="space-y-1.5">
          <h3 class="font-bold text-base text-foreground">Xác Nhận Xóa Deal</h3>
          <p class="text-xs text-muted-foreground leading-relaxed">
            Bạn có chắc chắn muốn xóa cơ hội kinh doanh <span class="font-bold text-foreground">"{dealToDelete.title}"</span>? Hành động này không thể hoàn tác.
          </p>
        </div>

        <div class="pt-2 flex items-center justify-center gap-3">
          <Button
            type="button"
            variant="outline"
            size="default"
            onclick={() => { showDeleteDealModal = false; dealToDelete = null; }}
            disabled={deletingDeal}
            class="rounded-xl h-10 px-5 text-xs font-semibold"
          >
            Hủy Bỏ
          </Button>
          <Button
            type="button"
            variant="destructive"
            size="default"
            onclick={confirmDeleteDeal}
            disabled={deletingDeal}
            class="rounded-xl h-10 px-6 text-xs font-bold shadow-md bg-destructive text-destructive-foreground hover:bg-destructive/90"
          >
            {deletingDeal ? 'Đang Xóa...' : 'Xóa Deal'}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}
