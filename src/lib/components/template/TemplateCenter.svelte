<script lang="ts">
  import { onMount } from 'svelte';
  import {
    BookOpen,
    Search,
    Plus,
    Folder,
    FolderPlus,
    Trash2,
    Edit3,
    Eye,
    RefreshCw,
    Sparkles,
    MessageSquare,
    MessageCircleMore,
    Mail,
    Share2,
    Layers,
    Tag,
    Clock,
    Check,
    X,
    Filter,
    Bot,
    ImageIcon,
    Copy,
    AlertCircle,
    ChevronRight,
    TrendingUp,
    MoreVertical
  } from 'lucide-svelte';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    getTemplates,
    getTemplateFolders,
    createTemplate,
    updateTemplate,
    deleteTemplate,
    createTemplateFolder,
    deleteTemplateFolder,
    incrementTemplateUsage,
    renderSpintaxPreview,
    parseTags,
    stringifyTags,
    type TemplateRecord,
    type TemplateFolderRecord,
    type CreateTemplateInput,
    type UpdateTemplateInput
  } from '$lib/services/templateService';

  let { activeTab = '' }: { activeTab?: string } = $props();

  // State
  let templates = $state<TemplateRecord[]>([]);
  let folders = $state<TemplateFolderRecord[]>([]);
  let loading = $state(true);
  let searchQuery = $state('');
  let selectedChannel = $state('all');
  let selectedFolderId = $state<number | 'all' | 'unassigned'>('all');
  let selectedStage = $state('all');
  let selectedTag = $state<string | null>(null);

  // Modals state
  let editModalOpen = $state(false);
  let editingTemplate = $state<TemplateRecord | null>(null);
  let previewModalOpen = $state(false);
  let previewingTemplate = $state<TemplateRecord | null>(null);
  let previewSampleText = $state('');

  let folderModalOpen = $state(false);
  let newFolderName = $state('');
  let newFolderColor = $state('#3b82f6');

  let deleteConfirmOpen = $state(false);
  let templateToDelete = $state<TemplateRecord | null>(null);
  let folderToDelete = $state<TemplateFolderRecord | null>(null);

  // Form state for Create / Edit modal
  let formName = $state('');
  let formChannel = $state('zalo');
  let formFolderId = $state<number | null>(null);
  let formStage = $state('cold');
  let formSubject = $state('');
  let formContent = $state('');
  let formMediaUrl = $state('');
  let formAiPrompt = $state('');
  let formTags = $state('');
  let formError = $state<string | null>(null);
  let formSubmitting = $state(false);
  let formLivePreviewActive = $state(false);
  let formLivePreviewText = $state('');

  const CHANNELS = [
    { id: 'all', label: 'Tất cả kênh', icon: Layers, color: 'text-muted-foreground' },
    { id: 'zalo', label: 'Zalo', icon: MessageSquare, color: 'text-blue-500' },
    { id: 'whatsapp', label: 'WhatsApp', icon: MessageCircleMore, color: 'text-emerald-500' },
    { id: 'email', label: 'Email', icon: Mail, color: 'text-amber-500' },
    { id: 'facebook', label: 'Facebook', icon: Share2, color: 'text-indigo-500' },
    { id: 'omnichannel', label: 'Đa Kênh', icon: Sparkles, color: 'text-purple-500' }
  ];

  const STAGES = [
    { id: 'all', label: 'Tất cả giai đoạn' },
    { id: 'cold', label: 'Tiếp cận đầu (Cold)' },
    { id: 'followup', label: 'Chăm sóc lại (Follow-up)' },
    { id: 'nurture', label: 'Nuôi dưỡng (Nurture)' },
    { id: 'closing', label: 'Chốt đơn (Closing)' }
  ];

  const COLOR_SWATCHES = [
    '#3b82f6', // blue
    '#10b981', // emerald
    '#f59e0b', // amber
    '#6366f1', // indigo
    '#ec4899', // pink
    '#8b5cf6', // purple
    '#ef4444'  // red
  ];

  const PLACEHOLDERS = [
    { token: '{{name}}', label: 'Tên khách' },
    { token: '{{phone}}', label: 'Số ĐT' },
    { token: '{{company}}', label: 'Công ty' },
    { token: '{{email}}', label: 'Email' },
    { token: '{{value}}', label: 'Giá trị' }
  ];

  async function loadData() {
    loading = true;
    try {
      const [tpls, flds] = await Promise.all([
        getTemplates(),
        getTemplateFolders().catch(() => [])
      ]);
      templates = tpls;
      folders = flds;
    } catch (err) {
      console.error('Failed to load templates:', err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  // Derived filtered templates
  let filteredTemplates = $derived.by(() => {
    let list = templates;

    // Filter by channel
    if (selectedChannel !== 'all') {
      list = list.filter(
        (t) => t.channel === selectedChannel || t.channel === 'omnichannel'
      );
    }

    // Filter by folder
    if (selectedFolderId === 'unassigned') {
      list = list.filter((t) => !t.folder_id);
    } else if (selectedFolderId !== 'all') {
      list = list.filter((t) => t.folder_id === selectedFolderId);
    }

    // Filter by stage
    if (selectedStage !== 'all') {
      list = list.filter((t) => t.stage === selectedStage);
    }

    // Filter by tag
    if (selectedTag) {
      list = list.filter((t) => {
        const tags = parseTags(t.tags_json);
        return tags.includes(selectedTag!);
      });
    }

    // Filter by search
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      list = list.filter(
        (t) =>
          t.name.toLowerCase().includes(q) ||
          t.content.toLowerCase().includes(q) ||
          (t.subject && t.subject.toLowerCase().includes(q)) ||
          (t.tags_json && t.tags_json.toLowerCase().includes(q))
      );
    }

    return list;
  });

  // Collect all unique tags for filter pills
  let allTags = $derived.by(() => {
    const set = new Set<string>();
    for (const t of templates) {
      const parsed = parseTags(t.tags_json);
      for (const tag of parsed) {
        set.add(tag);
      }
    }
    return Array.from(set);
  });

  // Compute folder counts
  let folderCounts = $derived.by(() => {
    const counts: Record<number, number> = {};
    let unassigned = 0;
    for (const t of templates) {
      if (t.folder_id) {
        counts[t.folder_id] = (counts[t.folder_id] || 0) + 1;
      } else {
        unassigned++;
      }
    }
    return { counts, unassigned, total: templates.length };
  });

  // Metric Stats
  let totalUsage = $derived(templates.reduce((sum, t) => sum + (t.usage_count || 0), 0));
  let mostUsedChannel = $derived.by(() => {
    const chUsage: Record<string, number> = {};
    for (const t of templates) {
      chUsage[t.channel] = (chUsage[t.channel] || 0) + (t.usage_count || 0);
    }
    let top = 'zalo';
    let max = -1;
    for (const [ch, cnt] of Object.entries(chUsage)) {
      if (cnt > max) {
        max = cnt;
        top = ch;
      }
    }
    return top;
  });
  let mostUsedCfg = $derived(getChannelConfig(mostUsedChannel));
  let MostUsedIcon = $derived(mostUsedCfg.icon);

  function openCreateModal() {
    editingTemplate = null;
    formName = '';
    formChannel = selectedChannel !== 'all' ? selectedChannel : 'zalo';
    formFolderId = typeof selectedFolderId === 'number' ? selectedFolderId : null;
    formStage = selectedStage !== 'all' ? selectedStage : 'cold';
    formSubject = '';
    formContent = '';
    formMediaUrl = '';
    formAiPrompt = '';
    formTags = '';
    formError = null;
    formSubmitting = false;
    formLivePreviewActive = false;
    formLivePreviewText = '';
    editModalOpen = true;
  }

  function openEditModal(template: TemplateRecord) {
    editingTemplate = template;
    formName = template.name;
    formChannel = template.channel;
    formFolderId = template.folder_id ?? null;
    formStage = template.stage ?? 'cold';
    formSubject = template.subject ?? '';
    formContent = template.content;
    formMediaUrl = template.media_url ?? '';
    formAiPrompt = template.ai_prompt ?? '';
    formTags = parseTags(template.tags_json).join(', ');
    formError = null;
    formSubmitting = false;
    formLivePreviewActive = false;
    formLivePreviewText = '';
    editModalOpen = true;
  }

  function openPreview(template: TemplateRecord) {
    previewingTemplate = template;
    previewSampleText = renderSpintaxPreview(template.content);
    previewModalOpen = true;
  }

  function randomizePreview() {
    if (previewingTemplate) {
      previewSampleText = renderSpintaxPreview(previewingTemplate.content);
    }
  }

  function updateFormPreview() {
    formLivePreviewText = renderSpintaxPreview(formContent);
  }

  function insertFormPlaceholder(token: string) {
    formContent = (formContent ? formContent + ' ' : '') + token;
    updateFormPreview();
  }

  function insertFormSpintax() {
    formContent = '{Xin chào|Chào bạn|Kính chào} ' + (formContent || '');
    updateFormPreview();
  }

  async function handleSaveTemplate() {
    if (!formName.trim()) {
      formError = 'Vui lòng nhập tên mẫu tin nhắn.';
      return;
    }
    if (!formContent.trim()) {
      formError = 'Vui lòng nhập nội dung tin nhắn.';
      return;
    }

    formSubmitting = true;
    formError = null;

    try {
      if (editingTemplate) {
        const updateInput: UpdateTemplateInput = {
          id: editingTemplate.id,
          name: formName.trim(),
          channel: formChannel,
          folder_id: formFolderId,
          stage: formStage || null,
          subject: formSubject.trim() || null,
          content: formContent.trim(),
          media_url: formMediaUrl.trim() || null,
          ai_prompt: formAiPrompt.trim() || null,
          tags_json: formTags.trim() ? stringifyTags(formTags) : null
        };
        await updateTemplate(updateInput);
      } else {
        const createInput: CreateTemplateInput = {
          name: formName.trim(),
          channel: formChannel,
          folder_id: formFolderId,
          stage: formStage || null,
          subject: formSubject.trim() || null,
          content: formContent.trim(),
          media_url: formMediaUrl.trim() || null,
          ai_prompt: formAiPrompt.trim() || null,
          tags_json: formTags.trim() ? stringifyTags(formTags) : null
        };
        await createTemplate(createInput);
      }

      await loadData();
      editModalOpen = false;
    } catch (err) {
      formError = 'Lỗi khi lưu mẫu: ' + (err instanceof Error ? err.message : String(err));
    } finally {
      formSubmitting = false;
    }
  }

  function confirmDeleteTemplate(template: TemplateRecord) {
    templateToDelete = template;
    folderToDelete = null;
    deleteConfirmOpen = true;
  }

  function confirmDeleteFolder(folder: TemplateFolderRecord) {
    folderToDelete = folder;
    templateToDelete = null;
    deleteConfirmOpen = true;
  }

  async function executeDelete() {
    try {
      if (templateToDelete) {
        await deleteTemplate(templateToDelete.id);
        templateToDelete = null;
      } else if (folderToDelete) {
        await deleteTemplateFolder(folderToDelete.id);
        if (selectedFolderId === folderToDelete.id) {
          selectedFolderId = 'all';
        }
        folderToDelete = null;
      }
      await loadData();
      deleteConfirmOpen = false;
    } catch (err) {
      console.error('Failed to delete item:', err);
    }
  }

  async function handleCreateFolder() {
    if (!newFolderName.trim()) return;
    try {
      await createTemplateFolder(newFolderName.trim(), newFolderColor);
      await loadData();
      newFolderName = '';
      folderModalOpen = false;
    } catch (err) {
      console.error('Failed to create folder:', err);
    }
  }

  function getChannelConfig(ch: string) {
    return CHANNELS.find((c) => c.id === ch) || CHANNELS[0];
  }

  function getFolderName(fId?: number | null): string | null {
    if (!fId) return null;
    return folders.find((f) => f.id === fId)?.name || null;
  }
</script>

<div class="flex flex-col gap-6 w-full max-w-7xl mx-auto">
  <!-- Top Metric Cards -->
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
    <div class="p-4 rounded-xl border border-border bg-card flex items-center justify-between shadow-2xs">
      <div class="flex flex-col gap-1">
        <span class="text-xs text-muted-foreground font-medium">Tổng số mẫu</span>
        <span class="text-2xl font-bold text-foreground font-mono">{templates.length}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
        <BookOpen class="w-5 h-5" />
      </div>
    </div>

    <div class="p-4 rounded-xl border border-border bg-card flex items-center justify-between shadow-2xs">
      <div class="flex flex-col gap-1">
        <span class="text-xs text-muted-foreground font-medium">Tổng lượt gửi mẫu</span>
        <span class="text-2xl font-bold text-foreground font-mono">{totalUsage}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-emerald-500/10 text-emerald-600 flex items-center justify-center">
        <TrendingUp class="w-5 h-5" />
      </div>
    </div>

    <div class="p-4 rounded-xl border border-border bg-card flex items-center justify-between shadow-2xs">
      <div class="flex flex-col gap-1">
        <span class="text-xs text-muted-foreground font-medium">Kênh gửi nhiều nhất</span>
        <span class="text-lg font-bold text-foreground capitalize flex items-center gap-1.5">
          <MostUsedIcon class="w-4 h-4 {mostUsedCfg.color}" />
          <span>{mostUsedChannel}</span>
        </span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-blue-500/10 text-blue-600 flex items-center justify-center">
        <MessageSquare class="w-5 h-5" />
      </div>
    </div>

    <div class="p-4 rounded-xl border border-border bg-card flex items-center justify-between shadow-2xs">
      <div class="flex flex-col gap-1">
        <span class="text-xs text-muted-foreground font-medium">Thư mục tổ chức</span>
        <span class="text-2xl font-bold text-foreground font-mono">{folders.length}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-purple-500/10 text-purple-600 flex items-center justify-center">
        <Folder class="w-5 h-5" />
      </div>
    </div>
  </div>

  <!-- Header Action Bar -->
  <div class="flex flex-wrap items-center justify-between gap-3 bg-card p-4 rounded-xl border border-border shadow-2xs">
    <!-- Channel Filter Tabs -->
    <div class="flex items-center gap-1 p-1 bg-muted/60 rounded-lg border border-border/60 overflow-x-auto max-w-full">
      {#each CHANNELS as ch}
        {@const Icon = ch.icon}
        {@const isActive = selectedChannel === ch.id}
        <button
          type="button"
          onclick={() => (selectedChannel = ch.id)}
          class="px-3 py-1.5 rounded-md text-xs font-medium flex items-center gap-1.5 transition-all whitespace-nowrap cursor-pointer {isActive
            ? 'bg-background text-foreground shadow-2xs font-semibold'
            : 'text-muted-foreground hover:text-foreground'}"
        >
          <Icon class="w-3.5 h-3.5 {isActive ? ch.color : ''}" />
          <span>{ch.label}</span>
        </button>
      {/each}
    </div>

    <!-- Right Action Buttons -->
    <div class="flex items-center gap-2.5">
      <Button
        type="button"
        variant="outline"
        size="sm"
        onclick={() => (folderModalOpen = true)}
        class="text-xs flex items-center gap-1.5"
      >
        <FolderPlus class="w-4 h-4 text-primary" />
        <span>+ Tạo Thư Mục</span>
      </Button>

      <Button
        type="button"
        size="sm"
        onclick={openCreateModal}
        class="text-xs font-semibold flex items-center gap-1.5 shadow-xs"
      >
        <Plus class="w-4 h-4" />
        <span>+ Thêm Mẫu Mới</span>
      </Button>
    </div>
  </div>

  <!-- Main Split Layout: Sidebar + Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
    <!-- Left Column: Folder Tree & Stage Filter (3 cols) -->
    <aside class="lg:col-span-3 flex flex-col gap-4">
      <!-- Folder Tree Box -->
      <div class="bg-card border border-border rounded-xl p-4 flex flex-col gap-3 shadow-2xs">
        <div class="flex items-center justify-between">
          <h3 class="text-xs font-semibold text-foreground uppercase tracking-wider flex items-center gap-1.5">
            <Folder class="w-3.5 h-3.5 text-primary" />
            <span>Thư Mục Quản Lý</span>
          </h3>
          <button
            type="button"
            onclick={() => (folderModalOpen = true)}
            class="text-[11px] text-primary hover:underline flex items-center gap-0.5 cursor-pointer"
          >
            <Plus class="w-3 h-3" />
            <span>Thêm</span>
          </button>
        </div>

        <div class="flex flex-col gap-1">
          <!-- All Templates -->
          <button
            type="button"
            onclick={() => (selectedFolderId = 'all')}
            class="w-full text-left px-2.5 py-2 rounded-lg text-xs font-medium flex items-center justify-between transition-colors cursor-pointer {selectedFolderId ===
            'all'
              ? 'bg-primary/10 text-primary font-semibold'
              : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
          >
            <div class="flex items-center gap-2">
              <Layers class="w-3.5 h-3.5" />
              <span>Tất cả mẫu</span>
            </div>
            <span class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground">
              {folderCounts.total}
            </span>
          </button>

          <!-- Unassigned Templates -->
          <button
            type="button"
            onclick={() => (selectedFolderId = 'unassigned')}
            class="w-full text-left px-2.5 py-2 rounded-lg text-xs font-medium flex items-center justify-between transition-colors cursor-pointer {selectedFolderId ===
            'unassigned'
              ? 'bg-primary/10 text-primary font-semibold'
              : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
          >
            <div class="flex items-center gap-2">
              <Folder class="w-3.5 h-3.5 text-muted-foreground" />
              <span>Chưa phân loại</span>
            </div>
            <span class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground">
              {folderCounts.unassigned}
            </span>
          </button>

          <!-- Custom Folders -->
          {#each folders as folder (folder.id)}
            {@const count = folderCounts.counts[folder.id] || 0}
            <div class="group flex items-center justify-between rounded-lg transition-colors {selectedFolderId === folder.id ? 'bg-primary/10' : 'hover:bg-muted/50'}">
              <button
                type="button"
                onclick={() => (selectedFolderId = folder.id)}
                class="flex-1 text-left px-2.5 py-2 text-xs font-medium flex items-center gap-2 truncate cursor-pointer {selectedFolderId ===
                folder.id
                  ? 'text-primary font-semibold'
                  : 'text-muted-foreground group-hover:text-foreground'}"
              >
                <span
                  class="w-2.5 h-2.5 rounded-full shrink-0"
                  style="background-color: {folder.color || '#3b82f6'};"
                ></span>
                <span class="truncate">{folder.name}</span>
              </button>

              <div class="flex items-center gap-1 pr-2">
                <span class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground">
                  {count}
                </span>
                <button
                  type="button"
                  onclick={() => confirmDeleteFolder(folder)}
                  class="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive p-1 transition-opacity cursor-pointer"
                  title="Xóa thư mục"
                >
                  <Trash2 class="w-3 h-3" />
                </button>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <!-- Stage Filter Box -->
      <div class="bg-card border border-border rounded-xl p-4 flex flex-col gap-3 shadow-2xs">
        <h3 class="text-xs font-semibold text-foreground uppercase tracking-wider flex items-center gap-1.5">
          <Filter class="w-3.5 h-3.5 text-primary" />
          <span>Giai Đoạn Tiếp Cận</span>
        </h3>

        <div class="flex flex-col gap-1">
          {#each STAGES as s}
            <button
              type="button"
              onclick={() => (selectedStage = s.id)}
              class="w-full text-left px-2.5 py-2 rounded-lg text-xs font-medium flex items-center justify-between transition-colors cursor-pointer {selectedStage ===
              s.id
                ? 'bg-primary/10 text-primary font-semibold'
                : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
            >
              <span>{s.label}</span>
              {#if selectedStage === s.id}
                <Check class="w-3 h-3 text-primary" />
              {/if}
            </button>
          {/each}
        </div>
      </div>

      <!-- Tag Filter Pills -->
      {#if allTags.length > 0}
        <div class="bg-card border border-border rounded-xl p-4 flex flex-col gap-2.5 shadow-2xs">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-semibold text-foreground uppercase tracking-wider flex items-center gap-1.5">
              <Tag class="w-3.5 h-3.5 text-primary" />
              <span>Tags ({allTags.length})</span>
            </h3>
            {#if selectedTag}
              <button
                type="button"
                onclick={() => (selectedTag = null)}
                class="text-[10px] text-muted-foreground hover:text-foreground underline cursor-pointer"
              >
                Bỏ chọn
              </button>
            {/if}
          </div>

          <div class="flex flex-wrap gap-1.5">
            {#each allTags as tag}
              <button
                type="button"
                onclick={() => (selectedTag = selectedTag === tag ? null : tag)}
                class="text-[11px] px-2 py-0.5 rounded-md border transition-all cursor-pointer {selectedTag ===
                tag
                  ? 'bg-primary text-primary-foreground border-primary'
                  : 'bg-muted/60 text-muted-foreground border-border/70 hover:text-foreground hover:bg-muted'}"
              >
                #{tag}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </aside>

    <!-- Right Column: Template Grid (9 cols) -->
    <main class="lg:col-span-9 flex flex-col gap-4">
      <!-- Search and Filter Indicator -->
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="relative flex-1 min-w-[260px]">
          <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Tìm kiếm theo tên mẫu, nội dung, biến, tags..."
            class="w-full h-9 pl-9 pr-4 rounded-lg border border-input bg-card text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
          {#if searchQuery}
            <button
              type="button"
              onclick={() => (searchQuery = '')}
              class="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X class="w-3.5 h-3.5" />
            </button>
          {/if}
        </div>

        <div class="text-xs text-muted-foreground flex items-center gap-2">
          <span>Hiển thị <b>{filteredTemplates.length}</b> / {templates.length} mẫu</span>
          {#if selectedChannel !== 'all' || selectedFolderId !== 'all' || selectedStage !== 'all' || selectedTag || searchQuery}
            <button
              type="button"
              onclick={() => {
                selectedChannel = 'all';
                selectedFolderId = 'all';
                selectedStage = 'all';
                selectedTag = null;
                searchQuery = '';
              }}
              class="text-primary hover:underline cursor-pointer"
            >
              (Đặt lại bộ lọc)
            </button>
          {/if}
        </div>
      </div>

      <!-- Loading State -->
      {#if loading}
        <div class="p-16 border border-border bg-card rounded-2xl flex flex-col items-center justify-center text-muted-foreground gap-3">
          <RefreshCw class="w-6 h-6 animate-spin text-primary" />
          <span class="text-xs">Đang tải kho mẫu kịch bản...</span>
        </div>
      {:else if filteredTemplates.length === 0}
        <!-- Empty State -->
        <div class="p-16 border border-dashed border-border bg-card/60 rounded-2xl flex flex-col items-center justify-center text-center">
          <div class="w-14 h-14 rounded-2xl bg-primary/10 text-primary flex items-center justify-center mb-3">
            <BookOpen class="w-7 h-7" />
          </div>
          <h3 class="text-base font-semibold text-foreground mb-1">Chưa tìm thấy mẫu tin nhắn nào</h3>
          <p class="text-xs text-muted-foreground max-w-sm mb-5">
            {templates.length === 0
              ? 'Bắt đầu tạo mẫu tin nhắn đầu tiên cho Zalo, WhatsApp, hoặc Email để tự động hóa chiến dịch bán hàng.'
              : 'Không có mẫu nào khớp với bộ lọc hiện tại. Hãy thử thay đổi từ khóa hoặc điều kiện lọc.'}
          </p>
          <Button type="button" size="sm" onclick={openCreateModal} class="flex items-center gap-1.5">
            <Plus class="w-4 h-4" />
            <span>Tạo Mẫu Tin Nhắn Mới</span>
          </Button>
        </div>
      {:else}
        <!-- Templates Grid -->
        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {#each filteredTemplates as tmpl (tmpl.id)}
            {@const chCfg = getChannelConfig(tmpl.channel)}
            {@const ChIcon = chCfg.icon}
            {@const folderName = getFolderName(tmpl.folder_id)}
            {@const tags = parseTags(tmpl.tags_json)}

            <div class="bg-card border border-border rounded-xl p-4 flex flex-col justify-between shadow-2xs hover:shadow-xs hover:border-border/80 transition-all group">
              <!-- Top Row: Channel, Name, Folder -->
              <div class="flex flex-col gap-2.5">
                <div class="flex items-start justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="p-1 rounded-md bg-muted border border-border/70 shrink-0">
                      <ChIcon class="w-3.5 h-3.5 {chCfg.color}" />
                    </span>
                    <span class="text-xs font-semibold text-foreground truncate" title={tmpl.name}>
                      {tmpl.name}
                    </span>
                  </div>

                  {#if tmpl.stage}
                    <Badge variant="secondary" class="text-[10px] py-0 px-1.5 h-4 capitalize shrink-0">
                      {tmpl.stage}
                    </Badge>
                  {/if}
                </div>

                <!-- Subject if Email -->
                {#if tmpl.subject}
                  <div class="text-[11px] font-medium text-foreground/80 truncate">
                    <span class="text-muted-foreground font-normal">Tiêu đề:</span> {tmpl.subject}
                  </div>
                {/if}

                <!-- Media Thumbnail if present -->
                {#if tmpl.media_url}
                  <div class="h-24 rounded-lg overflow-hidden border border-border bg-black/5 relative">
                    <img
                      src={tmpl.media_url}
                      alt="Media preview"
                      class="w-full h-full object-cover"
                      onerror={(e) => ((e.currentTarget as HTMLElement).style.display = 'none')}
                    />
                    <span class="absolute bottom-1 right-1 bg-black/60 text-white text-[9px] px-1.5 py-0.5 rounded flex items-center gap-1">
                      <ImageIcon class="w-2.5 h-2.5" />
                      <span>Đính kèm</span>
                    </span>
                  </div>
                {/if}

                <!-- Content snippet -->
                <p class="text-xs text-muted-foreground line-clamp-3 leading-relaxed font-sans">
                  {tmpl.content}
                </p>

                <!-- Tags & Folder -->
                <div class="flex items-center gap-1.5 flex-wrap text-[11px]">
                  {#if folderName}
                    <span class="text-[10px] text-muted-foreground flex items-center gap-1 bg-muted/60 px-1.5 py-0.5 rounded border border-border/40">
                      <Folder class="w-2.5 h-2.5" />
                      <span>{folderName}</span>
                    </span>
                  {/if}
                  {#each tags.slice(0, 2) as t}
                    <span class="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded">
                      #{t}
                    </span>
                  {/each}
                  {#if tags.length > 2}
                    <span class="text-[10px] text-muted-foreground">+{tags.length - 2}</span>
                  {/if}
                </div>
              </div>

              <!-- Card Footer: Stats & Actions -->
              <div class="pt-3 mt-3 border-t border-border/50 flex items-center justify-between gap-2">
                <span class="text-[11px] text-muted-foreground flex items-center gap-1 font-mono">
                  <Clock class="w-3 h-3" />
                  <span>{tmpl.usage_count} lượt</span>
                </span>

                <div class="flex items-center gap-1">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 text-muted-foreground hover:text-foreground"
                    onclick={() => openPreview(tmpl)}
                    title="Xem thử Spintax"
                  >
                    <Eye class="w-3.5 h-3.5" />
                  </Button>

                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 text-muted-foreground hover:text-foreground"
                    onclick={() => openEditModal(tmpl)}
                    title="Chỉnh sửa mẫu"
                  >
                    <Edit3 class="w-3.5 h-3.5" />
                  </Button>

                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                    onclick={() => confirmDeleteTemplate(tmpl)}
                    title="Xóa mẫu"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </Button>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </main>
  </div>
</div>

<!-- Modal: Create / Edit Template -->
{#if editModalOpen}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-3 md:p-6 animate-fade-in"
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-card border border-border rounded-2xl max-w-3xl w-full max-h-[90vh] flex flex-col shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-200">
      <div class="px-6 py-4 border-b border-border flex items-center justify-between gap-4 bg-muted/30 shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center shrink-0">
            {#if editingTemplate}
              <Edit3 class="w-5 h-5" />
            {:else}
              <Plus class="w-5 h-5" />
            {/if}
          </div>
          <div>
            <h2 class="text-base font-semibold text-foreground tracking-tight">
              {editingTemplate ? 'Chỉnh Sửa Mẫu Kịch Bản' : 'Thêm Mẫu Tin Nhắn Mới'}
            </h2>
            <p class="text-xs text-muted-foreground">
              {editingTemplate
                ? 'Cập nhật nội dung và phân loại cho mẫu này'
                : 'Tạo mẫu kịch bản mới cho chiến dịch tiếp cận'}
            </p>
          </div>
        </div>

        <button
          type="button"
          onclick={() => (editModalOpen = false)}
          class="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted/80 flex items-center justify-center transition-colors cursor-pointer"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-6 flex flex-col gap-4">
        {#if formError}
          <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2.5">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span>{formError}</span>
          </div>
        {/if}

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="edit-name" class="text-xs font-medium text-foreground">
              Tên Mẫu <span class="text-destructive">*</span>
            </label>
            <input
              id="edit-name"
              type="text"
              bind:value={formName}
              placeholder="VD: Lời chào Zalo - Tiếp cận ban giám đốc"
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="edit-channel" class="text-xs font-medium text-foreground">Kênh Tiếp Cận</label>
            <select
              id="edit-channel"
              bind:value={formChannel}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              {#each CHANNELS.filter(c => c.id !== 'all') as ch}
                <option value={ch.id}>{ch.label}</option>
              {/each}
            </select>
          </div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="edit-folder" class="text-xs font-medium text-foreground">Thư Mục</label>
            <select
              id="edit-folder"
              bind:value={formFolderId}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              <option value={null}>(Không phân loại)</option>
              {#each folders as f}
                <option value={f.id}>📁 {f.name}</option>
              {/each}
            </select>
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="edit-stage" class="text-xs font-medium text-foreground">Giai Đoạn Phễu</label>
            <select
              id="edit-stage"
              bind:value={formStage}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              {#each STAGES.filter(s => s.id !== 'all') as s}
                <option value={s.id}>{s.label}</option>
              {/each}
            </select>
          </div>
        </div>

        {#if formChannel === 'email' || formChannel === 'omnichannel'}
          <div class="flex flex-col gap-1.5">
            <label for="edit-subject" class="text-xs font-medium text-foreground">
              Tiêu Đề Email {formChannel === 'email' ? '(Bắt buộc)' : '(Tùy chọn)'}
            </label>
            <input
              id="edit-subject"
              type="text"
              bind:value={formSubject}
              placeholder="VD: {{name}} ơi, giải pháp tối ưu chi phí từ RustSale..."
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        {/if}

        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-2 flex-wrap">
            <label for="edit-content" class="text-xs font-medium text-foreground">
              Nội Dung Mẫu Tin Nhắn <span class="text-destructive">*</span>
            </label>

            <div class="flex items-center gap-1.5 flex-wrap">
              <span class="text-[11px] text-muted-foreground">Chèn biến:</span>
              {#each PLACEHOLDERS as p}
                <button
                  type="button"
                  onclick={() => insertFormPlaceholder(p.token)}
                  class="text-[11px] px-2 py-0.5 rounded bg-muted hover:bg-muted/80 text-foreground border border-border transition-colors font-mono cursor-pointer"
                >
                  {p.token}
                </button>
              {/each}
              <button
                type="button"
                onclick={insertFormSpintax}
                class="text-[11px] px-2 py-0.5 rounded bg-primary/10 hover:bg-primary/20 text-primary border border-primary/20 transition-colors font-medium flex items-center gap-1 cursor-pointer"
              >
                <Sparkles class="w-3 h-3" />
                <span>+ Spintax</span>
              </button>
            </div>
          </div>

          <textarea
            id="edit-content"
            bind:value={formContent}
            oninput={updateFormPreview}
            rows={5}
            placeholder="Nhập nội dung mẫu..."
            class="w-full p-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring font-sans leading-relaxed resize-y"
          ></textarea>

          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span class="text-[11px]">
              Dùng cú pháp <code>{'{A|B|C}'}</code> để tạo biến thể ngẫu nhiên.
            </span>
            <button
              type="button"
              onclick={() => {
                formLivePreviewActive = !formLivePreviewActive;
                if (formLivePreviewActive) updateFormPreview();
              }}
              class="text-xs text-primary font-medium hover:underline flex items-center gap-1 cursor-pointer"
            >
              <Eye class="w-3.5 h-3.5" />
              <span>{formLivePreviewActive ? 'Ẩn xem thử' : 'Xem thử tin nhắn'}</span>
            </button>
          </div>
        </div>

        {#if formLivePreviewActive}
          <div class="p-3 rounded-xl border border-border bg-muted/30 flex flex-col gap-2">
            <div class="flex items-center justify-between text-xs">
              <span class="font-semibold text-foreground">Xem Trước Mô Phỏng</span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onclick={updateFormPreview}
                class="h-6 px-2 text-[11px]"
              >
                <RefreshCw class="w-3 h-3" />
                <span>Đổi biến thể</span>
              </Button>
            </div>
            <div class="p-3 rounded-lg bg-card border border-border text-xs text-foreground whitespace-pre-wrap">
              {formLivePreviewText || '(Trống)'}
            </div>
          </div>
        {/if}

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="edit-media" class="text-xs font-medium text-foreground flex items-center gap-1">
              <ImageIcon class="w-3.5 h-3.5 text-muted-foreground" />
              <span>Media URL (Ảnh đính kèm)</span>
            </label>
            <input
              id="edit-media"
              type="url"
              bind:value={formMediaUrl}
              placeholder="https://..."
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="edit-tags" class="text-xs font-medium text-foreground flex items-center gap-1">
              <Tag class="w-3.5 h-3.5 text-muted-foreground" />
              <span>Tags (Phân cách bởi dấu phẩy)</span>
            </label>
            <input
              id="edit-tags"
              type="text"
              bind:value={formTags}
              placeholder="b2b, sale, kịch_bản_1"
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="edit-aiprompt" class="text-xs font-medium text-foreground flex items-center gap-1">
            <Bot class="w-3.5 h-3.5 text-primary" />
            <span>Chỉ Dẫn AI Copilot (Tùy chọn)</span>
          </label>
          <input
            id="edit-aiprompt"
            type="text"
            bind:value={formAiPrompt}
            placeholder="Chỉ dẫn phong cách cho AI khi tạo biến thể..."
            class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>
      </div>

      <div class="px-6 py-4 border-t border-border bg-muted/20 flex items-center justify-end gap-3 shrink-0">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onclick={() => (editModalOpen = false)}
          class="text-xs"
        >
          Hủy Bỏ
        </Button>
        <Button
          type="button"
          size="sm"
          disabled={formSubmitting}
          onclick={handleSaveTemplate}
          class="text-xs font-semibold flex items-center gap-1.5 shadow-xs"
        >
          {#if formSubmitting}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Đang lưu...</span>
          {:else}
            <Check class="w-3.5 h-3.5" />
            <span>{editingTemplate ? 'Lưu Thay Đổi' : 'Tạo Mẫu'}</span>
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Interactive Spintax & Channel Preview -->
{#if previewModalOpen && previewingTemplate}
  {@const chCfg = getChannelConfig(previewingTemplate.channel)}
  {@const ChIcon = chCfg.icon}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-3 md:p-6 animate-fade-in"
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-card border border-border rounded-2xl max-w-xl w-full p-6 shadow-2xl flex flex-col gap-4 animate-in fade-in zoom-in-95 duration-200">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <span class="p-1 rounded-md bg-muted border border-border/70">
            <ChIcon class="w-4 h-4 {chCfg.color}" />
          </span>
          <h3 class="text-base font-semibold text-foreground">{previewingTemplate.name}</h3>
        </div>
        <button
          type="button"
          onclick={() => (previewModalOpen = false)}
          class="h-7 w-7 rounded-md text-muted-foreground hover:text-foreground flex items-center justify-center"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <p class="text-xs text-muted-foreground">
        Mô phỏng tin nhắn sau khi hệ thống xử lý Spintax và điền thông tin khách hàng mẫu:
      </p>

      <!-- Message View Box -->
      <div class="p-4 rounded-xl border border-border bg-muted/20 flex flex-col gap-3">
        {#if previewingTemplate.channel === 'email'}
          <div class="pb-2 border-b border-border/60 text-xs">
            <div class="text-muted-foreground text-[11px]">
              <span class="font-medium text-foreground">Người gửi:</span> RustSale Outreach
            </div>
            <div class="text-muted-foreground text-[11px]">
              <span class="font-medium text-foreground">Tới:</span> Nguyễn Văn A &lt;nguyenvana@gmail.com&gt;
            </div>
            <div class="text-muted-foreground text-[11px] mt-1">
              <span class="font-semibold text-foreground">Tiêu đề:</span>{' '}
              {previewingTemplate.subject
                ? renderSpintaxPreview(previewingTemplate.subject)
                : '(Chưa có tiêu đề)'}
            </div>
          </div>
        {/if}

        {#if previewingTemplate.media_url}
          <div class="rounded-lg overflow-hidden border border-border max-h-40 bg-black/5">
            <img
              src={previewingTemplate.media_url}
              alt="Media đính kèm"
              class="max-h-40 w-full object-cover"
              onerror={(e) => ((e.currentTarget as HTMLElement).style.display = 'none')}
            />
          </div>
        {/if}

        <div
          class="p-3.5 rounded-2xl text-xs leading-relaxed whitespace-pre-wrap shadow-2xs max-w-[95%] {previewingTemplate.channel ===
          'zalo'
            ? 'bg-blue-600 text-white rounded-br-xs self-end'
            : previewingTemplate.channel === 'whatsapp'
              ? 'bg-emerald-700 text-white rounded-br-xs self-end'
              : previewingTemplate.channel === 'facebook'
                ? 'bg-indigo-600 text-white rounded-br-xs self-end'
                : 'bg-card text-foreground border border-border/80'}"
        >
          {previewSampleText || '(Nội dung mẫu trống)'}
        </div>
      </div>

      <div class="flex items-center justify-between pt-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onclick={randomizePreview}
          class="text-xs flex items-center gap-1.5"
        >
          <RefreshCw class="w-3.5 h-3.5 text-primary" />
          <span>Thử Biến Thể Spintax Khác</span>
        </Button>
        <Button size="sm" onclick={() => (previewModalOpen = false)} class="text-xs">
          Đóng
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Create Folder -->
{#if folderModalOpen}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4 animate-fade-in"
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-card border border-border rounded-2xl max-w-md w-full p-6 shadow-2xl flex flex-col gap-4 animate-in fade-in zoom-in-95 duration-200">
      <div class="flex items-center justify-between">
        <h3 class="text-base font-semibold text-foreground flex items-center gap-2">
          <FolderPlus class="w-4 h-4 text-primary" />
          <span>Tạo Thư Mục Mới</span>
        </h3>
        <button
          type="button"
          onclick={() => (folderModalOpen = false)}
          class="h-7 w-7 rounded-md text-muted-foreground hover:text-foreground flex items-center justify-center"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1.5">
          <label for="new-folder-name" class="text-xs font-medium text-foreground">Tên Thư Mục</label>
          <input
            id="new-folder-name"
            type="text"
            bind:value={newFolderName}
            placeholder="VD: Chiến dịch Tết 2026, Khách hàng B2B..."
            class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <span class="text-xs font-medium text-foreground">Màu Nhận Diện</span>
          <div class="flex items-center gap-2">
            {#each COLOR_SWATCHES as color}
              <button
                type="button"
                title={`Màu ${color}`}
                aria-label={`Màu ${color}`}
                onclick={() => (newFolderColor = color)}
                class="w-6 h-6 rounded-full border-2 transition-transform cursor-pointer {newFolderColor ===
                color
                  ? 'border-foreground scale-110'
                  : 'border-transparent hover:scale-105'}"
                style="background-color: {color};"
              ></button>
            {/each}
          </div>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2.5 pt-2">
        <Button variant="outline" size="sm" onclick={() => (folderModalOpen = false)} class="text-xs">
          Hủy
        </Button>
        <Button
          size="sm"
          disabled={!newFolderName.trim()}
          onclick={handleCreateFolder}
          class="text-xs font-semibold"
        >
          Tạo Thư Mục
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Delete Confirmation -->
{#if deleteConfirmOpen}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4 animate-fade-in"
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-card border border-border rounded-2xl max-w-sm w-full p-6 shadow-2xl flex flex-col gap-4 animate-in fade-in zoom-in-95 duration-200">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-destructive/10 text-destructive flex items-center justify-center shrink-0">
          <AlertCircle class="w-5 h-5" />
        </div>
        <div>
          <h3 class="text-sm font-semibold text-foreground">
            {templateToDelete ? 'Xác nhận xóa mẫu?' : 'Xác nhận xóa thư mục?'}
          </h3>
          <p class="text-xs text-muted-foreground mt-0.5">
            {templateToDelete
              ? `Bạn có chắc chắn muốn xóa mẫu "${templateToDelete.name}"? Thao tác này không thể hoàn tác.`
              : `Xóa thư mục "${folderToDelete?.name}"? Các mẫu trong thư mục sẽ chuyển thành chưa phân loại.`}
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2.5 pt-2">
        <Button variant="outline" size="sm" onclick={() => (deleteConfirmOpen = false)} class="text-xs">
          Hủy Bỏ
        </Button>
        <Button variant="destructive" size="sm" onclick={executeDelete} class="text-xs font-semibold">
          Xác Nhận Xóa
        </Button>
      </div>
    </div>
  </div>
{/if}
