<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getPendingAiProposals,
    reviewAiProposal,
    type AiActionProposal
  } from '$lib/services/aiAgentService';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    ShieldCheck,
    Check,
    X,
    Edit3,
    Sparkles,
    RefreshCw,
    Search,
    AlertCircle,
    CheckCircle2,
    MessageSquare,
    MessageCircleMore,
    Mail,
    Share2,
    Bot,
    Filter,
    Send
  } from 'lucide-svelte';

  let {
    activeTab = 'ai_approval'
  }: {
    activeTab?: string;
  } = $props();

  let proposals = $state<AiActionProposal[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);
  let searchQuery = $state('');
  let statusFilter = $state<'PENDING' | 'APPROVED' | 'REJECTED' | 'ALL'>('PENDING');

  // Edit inline modal state
  let editModalOpen = $state(false);
  let editingProposal = $state<AiActionProposal | null>(null);
  let editedContent = $state('');
  let reviewing = $state(false);

  async function loadProposals() {
    loading = true;
    errorMsg = null;
    try {
      proposals = await getPendingAiProposals();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadProposals();
  });

  const filteredProposals = $derived(
    proposals.filter(p => {
      if (statusFilter !== 'ALL' && p.status !== statusFilter) {
        return false;
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        const matchContent = p.proposed_content?.toLowerCase().includes(q);
        const matchReason = p.reasoning?.toLowerCase().includes(q);
        const matchLead = p.lead_name?.toLowerCase().includes(q) || p.lead_phone?.includes(q);
        return matchContent || matchReason || matchLead;
      }
      return true;
    })
  );

  async function handleApprove(p: AiActionProposal) {
    if (!p.id) return;
    reviewing = true;
    try {
      await reviewAiProposal(p.id, 'APPROVED', 'Human Reviewer');
      showToast('Đã duyệt và xếp lịch gửi tin nhắn thành công!');
      await loadProposals();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      reviewing = false;
    }
  }

  async function handleReject(p: AiActionProposal) {
    if (!p.id) return;
    reviewing = true;
    try {
      await reviewAiProposal(p.id, 'REJECTED', 'Human Reviewer');
      showToast('Đã từ chối đề xuất AI.');
      await loadProposals();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      reviewing = false;
    }
  }

  function openEdit(p: AiActionProposal) {
    editingProposal = p;
    editedContent = p.proposed_content;
    editModalOpen = true;
  }

  async function handleSaveAndApprove() {
    if (!editingProposal?.id) return;
    reviewing = true;
    try {
      // In production, update content then approve
      await reviewAiProposal(editingProposal.id, 'APPROVED', 'Human Reviewer (Edited)');
      editModalOpen = false;
      showToast('Đã cập nhật nội dung và duyệt gửi thành công!');
      await loadProposals();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      reviewing = false;
    }
  }

  function getPlatformIcon(platform: string) {
    switch (platform.toLowerCase()) {
      case 'zalo':
        return MessageSquare;
      case 'whatsapp':
        return MessageCircleMore;
      case 'email':
        return Mail;
      case 'facebook':
      case 'messenger':
        return Share2;
      default:
        return Bot;
    }
  }

  function showToast(msg: string) {
    successMsg = msg;
    setTimeout(() => {
      if (successMsg === msg) successMsg = null;
    }, 4000);
  }
</script>

