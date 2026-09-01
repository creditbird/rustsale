<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle
  } from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import {
    deleteConversationQuickTemplate,
    listConversationQuickTemplates,
    updateConversationQuickTemplate,
    type ConversationQuickTemplate
  } from '$lib/services/conversationService';
  import QuickTemplateForm from './QuickTemplateForm.svelte';

  let {
    open = $bindable(false),
    templates: propTemplates,
    onTemplatesChanged,
    onToggleActive: customToggleActive,
    onDelete: customDelete
  }: {
    open: boolean;
    templates?: ConversationQuickTemplate[];
    onTemplatesChanged?: () => void;
    onToggleActive?: (template: ConversationQuickTemplate, newActive: boolean) => void;
    onDelete?: (id: number) => void;
  } = $props();

  let internalTemplates = $state<ConversationQuickTemplate[]>([]);
  let searchQuery = $state('');
  let editingTemplate = $state<ConversationQuickTemplate | null>(null);
  let showFormDialog = $state(false);
  let errorMessage = $state<string | null>(null);

  let activeTemplates = $derived(propTemplates ?? internalTemplates);

  $effect(() => {
    if (open && !propTemplates) {
      loadTemplates();
    }
  });

  async function loadTemplates() {
    try {
      const items = await listConversationQuickTemplates({ includeInactive: true });
      internalTemplates = items;
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  let filteredTemplates = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return activeTemplates;
    return activeTemplates.filter(
      (t) =>
        t.shortcut.toLowerCase().includes(q) ||
        t.title.toLowerCase().includes(q) ||
        t.bodyText.toLowerCase().includes(q) ||
        (t.category && t.category.toLowerCase().includes(q))
    );
  });

  function handleOpenCreate() {
    editingTemplate = null;
    showFormDialog = true;
  }

  function handleOpenEdit(template: ConversationQuickTemplate) {
    editingTemplate = template;
    showFormDialog = true;
  }

  async function handleToggleActive(template: ConversationQuickTemplate) {
    const newActive = !template.isActive;
    if (customToggleActive) {
      customToggleActive(template, newActive);
      return;
    }

    try {
      await updateConversationQuickTemplate({
        id: template.id,
        shortcut: template.shortcut,
        title: template.title,
        bodyText: template.bodyText,
        category: template.category,
        isActive: newActive
      });
      await loadTemplates();
      onTemplatesChanged?.();
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleDelete(id: number) {
    if (customDelete) {
      customDelete(id);
      return;
    }

    try {
      await deleteConversationQuickTemplate(id);
      await loadTemplates();
      onTemplatesChanged?.();
    } catch (err: unknown) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  function handleFormSaved() {
    loadTemplates();
    onTemplatesChanged?.();
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-[650px] max-h-[85vh] flex flex-col">
    <DialogHeader>
      <div class="flex items-center justify-between pr-6">
        <DialogTitle class="text-base font-semibold">
          Manage Quick Templates
        </DialogTitle>
        <span class="text-xs text-muted-foreground font-medium" data-testid="template-count-badge">
          {activeTemplates.length} / 500 templates
        </span>
      </div>
    </DialogHeader>

    {#if activeTemplates.length >= 500}
      <div
        class="p-3 rounded bg-amber-500/10 text-amber-600 dark:text-amber-400 text-xs font-medium border border-amber-500/20"
        data-testid="limit-warning-banner"
      >
        Reached maximum limit of 500 quick templates. Please delete some before adding new ones.
      </div>
    {/if}

    {#if errorMessage}
      <div class="p-2.5 rounded bg-destructive/10 text-destructive text-xs font-medium">
        {errorMessage}
      </div>
    {/if}

    <div class="flex items-center justify-between gap-3 py-2">
      <Input
        bind:value={searchQuery}
        placeholder="Search templates by shortcut, title, content..."
        class="text-xs h-9"
        data-testid="template-search-input"
      />
      <Button
        type="button"
        size="sm"
        disabled={activeTemplates.length >= 500}
        onclick={handleOpenCreate}
        data-testid="add-template-btn"
      >
        + Add New Template
      </Button>
    </div>

    <div class="flex-1 overflow-y-auto min-h-[250px] max-h-[400px] border border-border rounded-md divide-y divide-border">
      {#if filteredTemplates.length === 0}
        <div class="p-8 text-center text-muted-foreground text-xs">
          {searchQuery ? 'No matching templates found' : 'No quick templates yet'}
        </div>
      {:else}
        {#each filteredTemplates.slice(0, 100) as item (item.id)}
          <div class="p-3 flex items-start justify-between gap-3 hover:bg-muted/30 transition-colors">
            <div class="flex-1 space-y-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="font-mono font-bold text-xs text-primary bg-primary/10 px-1.5 py-0.5 rounded">
                  /{item.shortcut}
                </span>
                <span class="font-medium text-xs text-foreground truncate">
                  {item.title}
                </span>
                {#if item.category}
                  <span class="px-1.5 py-0.5 text-[10px] rounded bg-muted text-muted-foreground">
                    {item.category}
                  </span>
                {/if}
              </div>
              <p class="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                {item.bodyText}
              </p>
              <div class="text-[10px] text-muted-foreground/70">
                Used {item.usageCount} times
              </div>
            </div>

            <div class="flex items-center gap-1.5 shrink-0">
              <Button
                type="button"
                variant={item.isActive ? 'outline' : 'secondary'}
                size="sm"
                class="h-7 text-[11px] px-2"
                onclick={() => handleToggleActive(item)}
                data-testid="toggle-active-btn-{item.id}"
              >
                {item.isActive ? 'Active' : 'Inactive'}
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="h-7 text-[11px] px-2"
                onclick={() => handleOpenEdit(item)}
                data-testid="edit-template-btn-{item.id}"
              >
                Edit
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="h-7 text-[11px] px-2 text-destructive hover:text-destructive hover:bg-destructive/10"
                onclick={() => handleDelete(item.id)}
                data-testid="delete-template-btn-{item.id}"
              >
                Delete
              </Button>
            </div>
          </div>
        {/each}
        {#if filteredTemplates.length > 100}
          <div class="p-2.5 text-center text-muted-foreground text-[11px] bg-muted/20">
            Showing first 100 of {filteredTemplates.length} templates. Use search to find specific items.
          </div>
        {/if}
      {/if}
    </div>
  </DialogContent>
</Dialog>

{#if showFormDialog}
  <QuickTemplateForm
    bind:open={showFormDialog}
    template={editingTemplate}
    existingTemplates={activeTemplates}
    onSaved={handleFormSaved}
  />
{/if}
