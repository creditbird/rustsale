<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getAllUnifiedCampaigns,
    saveUnifiedCampaign,
    deleteUnifiedCampaign,
    startUnifiedCampaign,
    pauseUnifiedCampaign,
    enrollLeadsIntoCampaign,
    getUnifiedCampaignAnalytics,
    type Campaign,
    type CampaignStep,
    type CampaignAnalytics
  } from '$lib/services/unifiedCampaignService';
  import { getLeads, getCategories, type Lead, type Category } from '$lib/services/crmService';
  import SequenceBuilder from './SequenceBuilder.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    Sparkles,
    Play,
    Pause,
    Plus,
    Trash2,
    Edit3,
    BarChart3,
    UserPlus,
    Users,
    Search,
    RefreshCw,
    CheckCircle2,
    Clock,
    AlertCircle,
    Send,
    MessageSquare,
    TrendingUp,
    Filter,
    X,
    Check,
    ChevronRight
  } from 'lucide-svelte';

  let {
    activeTab = 'campaign_center'
  }: {
    activeTab?: string;
  } = $props();

  let campaigns = $state<Campaign[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);

  // Stats
  let totalCampaignsCount = $derived(campaigns.length);
  let activeCampaignsCount = $derived(campaigns.filter(c => c.status === 'ACTIVE').length);

  // Campaign Create/Edit Modal State
  let editorOpen = $state(false);
  let editingCampaignId = $state<number | null>(null);
  let editorName = $state('');
  let editorDescription = $state('');
  let editorChannelType = $state('omnichannel');
  let editorStatus = $state('DRAFT');
  let editorSteps = $state<CampaignStep[]>([]);
  let saving = $state(false);

  // Enrollment Modal State
  let enrollModalOpen = $state(false);
  let selectedCampaignForEnroll = $state<Campaign | null>(null);
  let allLeads = $state<Lead[]>([]);
  let allCategories = $state<Category[]>([]);
  let leadSearchQuery = $state('');
  let selectedCategoryFilter = $state<number | null>(null);
  let selectedLeadIds = $state<Set<number>>(new Set());
  let enrolling = $state(false);

  // Analytics Modal State
  let analyticsModalOpen = $state(false);
  let activeAnalytics = $state<CampaignAnalytics | null>(null);
  let loadingAnalytics = $state(false);

  async function loadData() {
    loading = true;
    errorMsg = null;
    try {
      const [fetchedCampaigns, fetchedLeads, fetchedCats] = await Promise.all([
        getAllUnifiedCampaigns(),
        getLeads(),
        getCategories()
      ]);
      campaigns = fetchedCampaigns;
      allLeads = fetchedLeads;
      allCategories = fetchedCats;
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  function openCreateModal() {
    editingCampaignId = null;
    editorName = 'Chiến dịch Nuôi Dưỡng Lead Mới ' + new Date().toLocaleDateString('vi-VN');
    editorDescription = 'Tự động gửi thông điệp chào mừng và chăm sóc đa kênh theo phản hồi';
    editorChannelType = 'omnichannel';
    editorStatus = 'DRAFT';
    editorSteps = [
      {
        step_order: 1,
        channel: 'zalo',
        delay_minutes: 0,
        condition_type: 'ALWAYS',
        template_content: '{Xin chào|Chào bạn} {{name}}, bên mình nhận được yêu cầu tư vấn từ {{company}}. Em xin gửi catalog giải pháp qua đây nhé!',
        ai_prompt_override: null
      },
      {
        step_order: 2,
        channel: 'whatsapp',
        delay_minutes: 1440, // 1 day
        condition_type: 'IF_NOT_REPLIED',
        template_content: '{Dạ chào|Chào} anh/chị {{name}}, hôm qua bên em có gửi thông tin qua Zalo, không biết {{company}} đã xem qua chưa ạ? Nếu cần báo giá nhanh anh/chị cứ nhắn em nhé!',
        ai_prompt_override: null
      },
      {
        step_order: 3,
        channel: 'email',
        delay_minutes: 2880, // 2 days
        condition_type: 'IF_NOT_REPLIED',
        template_content: 'Kính gửi {{name}}, em xin gửi lại hồ sơ năng lực và chính sách ưu đãi tháng này của RustSale dành cho {{company}}.',
        ai_prompt_override: null
      }
    ];
    editorOpen = true;
  }

  function openEditModal(c: Campaign) {
    editingCampaignId = c.id ?? null;
    editorName = c.name;
    editorDescription = c.description ?? '';
    editorChannelType = c.channel_type;
    editorStatus = c.status;
    // We can populate default steps or existing steps
    editorSteps = [
      {
        step_order: 1,
        channel: 'zalo',
        delay_minutes: 0,
        condition_type: 'ALWAYS',
        template_content: '{Xin chào|Chào bạn} {{name}}, bên mình nhận được yêu cầu tư vấn từ {{company}}.',
        ai_prompt_override: null
      },
      {
        step_order: 2,
        channel: 'whatsapp',
        delay_minutes: 1440,
        condition_type: 'IF_NOT_REPLIED',
        template_content: '{Dạ chào|Chào} anh/chị {{name}}, không biết {{company}} đã xem qua báo giá chưa ạ?',
        ai_prompt_override: null
      }
    ];
    editorOpen = true;
  }

  async function handleSaveCampaign() {
    if (!editorName.trim()) {
      errorMsg = 'Vui lòng nhập tên chiến dịch!';
      return;
    }
    if (editorSteps.length === 0) {
      errorMsg = 'Vui lòng thêm ít nhất 1 bước trong kịch bản!';
      return;
    }

    saving = true;
    errorMsg = null;
    try {
      const payloadCampaign: Campaign = {
        id: editingCampaignId ?? undefined,
        name: editorName.trim(),
        description: editorDescription.trim() || null,
        channel_type: editorChannelType,
        status: editorStatus
      };

      await saveUnifiedCampaign(payloadCampaign, editorSteps);
      editorOpen = false;
      showToast('Đã lưu chiến dịch thành công!');
      await loadData();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }

  async function handleToggleStatus(c: Campaign) {
    if (!c.id) return;
    try {
      if (c.status === 'ACTIVE') {
        await pauseUnifiedCampaign(c.id);
        showToast(`Đã tạm dừng chiến dịch "${c.name}"`);
      } else {
        await startUnifiedCampaign(c.id);
        showToast(`Đã kích hoạt chiến dịch "${c.name}"`);
      }
      await loadData();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleDelete(c: Campaign) {
    if (!c.id) return;
    if (!confirm(`Bạn có chắc chắn muốn xóa chiến dịch "${c.name}"? Mọi dữ liệu phân tích sẽ bị hủy.`)) {
      return;
    }

    try {
      await deleteUnifiedCampaign(c.id);
      showToast(`Đã xóa chiến dịch "${c.name}"`);
      await loadData();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    }
  }

  function openEnrollmentModal(c: Campaign) {
    selectedCampaignForEnroll = c;
    selectedLeadIds = new Set();
    leadSearchQuery = '';
    selectedCategoryFilter = null;
    enrollModalOpen = true;
  }

  const filteredEnrollLeads = $derived(
    allLeads.filter(l => {
      if (selectedCategoryFilter !== null && l.category_id !== selectedCategoryFilter) {
        return false;
      }
      if (leadSearchQuery.trim()) {
        const q = leadSearchQuery.toLowerCase();
        const matchName = l.name?.toLowerCase().includes(q);
        const matchPhone = l.phone?.toLowerCase().includes(q);
        const matchCompany = l.company?.toLowerCase().includes(q);
        return matchName || matchPhone || matchCompany;
      }
      return true;
    })
  );

  function toggleLeadSelection(id?: number) {
    if (!id) return;
    const next = new Set(selectedLeadIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedLeadIds = next;
  }

  function selectAllFiltered() {
    const next = new Set(selectedLeadIds);
    filteredEnrollLeads.forEach(l => {
      if (l.id) next.add(l.id);
    });
    selectedLeadIds = next;
  }

  function deselectAll() {
    selectedLeadIds = new Set();
  }

  async function handleConfirmEnrollment() {
    if (!selectedCampaignForEnroll?.id || selectedLeadIds.size === 0) return;
    enrolling = true;
    errorMsg = null;
    try {
      const ids = Array.from(selectedLeadIds);
      const count = await enrollLeadsIntoCampaign(selectedCampaignForEnroll.id, ids);
      enrollModalOpen = false;
      showToast(`Đã ghi danh ${count} khách hàng vào chiến dịch "${selectedCampaignForEnroll.name}"!`);
      await loadData();
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      enrolling = false;
    }
  }

  async function openAnalytics(c: Campaign) {
    if (!c.id) return;
    analyticsModalOpen = true;
    loadingAnalytics = true;
    activeAnalytics = null;
    try {
      activeAnalytics = await getUnifiedCampaignAnalytics(c.id);
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loadingAnalytics = false;
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

  <!-- Header Section with Stats -->
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
    <!-- Stat 1: Total Campaigns -->
    <div class="p-5 rounded-xl border border-border bg-card shadow-2xs flex items-center justify-between">
      <div class="flex flex-col">
        <span class="text-xs font-medium text-muted-foreground uppercase tracking-wider">Tổng Chiến Dịch</span>
        <span class="text-2xl font-bold text-foreground mt-1 font-mono">{totalCampaignsCount}</span>
        <span class="text-[11px] text-muted-foreground mt-0.5">Tất cả kịch bản</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
        <Sparkles class="w-5 h-5" />
      </div>
    </div>

    <!-- Stat 2: Active Sequences -->
    <div class="p-5 rounded-xl border border-border bg-card shadow-2xs flex items-center justify-between">
      <div class="flex flex-col">
        <span class="text-xs font-medium text-muted-foreground uppercase tracking-wider">Đang Chạy Tự Động</span>
        <span class="text-2xl font-bold text-emerald-600 dark:text-emerald-400 mt-1 font-mono">{activeCampaignsCount}</span>
        <span class="text-[11px] text-emerald-600/80 dark:text-emerald-400/80 mt-0.5">Scheduler đang gửi</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 flex items-center justify-center">
        <Play class="w-5 h-5" />
      </div>
    </div>

    <!-- Stat 3: Total Leads Enrolled -->
    <div class="p-5 rounded-xl border border-border bg-card shadow-2xs flex items-center justify-between">
      <div class="flex flex-col">
        <span class="text-xs font-medium text-muted-foreground uppercase tracking-wider">Khách Đang Chăm Sóc</span>
        <span class="text-2xl font-bold text-blue-600 dark:text-blue-400 mt-1 font-mono">{allLeads.length}</span>
        <span class="text-[11px] text-muted-foreground mt-0.5">Toàn hệ thống CRM</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-blue-500/10 text-blue-600 dark:text-blue-400 flex items-center justify-center">
        <Users class="w-5 h-5" />
      </div>
    </div>

    <!-- Stat 4: Automation Engine -->
    <div class="p-5 rounded-xl border border-border bg-card shadow-2xs flex items-center justify-between">
      <div class="flex flex-col">
        <span class="text-xs font-medium text-muted-foreground uppercase tracking-wider">Động Cơ Đa Kênh</span>
        <span class="text-sm font-bold text-foreground mt-1.5 flex items-center gap-1.5">
          <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
          <span>Rust Tokio Core</span>
        </span>
        <span class="text-[11px] text-muted-foreground mt-0.5">Anti-ban & Điều phối AI</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-purple-500/10 text-purple-600 dark:text-purple-400 flex items-center justify-center">
        <TrendingUp class="w-5 h-5" />
      </div>
    </div>
  </div>

  <!-- Campaign Table Container -->
  <div class="border border-border bg-card rounded-xl shadow-2xs overflow-hidden flex flex-col">
    <!-- Action Bar -->
    <div class="p-4 border-b border-border/70 flex items-center justify-between gap-4 flex-wrap bg-muted/20">
      <div class="flex items-center gap-3">
        <h2 class="text-base font-bold text-foreground tracking-tight">Danh Sách Chiến Dịch Đa Kênh</h2>
        <Button variant="ghost" size="icon" onclick={loadData} disabled={loading} title="Làm mới" class="h-8 w-8">
          <RefreshCw class="w-3.5 h-3.5 {loading ? 'animate-spin' : ''}" />
        </Button>
      </div>

      <div class="flex items-center gap-2">
        <Button
          type="button"
          onclick={openCreateModal}
          class="flex items-center gap-1.5 font-medium shadow-xs"
        >
          <Plus class="w-4 h-4" />
          <span>Tạo Kịch Bản Mới</span>
        </Button>
      </div>
    </div>

    <!-- Campaigns List -->
    {#if loading && campaigns.length === 0}
      <div class="p-12 flex flex-col items-center justify-center text-muted-foreground gap-3">
        <RefreshCw class="w-6 h-6 animate-spin text-primary" />
        <span class="text-xs font-medium">Đang tải danh sách chiến dịch...</span>
      </div>
    {:else if campaigns.length === 0}
      <div class="p-16 flex flex-col items-center justify-center text-center">
        <div class="w-14 h-14 rounded-full bg-primary/10 text-primary flex items-center justify-center mb-4">
          <Sparkles class="w-7 h-7" />
        </div>
        <h3 class="text-base font-semibold text-foreground mb-1">Chưa có chiến dịch tiếp cận nào</h3>
        <p class="text-xs text-muted-foreground max-w-md mb-5">
          Tạo chuỗi tiếp cận tự động kết hợp Zalo, WhatsApp, Email và Facebook với AI Copilot để tăng 300% tỷ lệ phản hồi.
        </p>
        <Button onclick={openCreateModal} class="flex items-center gap-2">
          <Plus class="w-4 h-4" />
          <span>Tạo Chiến Dịch Đầu Tiên</span>
        </Button>
      </div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full text-left text-xs">
          <thead class="bg-muted/40 border-b border-border text-muted-foreground uppercase text-[10px] font-semibold tracking-wider">
            <tr>
              <th class="px-5 py-3.5">Tên Chiến Dịch & Mô Tả</th>
              <th class="px-4 py-3.5">Kênh</th>
              <th class="px-4 py-3.5">Trạng Thái</th>
              <th class="px-4 py-3.5">Ngày Tạo</th>
              <th class="px-5 py-3.5 text-right">Thao Tác</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/60">
            {#each campaigns as c (c.id)}
              <tr class="hover:bg-muted/30 transition-colors group">
                <!-- Name & Desc -->
                <td class="px-5 py-4">
                  <div class="flex flex-col">
                    <span class="font-semibold text-sm text-foreground group-hover:text-primary transition-colors">
                      {c.name}
                    </span>
                    <span class="text-xs text-muted-foreground line-clamp-1 mt-0.5">
                      {c.description || 'Không có mô tả'}
                    </span>
                  </div>
                </td>

                <!-- Channel Type -->
                <td class="px-4 py-4">
                  <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium border bg-primary/5 text-primary border-primary/20">
                    <Sparkles class="w-3 h-3" />
                    <span>{c.channel_type === 'omnichannel' ? 'Đa Kênh Tự Động' : c.channel_type.toUpperCase()}</span>
                  </span>
                </td>

                <!-- Status Badge -->
                <td class="px-4 py-4">
                  {#if c.status === 'ACTIVE'}
                    <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/30">
                      <span class="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
                      <span>ĐANG CHẠY</span>
                    </span>
                  {:else if c.status === 'PAUSED'}
                    <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/30">
                      <Pause class="w-3 h-3" />
                      <span>TẠM DỪNG</span>
                    </span>
                  {:else}
                    <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-zinc-500/10 text-zinc-600 dark:text-zinc-400 border border-zinc-500/30">
                      <span>BẢN NHÁP</span>
                    </span>
                  {/if}
                </td>

                <!-- Created At -->
                <td class="px-4 py-4 text-muted-foreground font-mono text-[11px]">
                  {c.created_at ? new Date(c.created_at).toLocaleDateString('vi-VN') : '—'}
                </td>

                <!-- Actions -->
                <td class="px-5 py-4 text-right">
                  <div class="flex items-center justify-end gap-1.5">
                    <!-- Toggle Run/Pause -->
                    <Button
                      variant={c.status === 'ACTIVE' ? 'secondary' : 'default'}
                      size="sm"
                      class="h-7 px-2.5 text-xs flex items-center gap-1"
                      onclick={() => handleToggleStatus(c)}
                      title={c.status === 'ACTIVE' ? 'Tạm dừng gửi' : 'Kích hoạt chiến dịch'}
                    >
                      {#if c.status === 'ACTIVE'}
                        <Pause class="w-3.5 h-3.5 text-amber-500" />
                        <span>Dừng</span>
                      {:else}
                        <Play class="w-3.5 h-3.5 text-emerald-500" />
                        <span>Chạy</span>
                      {/if}
                    </Button>

                    <!-- Enroll Leads -->
                    <Button
                      variant="outline"
                      size="sm"
                      class="h-7 px-2.5 text-xs flex items-center gap-1"
                      onclick={() => openEnrollmentModal(c)}
                      title="Ghi danh khách hàng vào chiến dịch"
                    >
                      <UserPlus class="w-3.5 h-3.5 text-primary" />
                      <span>Ghi Danh</span>
                    </Button>

                    <!-- View Analytics -->
                    <Button
                      variant="outline"
                      size="sm"
                      class="h-7 px-2.5 text-xs flex items-center gap-1"
                      onclick={() => openAnalytics(c)}
                      title="Xem báo cáo hiệu quả"
                    >
                      <BarChart3 class="w-3.5 h-3.5 text-blue-500" />
                      <span>Báo Cáo</span>
                    </Button>

                    <!-- Edit -->
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-muted-foreground hover:text-foreground"
                      onclick={() => openEditModal(c)}
                      title="Chỉnh sửa kịch bản"
                    >
                      <Edit3 class="w-3.5 h-3.5" />
                    </Button>

                    <!-- Delete -->
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-rose-500 hover:text-rose-600 hover:bg-rose-500/10"
                      onclick={() => handleDelete(c)}
                      title="Xóa chiến dịch"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </Button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<!-- Modal 1: Sequence Builder / Campaign Editor -->
{#if editorOpen}
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4 backdrop-blur-xs animate-fade-in">
    <div class="bg-card border border-border rounded-xl max-w-4xl w-full max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-border bg-muted/30 flex items-center justify-between shrink-0">
        <div>
          <h3 class="text-base font-bold text-foreground flex items-center gap-2">
            <Sparkles class="w-4 h-4 text-primary" />
            <span>{editingCampaignId ? 'Chỉnh Sửa Chiến Dịch' : 'Thiết Lập Chiến Dịch Đa Kênh Mới'}</span>
          </h3>
          <p class="text-xs text-muted-foreground mt-0.5">
            Cấu hình thông tin cơ bản và xây dựng chuỗi kịch bản chăm sóc tự động.
          </p>
        </div>
        <Button variant="ghost" size="icon" onclick={() => editorOpen = false}>
          <X class="w-4 h-4" />
        </Button>
      </div>

      <!-- Modal Body (Scrollable) -->
      <div class="p-6 overflow-y-auto flex flex-col gap-6 flex-1">
        <!-- Campaign Info Form -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="campaign-name-input" class="text-xs font-semibold text-foreground">Tên Chiến Dịch *</label>
            <input
              id="campaign-name-input"
              type="text"
              bind:value={editorName}
              placeholder="VD: Chăm sóc Lead bất động sản..."
              class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="campaign-channel-select" class="text-xs font-semibold text-foreground">Loại Chiến Dịch</label>
            <select
              id="campaign-channel-select"
              bind:value={editorChannelType}
              class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            >
              <option value="omnichannel">Đa Kênh Tự Động (Omnichannel)</option>
              <option value="zalo">Chuyên Zalo</option>
              <option value="whatsapp">Chuyên WhatsApp</option>
              <option value="email">Chuyên Email</option>
              <option value="facebook">Chuyên Facebook Messenger</option>
            </select>
          </div>

          <div class="flex flex-col gap-1.5 md:col-span-2">
            <label for="campaign-desc-input" class="text-xs font-semibold text-foreground">Mô Tả Mục Tiêu</label>
            <input
              id="campaign-desc-input"
              type="text"
              bind:value={editorDescription}
              placeholder="Mô tả tóm tắt đối tượng khách hàng hoặc mục tiêu chuyển đổi..."
              class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        </div>

        <!-- Visual Sequence Builder -->
        <div class="pt-4 border-t border-border">
          <SequenceBuilder bind:steps={editorSteps} />
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="px-6 py-3.5 border-t border-border bg-muted/20 flex items-center justify-between shrink-0">
        <Button variant="outline" size="sm" onclick={() => editorOpen = false}>Hủy Bỏ</Button>
        <div class="flex items-center gap-2">
          <Button size="sm" onclick={handleSaveCampaign} disabled={saving} class="flex items-center gap-1.5">
            {#if saving}
              <RefreshCw class="w-3.5 h-3.5 animate-spin" />
              <span>Đang lưu...</span>
            {:else}
              <Check class="w-3.5 h-3.5" />
              <span>Lưu Chiến Dịch</span>
            {/if}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Modal 2: Lead Enrollment Drawer / Modal -->
{#if enrollModalOpen && selectedCampaignForEnroll}
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4 backdrop-blur-xs animate-fade-in">
    <div class="bg-card border border-border rounded-xl max-w-3xl w-full max-h-[85vh] flex flex-col shadow-2xl overflow-hidden">
      <div class="px-6 py-4 border-b border-border bg-muted/30 flex items-center justify-between shrink-0">
        <div>
          <h3 class="text-base font-bold text-foreground flex items-center gap-2">
            <UserPlus class="w-4 h-4 text-primary" />
            <span>Ghi Danh Khách Hàng Vào Chiến Dịch</span>
          </h3>
          <p class="text-xs text-muted-foreground mt-0.5">
            Chiến dịch: <span class="font-semibold text-foreground">{selectedCampaignForEnroll.name}</span>
          </p>
        </div>
        <Button variant="ghost" size="icon" onclick={() => enrollModalOpen = false}>
          <X class="w-4 h-4" />
        </Button>
      </div>

      <!-- Filter Bar -->
      <div class="p-4 border-b border-border/60 bg-muted/10 flex items-center justify-between gap-3 flex-wrap shrink-0">
        <div class="flex items-center gap-2 flex-1 min-w-[200px]">
          <div class="relative w-full">
            <Search class="w-3.5 h-3.5 absolute left-3 top-2.5 text-muted-foreground" />
            <input
              type="text"
              bind:value={leadSearchQuery}
              placeholder="Tìm theo tên, điện thoại, công ty..."
              class="w-full h-8 pl-8 pr-3 text-xs rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        </div>

        <div class="flex items-center gap-2">
          <select
            bind:value={selectedCategoryFilter}
            class="h-8 px-2.5 text-xs rounded-md border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          >
            <option value={null}>Tất cả danh mục ({allLeads.length})</option>
            {#each allCategories as cat}
              <option value={cat.id}>{cat.name}</option>
            {/each}
          </select>

          <Button variant="outline" size="sm" class="h-8 text-xs" onclick={selectAllFiltered}>
            Chọn tất cả ({filteredEnrollLeads.length})
          </Button>
          {#if selectedLeadIds.size > 0}
            <Button variant="ghost" size="sm" class="h-8 text-xs text-muted-foreground" onclick={deselectAll}>
              Bỏ chọn
            </Button>
          {/if}
        </div>
      </div>

      <!-- Leads List Selection -->
      <div class="p-4 overflow-y-auto flex-1 flex flex-col gap-1.5 divide-y divide-border/40">
        {#if filteredEnrollLeads.length === 0}
          <div class="p-8 text-center text-muted-foreground text-xs">
            Không tìm thấy khách hàng nào phù hợp với bộ lọc.
          </div>
        {:else}
          {#each filteredEnrollLeads as lead (lead.id)}
            {@const isSelected = lead.id ? selectedLeadIds.has(lead.id) : false}
            <div
              role="button"
              tabindex="0"
              class="p-3 rounded-lg flex items-center justify-between gap-3 cursor-pointer transition-colors {isSelected ? 'bg-primary/10 border border-primary/30' : 'hover:bg-muted/40'}"
              onclick={() => toggleLeadSelection(lead.id)}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleLeadSelection(lead.id); } }}
            >
              <div class="flex items-center gap-3">
                <input
                  type="checkbox"
                  checked={isSelected}
                  class="rounded border-input text-primary focus:ring-primary h-4 w-4"
                />
                <div class="flex flex-col">
                  <span class="font-medium text-xs text-foreground">{lead.name || 'Không tên'}</span>
                  <span class="text-[11px] text-muted-foreground font-mono">{lead.phone || 'Không có SĐT'} • {lead.company || 'Cá nhân'}</span>
                </div>
              </div>

              <div class="flex items-center gap-2">
                {#if lead.has_zalo === 1}
                  <Badge variant="outline" class="text-[10px] text-blue-600 border-blue-500/30">Zalo</Badge>
                {/if}
                <Badge variant="secondary" class="text-[10px]">{lead.status || 'Mới'}</Badge>
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Footer -->
      <div class="px-6 py-3.5 border-t border-border bg-muted/20 flex items-center justify-between shrink-0">
        <span class="text-xs text-muted-foreground">
          Đã chọn: <strong class="text-foreground">{selectedLeadIds.size}</strong> khách hàng
        </span>
        <div class="flex items-center gap-2">
          <Button variant="outline" size="sm" onclick={() => enrollModalOpen = false}>Hủy</Button>
          <Button
            size="sm"
            onclick={handleConfirmEnrollment}
            disabled={enrolling || selectedLeadIds.size === 0}
            class="flex items-center gap-1.5"
          >
            {#if enrolling}
              <RefreshCw class="w-3.5 h-3.5 animate-spin" />
              <span>Đang xử lý...</span>
            {:else}
              <Check class="w-3.5 h-3.5" />
              <span>Bắt Đầu Ghi Danh ({selectedLeadIds.size})</span>
            {/if}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Modal 3: Analytics & Funnel Breakdown -->
{#if analyticsModalOpen}
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4 backdrop-blur-xs animate-fade-in">
    <div class="bg-card border border-border rounded-xl max-w-3xl w-full max-h-[85vh] flex flex-col shadow-2xl overflow-hidden">
      <div class="px-6 py-4 border-b border-border bg-muted/30 flex items-center justify-between shrink-0">
        <div>
          <h3 class="text-base font-bold text-foreground flex items-center gap-2">
            <BarChart3 class="w-4 h-4 text-primary" />
            <span>Phân Tích Hiệu Suất Chiến Dịch</span>
          </h3>
          <p class="text-xs text-muted-foreground mt-0.5">
            {activeAnalytics ? activeAnalytics.campaign_name : 'Đang tải dữ liệu...'}
          </p>
        </div>
        <Button variant="ghost" size="icon" onclick={() => analyticsModalOpen = false}>
          <X class="w-4 h-4" />
        </Button>
      </div>

      <div class="p-6 overflow-y-auto flex flex-col gap-6 flex-1">
        {#if loadingAnalytics}
          <div class="p-12 flex flex-col items-center justify-center text-muted-foreground gap-3">
            <RefreshCw class="w-6 h-6 animate-spin text-primary" />
            <span class="text-xs">Đang tính toán chỉ số phễu chuyển đổi...</span>
          </div>
        {:else if activeAnalytics}
          <!-- Top KPI Cards -->
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <div class="p-3.5 rounded-lg border border-border bg-muted/20 flex flex-col">
              <span class="text-[11px] text-muted-foreground font-medium">Ghi Danh</span>
              <span class="text-xl font-bold text-foreground font-mono mt-0.5">{activeAnalytics.total_enrolled}</span>
            </div>
            <div class="p-3.5 rounded-lg border border-border bg-muted/20 flex flex-col">
              <span class="text-[11px] text-muted-foreground font-medium">Đã Gửi Thành Công</span>
              <span class="text-xl font-bold text-blue-600 font-mono mt-0.5">{activeAnalytics.sent_count + activeAnalytics.delivered_count}</span>
            </div>
            <div class="p-3.5 rounded-lg border border-border bg-muted/20 flex flex-col">
              <span class="text-[11px] text-muted-foreground font-medium">Khách Phản Hồi</span>
              <span class="text-xl font-bold text-emerald-600 font-mono mt-0.5">{activeAnalytics.replied_count}</span>
              <span class="text-[10px] text-emerald-600/80 font-semibold mt-0.5">{activeAnalytics.response_rate.toFixed(1)}% tỷ lệ</span>
            </div>
            <div class="p-3.5 rounded-lg border border-border bg-muted/20 flex flex-col">
              <span class="text-[11px] text-muted-foreground font-medium">Chốt Deal (Won)</span>
              <span class="text-xl font-bold text-purple-600 font-mono mt-0.5">{activeAnalytics.converted_count}</span>
              <span class="text-[10px] text-purple-600/80 font-semibold mt-0.5">{activeAnalytics.conversion_rate.toFixed(1)}% chuyển đổi</span>
            </div>
          </div>

          <!-- Step Breakdown Funnel -->
          <div class="flex flex-col gap-3">
            <h4 class="text-xs font-bold text-foreground uppercase tracking-wider">Chi Tiết Từng Bước Trong Chuỗi</h4>
            {#if activeAnalytics.step_breakdown.length === 0}
              <div class="p-4 rounded-lg bg-muted/20 text-center text-xs text-muted-foreground">
                Chưa có dữ liệu thực thi cho từng bước.
              </div>
            {:else}
              <div class="border border-border rounded-lg overflow-hidden">
                <table class="w-full text-left text-xs">
                  <thead class="bg-muted/40 text-[10px] text-muted-foreground uppercase font-semibold">
                    <tr>
                      <th class="px-3.5 py-2.5">Bước</th>
                      <th class="px-3.5 py-2.5">Kênh</th>
                      <th class="px-3.5 py-2.5 text-right">Chờ gửi</th>
                      <th class="px-3.5 py-2.5 text-right">Đã gửi</th>
                      <th class="px-3.5 py-2.5 text-right">Phản hồi</th>
                      <th class="px-3.5 py-2.5 text-right">Thất bại</th>
                      <th class="px-3.5 py-2.5 text-right">Bỏ qua</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-border/60">
                    {#each activeAnalytics.step_breakdown as step}
                      <tr class="hover:bg-muted/30 font-mono text-xs">
                        <td class="px-3.5 py-2.5 font-bold font-sans">Bước {step.step_order}</td>
                        <td class="px-3.5 py-2.5 uppercase text-[11px] font-sans font-medium">{step.channel}</td>
                        <td class="px-3.5 py-2.5 text-right text-muted-foreground">{step.pending}</td>
                        <td class="px-3.5 py-2.5 text-right text-blue-600 font-semibold">{step.sent + step.delivered}</td>
                        <td class="px-3.5 py-2.5 text-right text-emerald-600 font-bold">{step.replied}</td>
                        <td class="px-3.5 py-2.5 text-right text-rose-500">{step.failed}</td>
                        <td class="px-3.5 py-2.5 text-right text-zinc-400">{step.skipped}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <div class="px-6 py-3.5 border-t border-border bg-muted/20 flex justify-end shrink-0">
        <Button size="sm" onclick={() => analyticsModalOpen = false}>Đóng</Button>
      </div>
    </div>
  </div>
{/if}
