<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Search,
    Folder,
    MessageSquare,
    MessageCircleMore,
    Mail,
    Share2,
    Layers,
    Sparkles,
    Check,
    X,
    Eye,
    RefreshCw,
    Tag,
    Bot,
    ImageIcon,
    Clock,
    Hash
  } from 'lucide-svelte';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    getTemplates,
    getTemplateFolders,
    renderSpintaxPreview,
    parseTags,
    type TemplateRecord,
    type TemplateFolderRecord
  } from '$lib/services/templateService';

  let {
    open = $bindable(false),
    channel = '',
    onSelect
  }: {
    open: boolean;
    channel?: string;
    onSelect: (template: TemplateRecord) => void;
  } = $props();

  let templates = $state<TemplateRecord[]>([]);
  let folders = $state<TemplateFolderRecord[]>([]);
  let loading = $state(false);
  let searchQuery = $state('');
  let selectedChannel = $state('all');
  let selectedFolderId = $state<number | 'all'>('all');
  let selectedTemplate = $state<TemplateRecord | null>(null);
  let previewText = $state('');

  const CHANNELS = [
    { id: 'all', label: 'Tất cả', icon: Layers, color: 'text-muted-foreground' },
    { id: 'zalo', label: 'Zalo', icon: MessageSquare, color: 'text-blue-500' },
    { id: 'whatsapp', label: 'WhatsApp', icon: MessageCircleMore, color: 'text-emerald-500' },
    { id: 'email', label: 'Email', icon: Mail, color: 'text-amber-500' },
    { id: 'facebook', label: 'Facebook', icon: Share2, color: 'text-indigo-500' },
    { id: 'omnichannel', label: 'Đa Kênh', icon: Sparkles, color: 'text-purple-500' }
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

      // Auto-select first matching template if none selected or not in current list
      syncSelection();
    } catch (err) {
      console.error('Failed to load templates in picker:', err);
    } finally {
      loading = false;
    }
  }

  function syncSelection() {
    const filtered = filteredTemplates;
    if (filtered.length > 0) {
      if (!selectedTemplate || !filtered.some((t) => t.id === selectedTemplate?.id)) {
        selectTemplate(filtered[0]);
      } else {
        updatePreview(selectedTemplate);
      }
    } else {
      selectedTemplate = null;
      previewText = '';
    }
  }

  function selectTemplate(tmpl: TemplateRecord) {
    selectedTemplate = tmpl;
    updatePreview(tmpl);
  }

  function updatePreview(tmpl: TemplateRecord | null) {
    if (!tmpl) {
      previewText = '';
      return;
    }
    previewText = renderSpintaxPreview(tmpl.content);
  }

  function randomizePreview() {
    if (selectedTemplate) {
      previewText = renderSpintaxPreview(selectedTemplate.content);
    }
  }

  // Reactive filtering
  let filteredTemplates = $derived.by(() => {
    let list = templates;

    // Filter by channel
    if (selectedChannel && selectedChannel !== 'all') {
      list = list.filter(
        (t) => t.channel === selectedChannel || t.channel === 'omnichannel'
      );
    }

    // Filter by folder
    if (selectedFolderId !== 'all') {
      list = list.filter((t) => t.folder_id === selectedFolderId);
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

  $effect(() => {
    if (open) {
      if (channel && channel !== 'all') {
        selectedChannel = channel;
      }
      loadData();
    }
  });

  $effect(() => {
    // Whenever filtered list updates, ensure selection is updated
    if (open && filteredTemplates) {
      if (!selectedTemplate || !filteredTemplates.some((t) => t.id === selectedTemplate?.id)) {
        if (filteredTemplates.length > 0) {
          selectTemplate(filteredTemplates[0]);
        } else {
          selectedTemplate = null;
          previewText = '';
        }
      }
    }
  });

  function handleChoose() {
    if (!selectedTemplate) return;
    onSelect(selectedTemplate);
    open = false;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      open = false;
    }
  }

  function getChannelConfig(ch: string) {
    return CHANNELS.find((c) => c.id === ch) || CHANNELS[0];
  }

  function getFolderName(folderId?: number | null): string | null {
    if (!folderId) return null;
    return folders.find((f) => f.id === folderId)?.name || null;
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if open}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-3 md:p-6 animate-fade-in"
    role="dialog"
    aria-modal="true"
    aria-labelledby="template-picker-title"
  >
    <div
      class="bg-card border border-border rounded-2xl max-w-5xl w-full h-[85vh] flex flex-col shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between gap-4 bg-muted/30 shrink-0">
        <div class="flex items-center gap-3 min-w-0">
          <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center shrink-0">
            <Sparkles class="w-5 h-5" />
          </div>
          <div class="min-w-0">
            <h2 id="template-picker-title" class="text-base font-semibold text-foreground tracking-tight">
              Thư Viện Mẫu Tin Nhắn
            </h2>
            <p class="text-xs text-muted-foreground truncate">
              Chọn mẫu kịch bản đã được định dạng sẵn với Spintax và biến cá nhân hóa
            </p>
          </div>
        </div>

        <button
          type="button"
          onclick={() => (open = false)}
          class="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted/80 flex items-center justify-center transition-colors cursor-pointer"
          title="Đóng"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Filter Bar -->
      <div class="px-6 py-3 border-b border-border/80 bg-card flex flex-wrap items-center gap-3 shrink-0">
        <!-- Search Input -->
        <div class="relative flex-1 min-w-[220px]">
          <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Tìm theo tên mẫu, nội dung, hashtag..."
            class="w-full h-9 pl-9 pr-4 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
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

        <!-- Channel Filter Tabs -->
        <div class="flex items-center gap-1 p-1 bg-muted/60 rounded-lg border border-border/60 overflow-x-auto">
          {#each CHANNELS as ch}
            {@const Icon = ch.icon}
            {@const isActive = selectedChannel === ch.id}
            <button
              type="button"
              onclick={() => (selectedChannel = ch.id)}
              class="px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1.5 transition-all whitespace-nowrap cursor-pointer {isActive
                ? 'bg-background text-foreground shadow-2xs font-semibold'
                : 'text-muted-foreground hover:text-foreground'}"
            >
              <Icon class="w-3.5 h-3.5 {isActive ? ch.color : ''}" />
              <span>{ch.label}</span>
            </button>
          {/each}
        </div>

        <!-- Folder Dropdown -->
        {#if folders.length > 0}
          <div class="flex items-center gap-1.5">
            <select
              bind:value={selectedFolderId}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              <option value="all">📁 Tất cả thư mục</option>
              {#each folders as f}
                <option value={f.id}>📁 {f.name}</option>
              {/each}
            </select>
          </div>
        {/if}
      </div>

      <!-- Main Body: Split View -->
      <div class="flex-1 grid grid-cols-1 md:grid-cols-12 min-h-0 divide-y md:divide-y-0 md:divide-x divide-border">
        <!-- Left Column: Template List (7 cols) -->
        <div class="md:col-span-7 flex flex-col h-full overflow-hidden bg-muted/10">
          <div class="px-4 py-2 bg-muted/30 border-b border-border/50 flex items-center justify-between text-xs text-muted-foreground shrink-0">
            <span>Danh sách ({filteredTemplates.length} mẫu)</span>
            {#if selectedChannel !== 'all'}
              <span class="capitalize">Đang lọc theo: {selectedChannel}</span>
            {/if}
          </div>

          <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-2.5">
            {#if loading}
              <div class="h-48 flex flex-col items-center justify-center text-muted-foreground gap-2">
                <RefreshCw class="w-5 h-5 animate-spin text-primary" />
                <span class="text-xs">Đang tải danh sách mẫu...</span>
              </div>
            {:else if filteredTemplates.length === 0}
              <div class="h-64 flex flex-col items-center justify-center text-center p-6">
                <div class="w-12 h-12 rounded-full bg-muted flex items-center justify-center text-muted-foreground mb-3">
                  <Layers class="w-6 h-6" />
                </div>
                <h4 class="text-sm font-medium text-foreground">Không tìm thấy mẫu phù hợp</h4>
                <p class="text-xs text-muted-foreground mt-1 max-w-xs">
                  Thử tìm kiếm với từ khóa khác hoặc chuyển sang bộ lọc "Tất cả" kênh.
                </p>
                {#if selectedChannel !== 'all' || searchQuery}
                  <Button
                    variant="outline"
                    size="sm"
                    class="mt-3 text-xs"
                    onclick={() => {
                      selectedChannel = 'all';
                      searchQuery = '';
                      selectedFolderId = 'all';
                    }}
                  >
                    Xóa bộ lọc
                  </Button>
                {/if}
              </div>
            {:else}
              {#each filteredTemplates as tmpl (tmpl.id)}
                {@const chCfg = getChannelConfig(tmpl.channel)}
                {@const ChIcon = chCfg.icon}
                {@const isSelected = selectedTemplate?.id === tmpl.id}
                {@const folderName = getFolderName(tmpl.folder_id)}
                {@const tags = parseTags(tmpl.tags_json)}

                <button
                  type="button"
                  onclick={() => selectTemplate(tmpl)}
                  class="w-full text-left p-3.5 rounded-xl border transition-all cursor-pointer flex flex-col gap-2 {isSelected
                    ? 'border-primary bg-primary/5 shadow-xs ring-1 ring-primary/30'
                    : 'border-border/70 bg-card hover:border-border hover:bg-muted/30'}"
                >
                  <div class="flex items-start justify-between gap-2">
                    <div class="flex items-center gap-2 min-w-0">
                      <span class="p-1 rounded-md bg-muted border border-border/60 shrink-0">
                        <ChIcon class="w-3.5 h-3.5 {chCfg.color}" />
                      </span>
                      <span class="font-semibold text-xs text-foreground truncate">{tmpl.name}</span>
                      {#if tmpl.stage}
                        <Badge variant="secondary" class="text-[10px] py-0 px-1.5 h-4 capitalize">
                          {tmpl.stage}
                        </Badge>
                      {/if}
                    </div>

                    <div class="flex items-center gap-1 shrink-0 text-[11px] text-muted-foreground">
                      <Clock class="w-3 h-3" />
                      <span>{tmpl.usage_count} lần</span>
                    </div>
                  </div>

                  {#if tmpl.subject}
                    <div class="text-[11px] font-medium text-foreground/80 truncate">
                      Tiêu đề: {tmpl.subject}
                    </div>
                  {/if}

                  <p class="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                    {tmpl.content}
                  </p>

                  <div class="flex items-center justify-between gap-2 pt-1 border-t border-border/40 text-[11px]">
                    <div class="flex items-center gap-1.5 flex-wrap min-w-0">
                      {#if folderName}
                        <span class="text-muted-foreground flex items-center gap-1 text-[10px] bg-muted/60 px-1.5 py-0.5 rounded border border-border/40">
                          <Folder class="w-2.5 h-2.5" />
                          <span>{folderName}</span>
                        </span>
                      {/if}
                      {#each tags.slice(0, 3) as tag}
                        <span class="text-[10px] text-primary/80 bg-primary/10 px-1.5 py-0.5 rounded">
                          #{tag}
                        </span>
                      {/each}
                      {#if tags.length > 3}
                        <span class="text-[10px] text-muted-foreground">+{tags.length - 3}</span>
                      {/if}
                    </div>

                    {#if isSelected}
                      <span class="text-primary font-medium flex items-center gap-1 text-xs shrink-0">
                        <Check class="w-3.5 h-3.5" />
                        <span>Đang chọn</span>
                      </span>
                    {/if}
                  </div>
                </button>
              {/each}
            {/if}
          </div>
        </div>

        <!-- Right Column: Live Preview & Action (5 cols) -->
        <div class="md:col-span-5 flex flex-col h-full overflow-hidden bg-card">
          <div class="px-4 py-2 bg-muted/30 border-b border-border/50 flex items-center justify-between text-xs text-muted-foreground shrink-0">
            <span class="font-medium text-foreground flex items-center gap-1.5">
              <Eye class="w-3.5 h-3.5 text-primary" />
              <span>Xem Trước Trực Quan</span>
            </span>
            {#if selectedTemplate}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onclick={randomizePreview}
                class="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground flex items-center gap-1"
                title="Đổi biến thể Spintax ngẫu nhiên"
              >
                <RefreshCw class="w-3 h-3" />
                <span>Thử biến thể khác</span>
              </Button>
            {/if}
          </div>

          <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
            {#if selectedTemplate}
              {@const chCfg = getChannelConfig(selectedTemplate.channel)}
              {@const ChIcon = chCfg.icon}
              {@const folderName = getFolderName(selectedTemplate.folder_id)}
              {@const tags = parseTags(selectedTemplate.tags_json)}

              <!-- Template Summary Card -->
              <div class="p-3 rounded-xl bg-muted/40 border border-border/70 flex flex-col gap-2">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-semibold text-foreground flex items-center gap-1.5">
                    <ChIcon class="w-4 h-4 {chCfg.color}" />
                    <span>{selectedTemplate.name}</span>
                  </span>
                  <Badge variant="outline" class="text-[10px] capitalize">
                    {selectedTemplate.channel}
                  </Badge>
                </div>

                <div class="flex items-center gap-2 text-[11px] text-muted-foreground flex-wrap">
                  {#if folderName}
                    <span class="flex items-center gap-1">📁 {folderName}</span>
                    <span>•</span>
                  {/if}
                  <span>Đã dùng: <b>{selectedTemplate.usage_count}</b> lần</span>
                  {#if selectedTemplate.stage}
                    <span>•</span>
                    <span class="capitalize">Giai đoạn: {selectedTemplate.stage}</span>
                  {/if}
                </div>

                {#if tags.length > 0}
                  <div class="flex items-center gap-1 flex-wrap pt-1">
                    {#each tags as tag}
                      <span class="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded">
                        #{tag}
                      </span>
                    {/each}
                  </div>
                {/if}
              </div>

              <!-- Channel Simulated Message Bubble -->
              <div class="flex flex-col gap-2">
                <div class="text-[11px] font-medium text-muted-foreground flex items-center justify-between">
                  <span>Mô phỏng gửi tới khách hàng:</span>
                  <span class="text-[10px] text-emerald-600 font-medium">Tự động điền dữ liệu</span>
                </div>

                <!-- Simulation Frame -->
                <div class="p-4 rounded-xl border border-border bg-muted/20 flex flex-col gap-3">
                  {#if selectedTemplate.channel === 'email'}
                    <div class="pb-2 border-b border-border/60 text-xs">
                      <div class="text-muted-foreground text-[11px]">
                        <span class="font-medium text-foreground">Người gửi:</span> RustSale Outreach
                      </div>
                      <div class="text-muted-foreground text-[11px]">
                        <span class="font-medium text-foreground">Tới:</span> Nguyễn Văn A &lt;nguyenvana@gmail.com&gt;
                      </div>
                      <div class="text-muted-foreground text-[11px] mt-1">
                        <span class="font-semibold text-foreground">Tiêu đề:</span>{' '}
                        {selectedTemplate.subject
                          ? renderSpintaxPreview(selectedTemplate.subject)
                          : '(Chưa có tiêu đề)'}
                      </div>
                    </div>
                  {/if}

                  {#if selectedTemplate.media_url}
                    <div class="rounded-lg overflow-hidden border border-border max-h-40 bg-black/5 flex items-center justify-center">
                      <img
                        src={selectedTemplate.media_url}
                        alt="Media đính kèm"
                        class="max-h-40 w-full object-cover"
                        onerror={(e) => ((e.currentTarget as HTMLElement).style.display = 'none')}
                      />
                    </div>
                  {/if}

                  <!-- Message Bubble -->
                  <div
                    class="p-3.5 rounded-2xl text-xs leading-relaxed whitespace-pre-wrap shadow-2xs max-w-[95%] {selectedTemplate.channel ===
                    'zalo'
                      ? 'bg-blue-600 text-white rounded-br-xs self-end'
                      : selectedTemplate.channel === 'whatsapp'
                        ? 'bg-emerald-700 text-white rounded-br-xs self-end'
                        : selectedTemplate.channel === 'facebook'
                          ? 'bg-indigo-600 text-white rounded-br-xs self-end'
                          : 'bg-card text-foreground border border-border/80'}"
                  >
                    {previewText || '(Nội dung mẫu trống)'}
                  </div>

                  <div class="text-[10px] text-muted-foreground text-right">
                    Đã xem • Vừa xong
                  </div>
                </div>
              </div>

              <!-- AI Copilot Prompt info if present -->
              {#if selectedTemplate.ai_prompt}
                <div class="p-3 rounded-xl border border-primary/20 bg-primary/5 flex items-start gap-2 text-xs">
                  <Bot class="w-4 h-4 text-primary shrink-0 mt-0.5" />
                  <div class="flex flex-col gap-0.5">
                    <span class="font-semibold text-primary text-[11px]">Chỉ dẫn AI Copilot kèm theo:</span>
                    <p class="text-muted-foreground text-[11px] leading-relaxed">
                      {selectedTemplate.ai_prompt}
                    </p>
                  </div>
                </div>
              {/if}
            {:else}
              <div class="h-64 flex flex-col items-center justify-center text-center p-6 text-muted-foreground">
                <Eye class="w-8 h-8 mb-2 opacity-40" />
                <p class="text-xs">Chọn một mẫu ở danh sách bên trái để xem thử giao diện tin nhắn</p>
              </div>
            {/if}
          </div>

          <!-- Bottom Footer Actions -->
          <div class="p-4 border-t border-border bg-muted/20 flex items-center justify-end gap-3 shrink-0">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onclick={() => (open = false)}
              class="text-xs"
            >
              Hủy Bỏ
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={!selectedTemplate}
              onclick={handleChoose}
              class="text-xs font-semibold flex items-center gap-1.5 shadow-xs"
            >
              <Check class="w-3.5 h-3.5" />
              <span>Chọn Mẫu Này</span>
            </Button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
