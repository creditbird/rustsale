<script module lang="ts">
  export interface UserClickPayload {
    identityId: number | null;
    displayName: string | null;
    externalId?: string;
    sourceMessageId?: number;
    avatarUrl?: string | null;
    role?: string | null;
    targetEl?: HTMLElement | null;
  }

  export function isValidExternalZaloId(id?: string | null): boolean {
    if (!id) return false;
    const clean = id.trim();
    // Valid phone number (09... or 849...) or Zalo UID (6-30 digits)
    return /^(0\d{9}|84\d{9,10}|\d{6,30})$/.test(clean);
  }
</script>

<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { MessageSquare, UserCheck, Copy, X, ExternalLink, ShieldCheck } from 'lucide-svelte';

  let {
    open = false,
    payload = null,
    onClose = () => {},
    onOpenDirectChat = () => {},
    onOpenLead = () => {}
  }: {
    open?: boolean;
    payload?: UserClickPayload | null;
    onClose?: () => void;
    onOpenDirectChat?: (payload: UserClickPayload) => void;
    onOpenLead?: (payload: UserClickPayload) => void;
  } = $props();

  let copied = $state(false);

  let displayName = $derived(payload?.displayName || 'Zalo Member');
  let avatarUrl = $derived(payload?.avatarUrl || null);
  let identityId = $derived(payload?.identityId ?? null);
  let realExternalId = $derived(
    payload?.externalId && isValidExternalZaloId(payload.externalId)
      ? payload.externalId.trim()
      : null
  );
  let role = $derived(payload?.role || null);

  let initialLetters = $derived.by(() => {
    if (!displayName) return 'Z';
    const parts = displayName.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    }
    return displayName.slice(0, 2).toUpperCase();
  });

  function handleCopyTag() {
    if (!displayName) return;
    const tagText = `@${displayName}`;
    void navigator.clipboard?.writeText(tagText);
    copied = true;
    setTimeout(() => {
      copied = false;
    }, 2000);
  }

  function handleChat1on1() {
    if (!payload) return;
    onClose();
    onOpenDirectChat(payload);
  }

  function handleLeadAction() {
    if (!payload) return;
    onClose();
    onOpenLead(payload);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && payload}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/20 backdrop-blur-[1px] transition-opacity"
    onclick={onClose}
    aria-hidden="true"
  ></div>

  <!-- Centered / Floating Card Dialog -->
  <div
    class="fixed left-1/2 top-1/2 z-50 w-full max-w-sm -translate-x-1/2 -translate-y-1/2 p-4 sm:p-0"
    role="dialog"
    aria-modal="true"
    aria-label={`Member profile ${displayName}`}
  >
    <div
      class="relative overflow-hidden rounded-2xl border border-border/80 bg-background p-5 shadow-2xl transition-all dark:bg-[#1f2c34] dark:border-slate-700/80"
    >
      <!-- Close button -->
      <button
        type="button"
        class="absolute right-3 top-3 rounded-full p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
        onclick={onClose}
        aria-label="Close"
      >
        <X class="h-4 w-4" />
      </button>

      <!-- Header Info -->
      <div class="flex items-start gap-4 pr-6">
        {#if avatarUrl}
          <img
            src={avatarUrl}
            alt={displayName}
            class="h-14 w-14 rounded-full object-cover ring-2 ring-primary/20"
          />
        {:else}
          <div
            class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/10 text-primary font-bold text-lg ring-2 ring-primary/20 dark:bg-primary/20 dark:text-emerald-400"
          >
            {initialLetters}
          </div>
        {/if}

        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-1.5 flex-wrap">
            <h3 class="font-bold text-base text-foreground truncate">{displayName}</h3>
            {#if role}
              <Badge variant="outline" class="text-[10px] px-1.5 py-0 uppercase tracking-wider font-semibold border-primary/30 text-primary">
                {role}
              </Badge>
            {/if}
          </div>

          <div class="flex items-center gap-2 mt-1 text-xs text-muted-foreground">
            <span class="px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-600 dark:text-blue-400 font-semibold uppercase text-[10px]">
              Zalo Contact
            </span>
            {#if realExternalId}
              <span class="truncate max-w-[140px] font-mono text-[11px] opacity-80" title={`UID: ${realExternalId}`}>
                UID: {realExternalId}
              </span>
            {/if}
          </div>
        </div>
      </div>

      <div class="my-4 border-t border-border/60"></div>

      <!-- Actions -->
      <div class="flex flex-col gap-2.5">
        <Button
          variant="default"
          class="w-full justify-start gap-2.5 font-medium bg-[#00a884] hover:bg-[#008f70] text-white dark:bg-[#00a884] dark:hover:bg-[#008f70] shadow-sm"
          onclick={handleChat1on1}
          data-testid="btn-chat-1on1"
        >
          <MessageSquare class="h-4 w-4" />
          <span>Direct Message (1-on-1 Chat)</span>
        </Button>

        <div class="grid grid-cols-2 gap-2">
          <Button
            variant="outline"
            class="justify-start gap-2 text-xs font-medium border-border/80 hover:bg-muted"
            onclick={handleLeadAction}
            data-testid="btn-lead-crm"
          >
            <UserCheck class="h-3.5 w-3.5 text-blue-500" />
            <span>View / Create Lead</span>
          </Button>

          <Button
            variant="outline"
            class="justify-start gap-2 text-xs font-medium border-border/80 hover:bg-muted"
            onclick={handleCopyTag}
            data-testid="btn-copy-tag"
          >
            <Copy class="h-3.5 w-3.5 text-muted-foreground" />
            <span>{copied ? 'Copied!' : 'Copy @Tag'}</span>
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}
