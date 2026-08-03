<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import {
    Dialog,
    DialogContent,
    DialogFooter,
    DialogHeader,
    DialogTitle
  } from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Textarea } from '$lib/components/ui/textarea';
  import {
    createConversationQuickTemplate,
    updateConversationQuickTemplate,
    type ConversationQuickTemplate
  } from '$lib/services/conversationService';

  let {
    open = $bindable(false),
    template = null,
    existingTemplates = [],
    onSaved = () => {},
    onCancel = () => {}
  }: {
    open: boolean;
    template?: ConversationQuickTemplate | null;
    existingTemplates?: ConversationQuickTemplate[];
    onSaved?: (template: ConversationQuickTemplate) => void;
    onCancel?: () => void;
  } = $props();

  let shortcut = $state('');
  let title = $state('');
  let bodyText = $state('');
  let category = $state('');
  let isActive = $state(true);
  let submitting = $state(false);
  let errorMessage = $state<string | null>(null);

  $effect(() => {
    if (open) {
      if (template) {
        shortcut = template.shortcut;
        title = template.title;
        bodyText = template.bodyText;
        category = template.category ?? '';
        isActive = template.isActive;
      } else {
        shortcut = '';
        title = '';
        bodyText = '';
        category = '';
        isActive = true;
      }
      errorMessage = null;
    }
  });

  function handleClose() {
    open = false;
    onCancel();
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    errorMessage = null;

    let cleanShortcut = shortcut.trim();
    if (cleanShortcut.startsWith('/')) {
      cleanShortcut = cleanShortcut.slice(1).trim();
    }

    if (!cleanShortcut) {
      errorMessage = 'Vui lòng nhập phím tắt (shortcut)';
      return;
    }

    if (/\s/.test(cleanShortcut)) {
      errorMessage = 'Phím tắt không được chứa khoảng trắng';
      return;
    }

    const cleanTitle = title.trim();
    if (!cleanTitle) {
      errorMessage = 'Vui lòng nhập tiêu đề mẫu';
      return;
    }

    const cleanBody = bodyText.trim();
    if (!cleanBody) {
      errorMessage = 'Vui lòng nhập nội dung tin nhắn';
      return;
    }

    // Duplicate check
    const isDuplicate = existingTemplates.some(
      (t) =>
        t.shortcut.toLowerCase() === cleanShortcut.toLowerCase() &&
        (!template || t.id !== template.id)
    );

    if (isDuplicate) {
      errorMessage = `Phím tắt "/${cleanShortcut}" đã tồn tại`;
      return;
    }

    submitting = true;
    try {
      let saved: ConversationQuickTemplate;
      if (template) {
        saved = await updateConversationQuickTemplate({
          id: template.id,
          shortcut: cleanShortcut,
          title: cleanTitle,
          bodyText: cleanBody,
          category: category.trim() || null,
          isActive
        });
      } else {
        saved = await createConversationQuickTemplate({
          shortcut: cleanShortcut,
          title: cleanTitle,
          bodyText: cleanBody,
          category: category.trim() || null
        });
      }

      onSaved(saved);
      open = false;
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-[480px]">
    <DialogHeader>
      <DialogTitle>
        {template ? 'Chỉnh sửa mẫu tin nhắn' : 'Thêm mẫu tin nhắn mới'}
      </DialogTitle>
    </DialogHeader>

    <form onsubmit={handleSubmit} class="space-y-4 pt-2" data-testid="quick-template-form">
      {#if errorMessage}
        <div class="p-2.5 rounded bg-destructive/10 text-destructive text-xs font-medium" data-testid="form-error">
          {errorMessage}
        </div>
      {/if}

      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1">
          <Label for="shortcut" class="text-xs">Phím tắt (Shortcut)</Label>
          <div class="relative flex items-center">
            <span class="absolute left-2.5 text-xs text-muted-foreground font-mono">/</span>
            <Input
              id="shortcut"
              bind:value={shortcut}
              placeholder="gia"
              class="pl-6 font-mono text-xs"
              data-testid="template-shortcut-input"
            />
          </div>
        </div>

        <div class="space-y-1">
          <Label for="category" class="text-xs">Danh mục (tùy chọn)</Label>
          <Input
            id="category"
            bind:value={category}
            placeholder="Bán hàng"
            class="text-xs"
            data-testid="template-category-input"
          />
        </div>
      </div>

      <div class="space-y-1">
        <Label for="title" class="text-xs">Tiêu đề mẫu</Label>
        <Input
          id="title"
          bind:value={title}
          placeholder="Bảng giá sản phẩm 2026"
          class="text-xs"
          data-testid="template-title-input"
        />
      </div>

      <div class="space-y-1">
        <Label for="bodyText" class="text-xs">Nội dung tin nhắn</Label>
        <Textarea
          id="bodyText"
          bind:value={bodyText}
          placeholder="Nhập nội dung tin nhắn sẽ được chèn khi nhập phím tắt..."
          rows={4}
          class="text-xs resize-none"
          data-testid="template-body-input"
        />
      </div>

      <DialogFooter class="pt-2">
        <Button
          type="button"
          variant="outline"
          onclick={handleClose}
          disabled={submitting}
        >
          Hủy
        </Button>
        <Button type="submit" disabled={submitting} data-testid="save-template-btn">
          {submitting ? 'Đang lưu...' : 'Lưu mẫu'}
        </Button>
      </DialogFooter>
    </form>
  </DialogContent>
</Dialog>
