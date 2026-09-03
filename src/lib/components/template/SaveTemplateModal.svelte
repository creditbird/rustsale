<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Save,
    X,
    Sparkles,
    Folder,
    Plus,
    AlertCircle,
    Check,
    Eye,
    RefreshCw,
    Bot,
    ImageIcon,
    Tag,
    Layers,
    MessageSquare,
    MessageCircleMore,
    Mail,
    Share2
  } from 'lucide-svelte';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    createTemplate,
    getTemplateFolders,
    createTemplateFolder,
    stringifyTags,
    renderSpintaxPreview,
    type TemplateFolderRecord,
    type CreateTemplateInput
  } from '$lib/services/templateService';

  let {
    open = $bindable(false),
    initialContent = '',
    initialChannel = 'zalo',
    onSaved
  }: {
    open: boolean;
    initialContent?: string;
    initialChannel?: string;
    onSaved?: (savedId: number) => void;
  } = $props();

  let name = $state('');
  let channel = $state('zalo');
  let folderId = $state<number | null>(null);
  let stage = $state('cold');
  let subject = $state('');
  let content = $state('');
  let mediaUrl = $state('');
  let aiPrompt = $state('');
  let tagInput = $state('');

  let folders = $state<TemplateFolderRecord[]>([]);
  let loadingFolders = $state(false);
  let isSaving = $state(false);
  let errorMessage = $state<string | null>(null);
  let showNewFolderInput = $state(false);
  let newFolderName = $state('');
  let previewActive = $state(false);
  let previewText = $state('');

  const CHANNELS = [
    { id: 'zalo', label: 'Zalo', icon: MessageSquare },
    { id: 'whatsapp', label: 'WhatsApp', icon: MessageCircleMore },
    { id: 'email', label: 'Email', icon: Mail },
    { id: 'facebook', label: 'Facebook / Messenger', icon: Share2 },
    { id: 'omnichannel', label: 'Đa Kênh', icon: Sparkles }
  ];

  const STAGES = [
    { id: 'cold', label: 'Tiếp cận đầu (Cold Outreach)' },
    { id: 'followup', label: 'Chăm sóc lại (Follow-up)' },
    { id: 'nurture', label: 'Nuôi dưỡng (Nurture)' },
    { id: 'closing', label: 'Chốt đơn (Closing)' }
  ];

  const PLACEHOLDERS = [
    { token: '{{name}}', label: 'Tên khách' },
    { token: '{{phone}}', label: 'Số ĐT' },
    { token: '{{company}}', label: 'Công ty' },
    { token: '{{email}}', label: 'Email' },
    { token: '{{value}}', label: 'Giá trị' }
  ];

  async function loadFolders() {
    loadingFolders = true;
    try {
      folders = await getTemplateFolders();
    } catch (err) {
      console.warn('Failed to load template folders:', err);
    } finally {
      loadingFolders = false;
    }
  }

  $effect(() => {
    if (open) {
      errorMessage = null;
      content = initialContent || '';
      channel = initialChannel || 'zalo';
      name = '';
      folderId = null;
      stage = 'cold';
      subject = '';
      mediaUrl = '';
      aiPrompt = '';
      tagInput = '';
      showNewFolderInput = false;
      newFolderName = '';
      previewActive = false;
      loadFolders();
    }
  });

  function insertPlaceholder(token: string) {
    content = (content ? content + ' ' : '') + token;
    updatePreview();
  }

  function insertSpintaxSample() {
    content = '{Xin chào|Chào bạn|Kính chào} ' + (content || '');
    updatePreview();
  }

  function updatePreview() {
    previewText = renderSpintaxPreview(content);
  }

  async function handleCreateFolder() {
    if (!newFolderName.trim()) return;
    try {
      const id = await createTemplateFolder(newFolderName.trim());
      await loadFolders();
      folderId = id;
      newFolderName = '';
      showNewFolderInput = false;
    } catch (err) {
      errorMessage = 'Không thể tạo thư mục: ' + (err instanceof Error ? err.message : String(err));
    }
  }

  async function handleSubmit() {
    if (!name.trim()) {
      errorMessage = 'Vui lòng nhập tên mẫu tin nhắn.';
      return;
    }
    if (!content.trim()) {
      errorMessage = 'Vui lòng nhập nội dung mẫu tin nhắn.';
      return;
    }

    isSaving = true;
    errorMessage = null;

    try {
      const input: CreateTemplateInput = {
        name: name.trim(),
        channel,
        folder_id: folderId,
        stage: stage || null,
        subject: subject.trim() || null,
        content: content.trim(),
        media_url: mediaUrl.trim() || null,
        ai_prompt: aiPrompt.trim() || null,
        tags_json: tagInput.trim() ? stringifyTags(tagInput) : null
      };

      const savedId = await createTemplate(input);
      onSaved?.(savedId);
      open = false;
    } catch (err) {
      errorMessage = 'Lỗi khi lưu mẫu: ' + (err instanceof Error ? err.message : String(err));
    } finally {
      isSaving = false;
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      open = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if open}
  <div
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-3 md:p-6 animate-fade-in"
    role="dialog"
    aria-modal="true"
    aria-labelledby="save-template-title"
  >
    <div
      class="bg-card border border-border rounded-2xl max-w-3xl w-full max-h-[90vh] flex flex-col shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-border flex items-center justify-between gap-4 bg-muted/30 shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center shrink-0">
            <Save class="w-5 h-5" />
          </div>
          <div>
            <h2 id="save-template-title" class="text-base font-semibold text-foreground tracking-tight">
              Lưu Thành Mẫu Kịch Bản
            </h2>
            <p class="text-xs text-muted-foreground">
              Lưu nội dung vào Thư Viện Mẫu tập trung để tái sử dụng trên các chiến dịch khác
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

      <!-- Form Body -->
      <div class="flex-1 overflow-y-auto p-6 flex flex-col gap-4">
        {#if errorMessage}
          <div class="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2.5">
            <AlertCircle class="w-4 h-4 shrink-0" />
            <span>{errorMessage}</span>
          </div>
        {/if}

        <!-- Name & Channel -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="template-name" class="text-xs font-medium text-foreground">
              Tên Mẫu <span class="text-destructive">*</span>
            </label>
            <input
              id="template-name"
              type="text"
              bind:value={name}
              placeholder="VD: Chào hàng B2B - Giảm 20% tháng 9"
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="template-channel" class="text-xs font-medium text-foreground">
              Kênh Tiếp Cận
            </label>
            <select
              id="template-channel"
              bind:value={channel}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer capitalize"
            >
              {#each CHANNELS as ch}
                <option value={ch.id}>{ch.label}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Folder & Stage -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <!-- Folder Selection -->
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center justify-between">
              <label for="template-folder" class="text-xs font-medium text-foreground">Thư Mục</label>
              <button
                type="button"
                onclick={() => (showNewFolderInput = !showNewFolderInput)}
                class="text-[11px] text-primary hover:underline flex items-center gap-1 cursor-pointer"
              >
                <Plus class="w-3 h-3" />
                <span>Tạo thư mục mới</span>
              </button>
            </div>

            {#if showNewFolderInput}
              <div class="flex items-center gap-1.5">
                <input
                  type="text"
                  bind:value={newFolderName}
                  placeholder="Tên thư mục..."
                  class="h-9 flex-1 px-3 rounded-lg border border-input bg-background text-xs text-foreground"
                />
                <Button type="button" size="sm" class="h-9 text-xs" onclick={handleCreateFolder}>Tạo</Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="h-9 text-xs"
                  onclick={() => (showNewFolderInput = false)}
                >
                  Hủy
                </Button>
              </div>
            {:else}
              <select
                id="template-folder"
                bind:value={folderId}
                class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
              >
                <option value={null}>(Không chọn - Thư mục mặc định)</option>
                {#each folders as f}
                  <option value={f.id}>📁 {f.name}</option>
                {/each}
              </select>
            {/if}
          </div>

          <!-- Stage -->
          <div class="flex flex-col gap-1.5">
            <label for="template-stage" class="text-xs font-medium text-foreground">Giai Đoạn Phễu</label>
            <select
              id="template-stage"
              bind:value={stage}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring cursor-pointer"
            >
              {#each STAGES as s}
                <option value={s.id}>{s.label}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Subject (for Email or Omnichannel) -->
        {#if channel === 'email' || channel === 'omnichannel'}
          <div class="flex flex-col gap-1.5">
            <label for="template-subject" class="text-xs font-medium text-foreground">
              Tiêu Đề Email {channel === 'email' ? '(Bắt buộc)' : '(Tùy chọn)'}
            </label>
            <input
              id="template-subject"
              type="text"
              bind:value={subject}
              placeholder={'VD: {{name}} ơi, đề xuất hợp tác đặc quyền từ RustSale...'}
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        {/if}

        <!-- Content & Variables -->
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-2 flex-wrap">
            <label for="template-content-input" class="text-xs font-medium text-foreground">
              Nội Dung Mẫu Tin Nhắn <span class="text-destructive">*</span>
            </label>

            <div class="flex items-center gap-1.5 flex-wrap">
              <span class="text-[11px] text-muted-foreground">Chèn nhanh:</span>
              {#each PLACEHOLDERS as p}
                <button
                  type="button"
                  onclick={() => insertPlaceholder(p.token)}
                  class="text-[11px] px-2 py-0.5 rounded bg-muted hover:bg-muted/80 text-foreground border border-border transition-colors font-mono cursor-pointer"
                  title={`Chèn ${p.label}`}
                >
                  {p.token}
                </button>
              {/each}
              <button
                type="button"
                onclick={insertSpintaxSample}
                class="text-[11px] px-2 py-0.5 rounded bg-primary/10 hover:bg-primary/20 text-primary border border-primary/20 transition-colors font-medium flex items-center gap-1 cursor-pointer"
              >
                <Sparkles class="w-3 h-3" />
                <span>+ Spintax</span>
              </button>
            </div>
          </div>

          <textarea
            id="template-content-input"
            bind:value={content}
            oninput={updatePreview}
            rows={5}
            placeholder={'Nhập nội dung tin nhắn. Hỗ trợ Spintax dạng {Chào anh|Chào chị} {{name}}...'}
            class="w-full p-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring font-sans leading-relaxed resize-y"
          ></textarea>

          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span class="text-[11px]">
              💡 Hỗ trợ Spintax <code>{'{A|B}'}</code> và biến cá nhân hóa CRM.
            </span>
            <button
              type="button"
              onclick={() => {
                previewActive = !previewActive;
                if (previewActive) updatePreview();
              }}
              class="text-xs font-medium text-primary hover:underline flex items-center gap-1 cursor-pointer"
            >
              <Eye class="w-3.5 h-3.5" />
              <span>{previewActive ? 'Ẩn xem thử' : 'Xem thử mẫu tin nhắn'}</span>
            </button>
          </div>
        </div>

        <!-- Live Preview Panel if active -->
        {#if previewActive}
          <div class="p-3 rounded-xl border border-border bg-muted/30 flex flex-col gap-2">
            <div class="flex items-center justify-between text-xs">
              <span class="font-semibold text-foreground flex items-center gap-1">
                <Eye class="w-3.5 h-3.5 text-primary" />
                <span>Mô Phỏng Tin Nhắn Gửi Đi</span>
              </span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onclick={updatePreview}
                class="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground flex items-center gap-1"
              >
                <RefreshCw class="w-3 h-3" />
                <span>Randomize</span>
              </Button>
            </div>
            <div class="p-3 rounded-lg bg-card border border-border text-xs text-foreground whitespace-pre-wrap leading-relaxed">
              {previewText || '(Chưa có nội dung)'}
            </div>
          </div>
        {/if}

        <!-- Media URL & Tags -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label for="template-media" class="text-xs font-medium text-foreground flex items-center gap-1">
              <ImageIcon class="w-3.5 h-3.5 text-muted-foreground" />
              <span>Ảnh Đính Kèm (Media URL)</span>
            </label>
            <input
              id="template-media"
              type="url"
              bind:value={mediaUrl}
              placeholder="https://example.com/banner.jpg"
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label for="template-tags" class="text-xs font-medium text-foreground flex items-center gap-1">
              <Tag class="w-3.5 h-3.5 text-muted-foreground" />
              <span>Tags (Phân cách bởi dấu phẩy)</span>
            </label>
            <input
              id="template-tags"
              type="text"
              bind:value={tagInput}
              placeholder="chao_hang, b2b, khuyen_mai"
              class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            />
          </div>
        </div>

        <!-- AI Copilot Prompt Guideline -->
        <div class="flex flex-col gap-1.5">
          <label for="template-aiprompt" class="text-xs font-medium text-foreground flex items-center gap-1">
            <Bot class="w-3.5 h-3.5 text-primary" />
            <span>Chỉ Dẫn AI Copilot Cho Mẫu Này (Tùy chọn)</span>
          </label>
          <input
            id="template-aiprompt"
            type="text"
            bind:value={aiPrompt}
            placeholder="VD: Khi tạo biến thể, tập trung vào giải pháp tiết kiệm thời gian cho giám đốc bán hàng..."
            class="h-9 px-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-border bg-muted/20 flex items-center justify-end gap-3 shrink-0">
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
          disabled={isSaving}
          onclick={handleSubmit}
          class="text-xs font-semibold flex items-center gap-1.5 shadow-xs"
        >
          {#if isSaving}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Đang lưu...</span>
          {:else}
            <Save class="w-3.5 h-3.5" />
            <span>Lưu Vào Thư Viện</span>
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}
