<script lang="ts">
  import {
    classifyCustomerIntent,
    generateSmartReplySuggestions,
    getIntentBadgeColor,
    type CustomerIntent,
    type IntentResult,
    type SmartReplyOutput,
    type SmartReplyVariant
  } from '$lib/services/aiAgentService';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import {
    Sparkles,
    Bot,
    Send,
    Edit3,
    RefreshCw,
    ShieldCheck,
    AlertTriangle,
    ChevronDown,
    ChevronUp,
    Zap,
    Check
  } from 'lucide-svelte';

  let {
    leadId = null,
    conversationId = null,
    lastMessageText = '',
    onInsertDraft = () => {},
    onSendDraft = () => {}
  }: {
    leadId?: number | null;
    conversationId?: number | null;
    lastMessageText?: string;
    onInsertDraft?: (text: string) => void;
    onSendDraft?: (text: string) => void;
  } = $props();

  let loading = $state(false);
  let expanded = $state(false);
  let intentResult = $state<IntentResult | null>(null);
  let smartReplyData = $state<SmartReplyOutput | null>(null);
  let selectedVariantIndex = $state<number>(0);
  let customPrompt = $state('');
  let errorMsg = $state<string | null>(null);

  const selectedVariant = $derived(
    smartReplyData?.variants[selectedVariantIndex] ?? null
  );

  const badgeInfo = $derived(
    intentResult ? getIntentBadgeColor(intentResult.intent) : null
  );

  // Auto classify intent when lastMessageText changes
  $effect(() => {
    if (lastMessageText && lastMessageText.trim().length > 1) {
      handleClassifyIntent(lastMessageText.trim());
    } else {
      intentResult = null;
      smartReplyData = null;
    }
  });

  async function handleClassifyIntent(text: string) {
    try {
      intentResult = await classifyCustomerIntent(text);
    } catch (err) {
      console.warn('Failed to classify intent:', err);
    }
  }

  async function handleGenerateReplies() {
    if (!leadId) {
      // Fallback: classify text if no leadId
      if (lastMessageText) {
        await handleClassifyIntent(lastMessageText);
      }
      return;
    }

    loading = true;
    errorMsg = null;
    try {
      const output = await generateSmartReplySuggestions(
        leadId,
        conversationId,
        customPrompt.trim() || null
      );
      smartReplyData = output;
      intentResult = output.intent;
      selectedVariantIndex = 0;
      expanded = true;
    } catch (err) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function handleSelectVariant(index: number) {
    selectedVariantIndex = index;
  }

  function applyDraftToComposer() {
    if (selectedVariant) {
      onInsertDraft(selectedVariant.content);
    }
  }

  function sendDraftImmediately() {
    if (selectedVariant) {
      onSendDraft(selectedVariant.content);
    }
  }
</script>

<div class="border-b border-border/80 bg-background/95 backdrop-blur-xs px-4 py-2 flex flex-col gap-2 transition-all">
  <!-- Top Bar: Intent Tag & Quick Trigger -->
  <div class="flex items-center justify-between gap-3 flex-wrap">
    <div class="flex items-center gap-2">
      <div class="w-6 h-6 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0">
        <Sparkles class="w-3.5 h-3.5" />
      </div>
      <span class="text-xs font-semibold text-foreground tracking-tight">AI Copilot</span>

      {#if badgeInfo}
        <span class="text-[11px] font-medium px-2 py-0.5 rounded-full border flex items-center gap-1.5 {badgeInfo.bg} {badgeInfo.text} {badgeInfo.border}">
          <Bot class="w-3 h-3" />
          <span>Ý định: {badgeInfo.labelVi}</span>
          {#if intentResult}
            <span class="opacity-75 font-mono text-[10px]">({Math.round(intentResult.confidence * 100)}%)</span>
          {/if}
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-1.5">
      <Button
        type="button"
        variant={expanded ? 'secondary' : 'outline'}
        size="sm"
        onclick={handleGenerateReplies}
        disabled={loading}
        class="h-7 px-2.5 text-xs flex items-center gap-1.5 font-medium shadow-2xs"
      >
        {#if loading}
          <RefreshCw class="w-3 h-3 animate-spin text-primary" />
          <span>Đang tạo gợi ý...</span>
        {:else}
          <Zap class="w-3 h-3 text-primary" />
          <span>Gợi Ý Trả Lời AI</span>
        {/if}
      </Button>

      {#if smartReplyData}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground hover:text-foreground"
          onclick={() => expanded = !expanded}
          title={expanded ? 'Thu gọn' : 'Mở rộng'}
        >
          {#if expanded}
            <ChevronUp class="w-3.5 h-3.5" />
          {:else}
            <ChevronDown class="w-3.5 h-3.5" />
          {/if}
        </Button>
      {/if}
    </div>
  </div>

  <!-- Expanded Section: Smart Reply Pills & Preview -->
  {#if expanded && smartReplyData}
    <div class="p-3 rounded-xl border border-border bg-card/60 flex flex-col gap-3 animate-fade-in shadow-xs">
      <!-- 3 Variant Pills -->
      <div class="flex items-center gap-2 flex-wrap">
        {#each smartReplyData.variants as v, i}
          {@const isSelected = i === selectedVariantIndex}
          <button
            type="button"
            onclick={() => handleSelectVariant(i)}
            class="px-3 py-1.5 rounded-lg text-xs font-medium border transition-all cursor-pointer flex items-center gap-1.5 {isSelected ? 'bg-primary text-primary-foreground border-primary shadow-2xs font-semibold' : 'bg-muted/50 hover:bg-muted text-foreground border-border'}"
          >
            <span>{v.title}</span>
            {#if v.policy_decision === 'AUTO_APPROVE'}
              <ShieldCheck class="w-3 h-3 text-emerald-400" />
            {:else}
              <AlertTriangle class="w-3 h-3 text-amber-400" />
            {/if}
          </button>
        {/each}
      </div>

      <!-- Selected Draft Preview -->
      {#if selectedVariant}
        <div class="p-3 rounded-lg bg-muted/40 border border-border/80 text-xs text-foreground font-sans leading-relaxed flex flex-col gap-2">
          <p class="whitespace-pre-wrap">{selectedVariant.content}</p>

          <div class="flex items-center justify-between pt-2 border-t border-border/50 text-[11px] text-muted-foreground flex-wrap gap-2">
            <div class="flex items-center gap-2">
              <span class="flex items-center gap-1 text-emerald-600 dark:text-emerald-400">
                <ShieldCheck class="w-3 h-3" />
                <span>Độ an toàn: {selectedVariant.policy_decision === 'AUTO_APPROVE' ? 'Đã kiểm duyệt CRM' : 'Cần xem xét'}</span>
              </span>
              {#if selectedVariant.suggested_action}
                <span class="opacity-75">• Mục tiêu: {selectedVariant.suggested_action}</span>
              {/if}
            </div>

            <div class="flex items-center gap-1.5">
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="h-6 px-2 text-[11px] flex items-center gap-1"
                onclick={applyDraftToComposer}
              >
                <Edit3 class="w-3 h-3" />
                <span>Chèn vào soạn thảo</span>
              </Button>
              <Button
                type="button"
                size="sm"
                class="h-6 px-2 text-[11px] flex items-center gap-1 bg-emerald-600 hover:bg-emerald-700 text-white"
                onclick={sendDraftImmediately}
              >
                <Send class="w-3 h-3" />
                <span>Gửi ngay</span>
              </Button>
            </div>
          </div>
        </div>
      {/if}

      <!-- Custom Prompt Refinement Input -->
      <div class="flex items-center gap-2 pt-1">
        <input
          type="text"
          bind:value={customPrompt}
          placeholder="Yêu cầu AI điều chỉnh (VD: giảm thêm 5%, hẹn chiều mai 15h, thân thiện hơn...)"
          onkeydown={(e) => { if (e.key === 'Enter') handleGenerateReplies(); }}
          class="flex-1 h-7 px-2.5 text-xs rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onclick={handleGenerateReplies}
          disabled={loading}
          class="h-7 text-xs flex items-center gap-1 shrink-0"
        >
          <RefreshCw class="w-3 h-3 {loading ? 'animate-spin' : ''}" />
          <span>Tạo lại</span>
        </Button>
      </div>
    </div>
  {/if}
</div>