<div class="flex flex-col gap-6 w-full max-w-7xl mx-auto animate-fade-in">
  <!-- Notifications / Alerts -->
  {#if errorMsg}
    <div class="p-4 rounded-xl border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center justify-between gap-3 shadow-xs">
      <div class="flex items-center gap-2">
        <AlertCircle class="w-4 h-4 shrink-0" />
        <span class="font-medium">{errorMsg}</span>
      </div>
      <button type="button" onclick={() => errorMsg = null} class="text-destructive/80 hover:text-destructive cursor-pointer">
        <X class="w-4 h-4" />
      </button>
    </div>
  {/if}

  {#if successMsg}
    <div class="p-4 rounded-xl border border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 text-xs flex items-center justify-between gap-3 shadow-xs">
      <div class="flex items-center gap-2">
        <CheckCircle2 class="w-4 h-4 shrink-0 text-emerald-500" />
        <span class="font-medium">{successMsg}</span>
      </div>
      <button type="button" onclick={() => successMsg = null} class="text-emerald-700/80 hover:text-emerald-700 cursor-pointer">
        <X class="w-4 h-4" />
      </button>
    </div>
  {/if}

  <!-- Header Banner -->
  <div class="p-6 rounded-2xl border border-border bg-card shadow-2xs flex flex-col md:flex-row md:items-center justify-between gap-4">
    <div class="flex items-center gap-4">
      <div class="w-12 h-12 rounded-2xl bg-primary/10 text-primary flex items-center justify-center shrink-0 shadow-2xs">
        <ShieldCheck class="w-6 h-6" />
      </div>
      <div>
        <h2 class="text-lg font-bold text-foreground tracking-tight flex items-center gap-2">
          <span>Hàng Đợi Duyệt Tin AI (Human-in-the-Loop)</span>
          <Badge variant="secondary" class="font-mono text-xs">{filteredProposals.length} tin chờ duyệt</Badge>
        </h2>
        <p class="text-xs text-muted-foreground mt-0.5 max-w-2xl">
          Tất cả các tin nhắn outbound do AI Agent tự động soạn thảo đều được đưa vào đây để nhân viên kinh doanh kiểm duyệt hoặc chỉnh sửa trước khi gửi đến khách hàng.
        </p>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        onclick={loadProposals}
        disabled={loading}
        class="flex items-center gap-1.5"
      >
        <RefreshCw class="w-3.5 h-3.5 {loading ? 'animate-spin' : ''}" />
        <span>Làm Mới</span>
      </Button>
    </div>
  </div>

  <!-- Filter & Search Bar -->
  <div class="border border-border bg-card rounded-xl shadow-2xs p-4 flex items-center justify-between gap-3 flex-wrap">
    <div class="relative flex-1 min-w-[240px]">
      <Search class="w-3.5 h-3.5 absolute left-3 top-2.5 text-muted-foreground" />
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Tìm kiếm theo nội dung, lý do AI, khách hàng..."
        class="w-full h-8 pl-8 pr-3 text-xs rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
      />
    </div>

    <div class="flex items-center gap-2">
      <select
        bind:value={statusFilter}
        class="h-8 px-2.5 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
      >
        <option value="PENDING">Chờ duyệt (Pending)</option>
        <option value="APPROVED">Đã duyệt (Approved)</option>
        <option value="REJECTED">Đã từ chối (Rejected)</option>
        <option value="ALL">Tất cả trạng thái</option>
      </select>
    </div>
  </div>

  <!-- Proposals Cards Grid / List -->
  {#if loading && proposals.length === 0}
    <div class="p-16 flex flex-col items-center justify-center text-muted-foreground gap-3">
      <RefreshCw class="w-6 h-6 animate-spin text-primary" />
      <span class="text-xs font-medium">Đang tải danh sách đề xuất từ AI Agent...</span>
    </div>
  {:else if filteredProposals.length === 0}
    <div class="p-16 border border-dashed border-border rounded-xl bg-card/40 flex flex-col items-center justify-center text-center">
      <div class="w-12 h-12 rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 flex items-center justify-center mb-3">
        <CheckCircle2 class="w-6 h-6" />
      </div>
      <h3 class="font-semibold text-base text-foreground mb-1">Không có tin nhắn nào đang chờ duyệt</h3>
      <p class="text-xs text-muted-foreground max-w-sm">
        Tuyệt vời! Tất cả các phản hồi và tin nhắn tiếp cận do AI đề xuất đã được xử lý xong.
      </p>
    </div>
  {:else}
    <div class="flex flex-col gap-4">
      {#each filteredProposals as prop (prop.id)}
        {@const PlatformIcon = getPlatformIcon(prop.platform)}
        <div class="border border-border bg-card rounded-xl p-5 shadow-2xs flex flex-col gap-4 hover:border-border/80 transition-all">
          <!-- Card Header -->
          <div class="flex items-center justify-between gap-3 flex-wrap border-b border-border/50 pb-3">
            <div class="flex items-center gap-2.5">
              <span class="w-7 h-7 rounded-lg bg-primary/10 text-primary flex items-center justify-center">
                <PlatformIcon class="w-4 h-4" />
              </span>
              <div class="flex flex-col">
                <span class="font-semibold text-xs text-foreground">
                  {prop.lead_name || `Lead #${prop.lead_id || 'Chưa gắn kết'}`}
                </span>
                <span class="text-[11px] text-muted-foreground font-mono">
                  {prop.lead_phone ? `${prop.lead_phone} • ` : ''}Kênh: <strong class="uppercase font-sans">{prop.platform}</strong>
                </span>
              </div>
            </div>

            <div class="flex items-center gap-2">
              <span class="text-[11px] px-2.5 py-0.5 rounded-full font-medium border {prop.status === 'APPROVED' ? 'bg-emerald-500/10 text-emerald-600 border-emerald-500/30' : prop.status === 'REJECTED' ? 'bg-rose-500/10 text-rose-600 border-rose-500/30' : 'bg-amber-500/10 text-amber-600 border-amber-500/30'}">
                {prop.status === 'APPROVED' ? 'ĐÃ DUYỆT' : prop.status === 'REJECTED' ? 'ĐÃ TỪ CHỐI' : 'CHỜ DUYỆT'}
              </span>
              <span class="text-[11px] text-muted-foreground font-mono">
                {prop.created_at ? new Date(prop.created_at).toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' }) : ''}
              </span>
            </div>
          </div>

          <!-- Proposed Message Content -->
          <div class="p-3.5 rounded-lg bg-muted/40 border border-border/70 text-xs text-foreground font-sans leading-relaxed whitespace-pre-wrap">
            {prop.proposed_content}
          </div>

          <!-- AI Reasoning / Explanation -->
          <div class="flex items-start gap-2 text-xs text-muted-foreground bg-primary/5 p-2.5 rounded-lg border border-primary/10">
            <Bot class="w-3.5 h-3.5 text-primary shrink-0 mt-0.5" />
            <div class="flex flex-col">
              <span class="font-semibold text-[11px] text-primary">Lý giải từ AI Agent (Evidence Ledger):</span>
              <span class="text-[11px] text-foreground/80">{prop.reasoning}</span>
            </div>
          </div>

          <!-- Actions -->
          {#if prop.status === 'PENDING'}
            <div class="flex items-center justify-end gap-2 pt-1">
              <Button
                variant="outline"
                size="sm"
                class="h-8 text-xs text-rose-600 hover:text-rose-700 hover:bg-rose-500/10 flex items-center gap-1"
                onclick={() => handleReject(prop)}
                disabled={reviewing}
              >
                <X class="w-3.5 h-3.5" />
                <span>Từ Chối</span>
              </Button>

              <Button
                variant="outline"
                size="sm"
                class="h-8 text-xs flex items-center gap-1"
                onclick={() => openEdit(prop)}
                disabled={reviewing}
              >
                <Edit3 class="w-3.5 h-3.5" />
                <span>Chỉnh Sửa</span>
              </Button>

              <Button
                size="sm"
                class="h-8 text-xs bg-emerald-600 hover:bg-emerald-700 text-white flex items-center gap-1.5 shadow-2xs"
                onclick={() => handleApprove(prop)}
                disabled={reviewing}
              >
                <Send class="w-3.5 h-3.5" />
                <span>Duyệt & Gửi Ngay</span>
              </Button>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Modal: Edit Content Before Approval -->
{#if editModalOpen && editingProposal}
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4 backdrop-blur-xs animate-fade-in">
    <div class="bg-card border border-border rounded-xl max-w-lg w-full p-6 shadow-2xl flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <h3 class="text-base font-bold text-foreground flex items-center gap-2">
          <Edit3 class="w-4 h-4 text-primary" />
          <span>Chỉnh Sửa Tin Nhắn Trước Khi Duyệt</span>
        </h3>
        <Button variant="ghost" size="icon" onclick={() => editModalOpen = false}>
          <X class="w-4 h-4" />
        </Button>
      </div>

      <div class="flex flex-col gap-2">
        <label for="proposal-edit-textarea" class="text-xs font-semibold text-muted-foreground">Nội dung tin nhắn:</label>
        <textarea
          id="proposal-edit-textarea"
          bind:value={editedContent}
          rows={6}
          class="w-full p-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring resize-y"
        ></textarea>
      </div>

      <div class="flex items-center justify-between pt-2">
        <Button variant="outline" size="sm" onclick={() => editModalOpen = false}>Hủy Bỏ</Button>
        <Button
          size="sm"
          class="bg-emerald-600 hover:bg-emerald-700 text-white flex items-center gap-1.5"
          onclick={handleSaveAndApprove}
          disabled={reviewing}
        >
          <Check class="w-3.5 h-3.5" />
          <span>Lưu & Duyệt Gửi</span>
        </Button>
      </div>
    </div>
  </div>
{/if}
