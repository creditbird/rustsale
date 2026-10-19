<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Calendar,
    Send,
    Bot,
    ShieldCheck,
    RefreshCw,
    Plus,
    Trash2,
    CheckCircle2,
    AlertTriangle,
    XCircle,
    Smartphone,
    Monitor,
    Image,
    Video,
    Film,
    Link as LinkIcon,
    FileText,
    Layers,
    ThumbsUp,
    MessageCircle,
    Share2,
    Clock,
    Check,
    X,
    Settings2,
    ChevronDown,
    ExternalLink
  } from 'lucide-svelte';
  import {
    scheduleFacebookPost,
    publishFacebookPostNow,
    listFacebookScheduledPosts,
    cancelFacebookScheduledPost,
    createCommentAutomation,
    listCommentAutomations,
    updateCommentAutomation,
    deleteCommentAutomation,
    listCommentLogs,
    getFacebookPageHealth,
    type FacebookScheduledPost,
    type CommentAutomationRule,
    type CommentLog,
    type PageHealth
  } from '$lib/services/facebookService';
  import {
    listManagedFacebookAccounts,
    type ManagedFacebookAccount
  } from '$lib/services/accountSettingsService';
  import { m } from '$lib/i18n';

  let {
    activeTab = 'facebook_hub',
    onOpenSettings
  }: {
    activeTab?: string;
    onOpenSettings?: () => void;
  } = $props();

  // Sub-tabs
  let activeSubTab = $state<'composer' | 'automations' | 'compliance'>('composer');

  // Accounts & Page state
  let connectedAccounts = $state<ManagedFacebookAccount[]>([]);
  let isLoadingAccounts = $state(false);
  let selectedPageId = $state<string>('');
  let currentAccount = $derived(
    connectedAccounts.find((a) => a.pageId === selectedPageId) ||
    connectedAccounts[0] ||
    null
  );
  let pageHealth = $state<PageHealth | null>(null);
  let isLoadingHealth = $state(false);

  // Composer state
  let postType = $state<'status' | 'link' | 'photo' | 'multi_photo' | 'video' | 'reel'>('status');
  let postMessage = $state('');
  let postLinkUrl = $state('');
  let postMediaPaths = $state<string[]>([]);
  let mediaInput = $state('');
  let isScheduled = $state(false);
  let scheduledDateTime = $state('');
  let previewDevice = $state<'desktop' | 'mobile'>('desktop');
  let isSubmittingPost = $state(false);
  let postError = $state<string | null>(null);
  let postSuccess = $state<string | null>(null);

  // Scheduled posts list
  let scheduledPosts = $state<FacebookScheduledPost[]>([]);
  let isLoadingPosts = $state(false);

  // Automations state
  let automationRules = $state<CommentAutomationRule[]>([]);
  let commentLogs = $state<CommentLog[]>([]);
  let isLoadingAutomations = $state(false);
  let showRuleModal = $state(false);
  let editingRuleId = $state<number | null>(null);

  // Rule Form
  let ruleName = $state('');
  let rulePostIds = $state('');
  let ruleKeywords = $state('');
  let ruleMatchMode = $state<'contains' | 'exact' | 'regex' | 'all'>('contains');
  let ruleAutoLike = $state(true);
  let rulePublicReply = $state('');
  let rulePrivateReply = $state('');
  let ruleIsEnabled = $state(true);

  // Compliance state
  let isSyncingDeletion = $state(false);
  let deletionSyncResult = $state<string | null>(null);

  let activeRequestId = 0;
  let lastLoadedTab = '';
  $effect(() => {
    const tab = activeTab;
    if (tab === 'facebook_hub' && lastLoadedTab !== 'facebook_hub') {
      lastLoadedTab = 'facebook_hub';
      loadAccounts();
    } else if (tab !== 'facebook_hub') {
      lastLoadedTab = tab;
    }
  });

  async function loadAccounts() {
    if (isLoadingAccounts) return;
    const reqId = ++activeRequestId;
    isLoadingAccounts = true;
    try {
      connectedAccounts = await listManagedFacebookAccounts();
      if (reqId !== activeRequestId) return;

      const savedPageId = localStorage.getItem('rustsale_facebook_hub_selected_page_id');
      if (savedPageId && connectedAccounts.some((a) => a.pageId === savedPageId)) {
        selectedPageId = savedPageId;
      } else if (connectedAccounts.length > 0) {
        selectedPageId = connectedAccounts[0].pageId;
        localStorage.setItem('rustsale_facebook_hub_selected_page_id', selectedPageId);
      } else {
        selectedPageId = '';
        localStorage.removeItem('rustsale_facebook_hub_selected_page_id');
      }

      if (selectedPageId) {
        await loadPageData(selectedPageId, reqId);
      } else {
        pageHealth = null;
        scheduledPosts = [];
        automationRules = [];
        commentLogs = [];
      }
    } catch (e) {
      if (reqId !== activeRequestId) return;
      console.error('Failed to load connected facebook accounts:', e);
      connectedAccounts = [];
      selectedPageId = '';
    } finally {
      if (reqId === activeRequestId) {
        isLoadingAccounts = false;
      }
    }
  }

  async function handlePageSelect(pageId: string) {
    const reqId = ++activeRequestId;
    selectedPageId = pageId;
    localStorage.setItem('rustsale_facebook_hub_selected_page_id', selectedPageId);
    await loadPageData(pageId, reqId);
  }

  function handlePageSelectChange(e: Event) {
    const target = (e.target || e.currentTarget) as HTMLSelectElement;
    if (target?.value) {
      handlePageSelect(target.value);
    }
  }

  async function loadPageData(pageId: string, reqId: number) {
    if (!pageId) return;
    isLoadingHealth = true;
    isLoadingPosts = true;
    isLoadingAutomations = true;
    try {
      const [health, posts, rules, logs] = await Promise.all([
        getFacebookPageHealth(pageId).catch((e) => {
          console.error('Failed to load page health:', e);
          return null;
        }),
        listFacebookScheduledPosts(pageId).catch((e) => {
          console.error('Failed to load scheduled posts:', e);
          return [];
        }),
        listCommentAutomations(pageId).catch((e) => {
          console.error('Failed to load automations:', e);
          return [];
        }),
        listCommentLogs(pageId, 50).catch((e) => {
          console.error('Failed to load comment logs:', e);
          return [];
        })
      ]);

      if (reqId !== activeRequestId || selectedPageId !== pageId) return;

      pageHealth = health;
      scheduledPosts = Array.isArray(posts) ? posts : [];
      automationRules = Array.isArray(rules) ? rules : [];
      commentLogs = Array.isArray(logs) ? logs : [];
    } finally {
      if (reqId === activeRequestId) {
        isLoadingHealth = false;
        isLoadingPosts = false;
        isLoadingAutomations = false;
      }
    }
  }

  async function loadScheduledPosts() {
    if (selectedPageId) {
      await loadPageData(selectedPageId, ++activeRequestId);
    }
  }

  async function loadAutomations() {
    if (selectedPageId) {
      await loadPageData(selectedPageId, ++activeRequestId);
    }
  }

  async function loadPageHealth() {
    if (selectedPageId) {
      await loadPageData(selectedPageId, ++activeRequestId);
    }
  }

  function addMediaPath() {
    if (mediaInput.trim()) {
      postMediaPaths = [...postMediaPaths, mediaInput.trim()];
      mediaInput = '';
    }
  }

  function removeMediaPath(index: number) {
    postMediaPaths = postMediaPaths.filter((_, i) => i !== index);
  }

  async function handleCreatePost() {
    postError = null;
    postSuccess = null;
    if (!selectedPageId || !currentAccount?.channelAccountId) {
      postError = m.facebook_hub_err_no_page();
      return;
    }
    if (!postMessage.trim()) {
      postError = m.facebook_hub_err_empty_message();
      return;
    }

    isSubmittingPost = true;
    try {
      const scheduledPublishTime = isScheduled && scheduledDateTime
        ? new Date(scheduledDateTime).toISOString()
        : null;

      const postId = await scheduleFacebookPost({
        channel_account_id: currentAccount.channelAccountId,
        page_id: selectedPageId,
        post_type: postType,
        message: postMessage,
        link_url: postType === 'link' ? postLinkUrl : null,
        media_paths: postMediaPaths,
        scheduled_publish_time: scheduledPublishTime
      });

      if (!isScheduled) {
        await publishFacebookPostNow(postId);
        postSuccess = m.facebook_hub_msg_published_now();
      } else {
        postSuccess = m.facebook_hub_msg_scheduled_success();
      }

      postMessage = '';
      postLinkUrl = '';
      postMediaPaths = [];
      scheduledDateTime = '';
      const reqId = ++activeRequestId;
      await loadPageData(selectedPageId, reqId);
    } catch (e: any) {
      postError = e?.message || String(e) || m.facebook_hub_err_submit_failed();
    } finally {
      isSubmittingPost = false;
    }
  }

  async function handleCancelPost(id: number) {
    try {
      await cancelFacebookScheduledPost(id);
      await loadScheduledPosts();
    } catch (e) {
      alert(m.facebook_hub_err_cancel_failed({ error: errToString(e) }));
    }
  }

  async function handlePublishNow(id: number) {
    try {
      await publishFacebookPostNow(id);
      await loadScheduledPosts();
    } catch (e) {
      alert(m.facebook_hub_err_publish_failed({ error: errToString(e) }));
    }
  }

  function openCreateRuleModal() {
    editingRuleId = null;
    ruleName = '';
    rulePostIds = '';
    ruleKeywords = '';
    ruleMatchMode = 'contains';
    ruleAutoLike = true;
    rulePublicReply = m.facebook_hub_rule_modal_public_reply_placeholder({ name: '{{name}}' });
    rulePrivateReply = m.facebook_hub_rule_modal_private_reply_placeholder({ name: '{{name}}' });
    ruleIsEnabled = true;
    showRuleModal = true;
  }

  function openEditRuleModal(rule: CommentAutomationRule) {
    editingRuleId = rule.id;
    ruleName = rule.rule_name;
    rulePostIds = rule.post_ids.join(', ');
    ruleKeywords = rule.keywords.join(', ');
    ruleMatchMode = rule.match_mode;
    ruleAutoLike = rule.auto_like;
    rulePublicReply = rule.public_reply_template || '';
    rulePrivateReply = rule.private_reply_template || '';
    ruleIsEnabled = rule.is_enabled;
    showRuleModal = true;
  }

  async function handleSaveRule() {
    if (!selectedPageId || !currentAccount?.channelAccountId) {
      alert(m.facebook_hub_err_select_page_rule());
      return;
    }
    if (!ruleName.trim()) {
      alert(m.facebook_hub_err_rule_name_req());
      return;
    }

    const keywords = ruleKeywords
      .split(',')
      .map((k) => k.trim())
      .filter(Boolean);
    const postIds = rulePostIds
      .split(',')
      .map((p) => p.trim())
      .filter(Boolean);

    try {
      if (editingRuleId) {
        await updateCommentAutomation({
          id: editingRuleId,
          rule_name: ruleName,
          keywords,
          post_ids: postIds,
          match_mode: ruleMatchMode,
          auto_like: ruleAutoLike,
          public_reply_template: rulePublicReply || null,
          private_reply_template: rulePrivateReply || null,
          is_enabled: ruleIsEnabled
        });
      } else {
        await createCommentAutomation({
          channel_account_id: currentAccount.channelAccountId,
          page_id: selectedPageId,
          rule_name: ruleName,
          keywords,
          post_ids: postIds,
          match_mode: ruleMatchMode,
          auto_like: ruleAutoLike,
          public_reply_template: rulePublicReply || null,
          private_reply_template: rulePrivateReply || null,
          is_enabled: ruleIsEnabled
        });
      }
      showRuleModal = false;
      const reqId = ++activeRequestId;
      await loadPageData(selectedPageId, reqId);
    } catch (e) {
      alert(m.facebook_hub_err_save_rule({ error: errToString(e) }));
    }
  }

  async function handleDeleteRule(id: number) {
    if (confirm(m.facebook_hub_confirm_delete_rule())) {
      try {
        await deleteCommentAutomation(id);
        const reqId = ++activeRequestId;
        await loadPageData(selectedPageId, reqId);
      } catch (e) {
        alert(m.facebook_hub_err_delete_rule({ error: errToString(e) }));
      }
    }
  }

  function errToString(e: unknown): string {
    if (typeof e === 'string') return e;
    if (e instanceof Error) return e.message;
    return JSON.stringify(e);
  }
</script>

<div class="flex flex-col gap-6 w-full max-w-7xl mx-auto pb-12">
  {#if connectedAccounts.length === 0 && !isLoadingAccounts}
    <!-- Zero State Banner -->
    <div class="bg-card border border-border rounded-2xl p-8 shadow-xs flex flex-col items-center justify-center text-center gap-4 py-12">
      <div class="w-16 h-16 rounded-2xl bg-blue-600/10 text-blue-600 flex items-center justify-center font-bold text-2xl">
        f
      </div>
      <div class="max-w-md flex flex-col gap-1.5">
        <h3 class="text-base font-bold text-foreground">{m.facebook_hub_zero_title()}</h3>
        <p class="text-xs text-muted-foreground leading-relaxed">
          {m.facebook_hub_zero_desc()}
        </p>
      </div>
      <button
        type="button"
        onclick={() => onOpenSettings?.()}
        class="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-blue-600 text-white hover:bg-blue-700 text-xs font-semibold shadow-xs transition-colors"
      >
        <Settings2 class="w-4 h-4" />
        {m.facebook_hub_zero_goto_settings()}
      </button>
    </div>
  {:else}
    <!-- Page Header & Health Banner -->
    <div class="bg-card border border-border rounded-xl p-5 shadow-2xs flex flex-col md:flex-row md:items-center justify-between gap-4">
      <div class="flex items-center gap-4 min-w-0">
        {#if currentAccount?.avatarUrl}
          <img
            src={currentAccount.avatarUrl}
            alt={currentAccount.pageName || 'Fanpage'}
            class="w-12 h-12 rounded-xl object-cover border border-border shrink-0 shadow-2xs"
          />
        {:else}
          <div class="w-12 h-12 rounded-xl bg-blue-600 text-white flex items-center justify-center font-bold text-lg shrink-0 shadow-2xs">
            {currentAccount?.pageName ? currentAccount.pageName.charAt(0).toUpperCase() : 'F'}
          </div>
        {/if}
        <div class="min-w-0 flex flex-col gap-1">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="text-base font-bold text-foreground truncate">
              {currentAccount?.pageName || 'Facebook Marketing Hub'}
            </h2>
            {#if pageHealth?.token_status === 'valid' || currentAccount?.tokenStatus === 'valid'}
              <span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-semibold bg-emerald-500/10 text-emerald-600 border border-emerald-500/20">
                <CheckCircle2 class="w-3 h-3" /> {m.facebook_hub_token_active()}
              </span>
            {:else}
              <span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-semibold bg-amber-500/10 text-amber-600 border border-amber-500/20">
                <AlertTriangle class="w-3 h-3" /> {m.facebook_hub_token_invalid()}
              </span>
            {/if}
          </div>
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
            {#if connectedAccounts.length > 1}
              <div class="flex items-center gap-1.5">
                <span class="font-medium text-foreground">{m.facebook_hub_select_page()}</span>
                <select
                  aria-label={m.facebook_hub_select_page_aria()}
                  value={selectedPageId}
                  onchange={handlePageSelectChange}
                  class="px-2 py-0.5 rounded-md border border-input bg-background text-xs text-foreground font-medium focus:outline-hidden focus:ring-1 focus:ring-ring"
                >
                  {#each connectedAccounts as acc (acc.pageId)}
                    <option value={acc.pageId}>{acc.pageName || acc.pageId} (ID: {acc.pageId})</option>
                  {/each}
                </select>
              </div>
            {:else}
              <span>ID: <span class="font-mono text-foreground font-medium">{currentAccount?.pageId || selectedPageId}</span></span>
            {/if}
            <span>•</span>
            <span>{m.facebook_hub_active_automations({ count: pageHealth?.active_automations_count ?? (automationRules || []).filter(r => r?.is_enabled).length })}</span>
            <span>•</span>
            <span>{m.facebook_hub_pending_posts({ count: pageHealth?.active_scheduled_count ?? (scheduledPosts || []).filter(p => p?.status === 'queued' || p?.status === 'scheduled_on_meta').length })}</span>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        <button
          type="button"
          onclick={() => {
            loadAccounts();
          }}
          class="inline-flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-lg border border-input bg-background hover:bg-accent text-foreground transition-colors shadow-2xs"
          title={m.facebook_hub_refresh_status()}
        >
          <RefreshCw class="w-3.5 h-3.5 {isLoadingHealth || isLoadingAccounts ? 'animate-spin' : ''}" />
          {m.common_refresh()}
        </button>
        <button
          type="button"
          onclick={() => onOpenSettings?.()}
          class="inline-flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-lg border border-border bg-muted/40 hover:bg-muted text-foreground transition-colors shadow-2xs"
          title={m.facebook_hub_settings_title()}
        >
          <Settings2 class="w-3.5 h-3.5 text-muted-foreground" />
          {m.facebook_hub_settings_btn()}
        </button>
      </div>
    </div>
  {/if}

  <!-- Sub-Tabs Navigation -->
  <div class="flex border-b border-border gap-2">
    <button
      type="button"
      onclick={() => (activeSubTab = 'composer')}
      class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-semibold border-b-2 transition-all {activeSubTab === 'composer' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
    >
      <Calendar class="w-4 h-4" />
      {m.facebook_hub_tab_composer()}
    </button>
    <button
      type="button"
      onclick={() => (activeSubTab = 'automations')}
      class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-semibold border-b-2 transition-all {activeSubTab === 'automations' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
    >
      <Bot class="w-4 h-4" />
      {m.facebook_hub_tab_automations()}
    </button>
    <button
      type="button"
      onclick={() => (activeSubTab = 'compliance')}
      class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-semibold border-b-2 transition-all {activeSubTab === 'compliance' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
    >
      <ShieldCheck class="w-4 h-4" />
      {m.facebook_hub_tab_compliance()}
    </button>
  </div>

  <!-- Tab 1: Post Composer & Live Preview -->
  {#if activeSubTab === 'composer'}
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
      <!-- Composer Form (Left 7 Cols) -->
      <div class="lg:col-span-7 flex flex-col gap-6">
        <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-5">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-bold text-foreground">{m.facebook_hub_create_post_title()}</h3>
            <div class="flex items-center gap-1.5 bg-muted/60 p-1 rounded-lg border border-border">
              <button
                type="button"
                onclick={() => (postType = 'status')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'status' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_status()}
              </button>
              <button
                type="button"
                onclick={() => (postType = 'link')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'link' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_link()}
              </button>
              <button
                type="button"
                onclick={() => (postType = 'photo')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'photo' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_photo()}
              </button>
              <button
                type="button"
                onclick={() => (postType = 'multi_photo')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'multi_photo' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_multi_photo()}
              </button>
              <button
                type="button"
                onclick={() => (postType = 'video')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'video' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_video()}
              </button>
              <button
                type="button"
                onclick={() => (postType = 'reel')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'reel' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                {m.facebook_hub_post_type_reel()}
              </button>
            </div>
          </div>

          <!-- Message Textarea -->
          <div class="flex flex-col gap-1.5">
            <label for="post-msg-input" class="text-xs font-semibold text-foreground">{m.facebook_hub_post_content_label()}</label>
            <textarea
              id="post-msg-input"
              bind:value={postMessage}
              oninput={(e) => {
                const target = (e.target || e.currentTarget) as HTMLTextAreaElement;
                if (target) postMessage = target.value;
              }}
              rows="4"
              placeholder={m.facebook_hub_post_content_placeholder()}
              class="w-full rounded-lg border border-input bg-background p-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            ></textarea>
          </div>

          <!-- Link URL Input (If link type) -->
          {#if postType === 'link'}
            <div class="flex flex-col gap-1.5">
              <label for="post-link-input" class="text-xs font-semibold text-foreground">{m.facebook_hub_link_url_label()}</label>
              <div class="relative">
                <LinkIcon class="w-4 h-4 absolute left-3 top-3 text-muted-foreground" />
                <input
                  id="post-link-input"
                  type="url"
                  bind:value={postLinkUrl}
                  placeholder={m.facebook_hub_link_url_placeholder()}
                  class="w-full pl-9 pr-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                />
              </div>
            </div>
          {/if}

          <!-- Media Paths / URLs (If photo/multi/video/reel) -->
          {#if postType !== 'status' && postType !== 'link'}
            <div class="flex flex-col gap-2">
              <label for="post-media-input" class="text-xs font-semibold text-foreground">{m.facebook_hub_media_label()}</label>
              <div class="flex gap-2">
                <input
                  id="post-media-input"
                  type="text"
                  bind:value={mediaInput}
                  placeholder={m.facebook_hub_media_placeholder()}
                  class="flex-1 px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                  onkeydown={(e) => e.key === 'Enter' && addMediaPath()}
                />
                <button
                  type="button"
                  onclick={addMediaPath}
                  class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-medium rounded-lg hover:bg-secondary/80"
                >
                  {m.facebook_hub_add_media_btn()}
                </button>
              </div>

              {#if postMediaPaths.length > 0}
                <div class="flex flex-wrap gap-2 mt-1">
                  {#each postMediaPaths as path, index (index)}
                    <div class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-muted text-xs text-foreground border border-border">
                      <span class="max-w-[200px] truncate">{path}</span>
                      <button
                        type="button"
                        onclick={() => removeMediaPath(index)}
                        class="text-muted-foreground hover:text-destructive"
                      >
                        <X class="w-3 h-3" />
                      </button>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}

          <!-- Scheduling Options -->
          <div class="border-t border-border pt-4 flex flex-col gap-3">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="schedule-toggle"
                  bind:checked={isScheduled}
                  class="rounded border-input text-blue-600 focus:ring-blue-500"
                />
                <label for="schedule-toggle" class="text-sm font-semibold text-foreground cursor-pointer">
                  {m.facebook_hub_schedule_toggle()}
                </label>
              </div>
              {#if isScheduled}
                <span class="text-xs text-muted-foreground">
                  {m.facebook_hub_schedule_hint()}
                </span>
              {/if}
            </div>

            {#if isScheduled}
              <div class="flex flex-col gap-1.5">
                <label for="schedule-dt-input" class="text-xs font-semibold text-foreground">{m.facebook_hub_target_datetime()}</label>
                <input
                  id="schedule-dt-input"
                  type="datetime-local"
                  bind:value={scheduledDateTime}
                  class="w-full px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                />
              </div>
            {/if}
          </div>

          {#if postError}
            <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
              <AlertTriangle class="w-4 h-4 shrink-0" />
              <span>{postError}</span>
            </div>
          {/if}
          {#if postSuccess}
            <div class="p-3 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 text-xs flex items-center gap-2">
              <CheckCircle2 class="w-4 h-4 shrink-0" />
              <span>{postSuccess}</span>
            </div>
          {/if}

          <!-- Submit Button -->
          <button
            type="button"
            onclick={() => handleCreatePost()}
            disabled={isSubmittingPost}
            class="w-full py-2.5 px-4 rounded-lg bg-blue-600 text-white font-semibold text-sm hover:bg-blue-700 disabled:opacity-50 transition-colors flex items-center justify-center gap-2 shadow-sm"
          >
            {#if isSubmittingPost}
              <RefreshCw class="w-4 h-4 animate-spin" />
              {m.facebook_hub_submitting()}
            {:else if isScheduled}
              <Clock class="w-4 h-4" />
              {m.facebook_hub_schedule_btn()}
            {:else}
              <Send class="w-4 h-4" />
              {m.facebook_hub_publish_now_btn()}
            {/if}
          </button>
        </div>
      </div>

      <!-- Live Feed Preview (Right 5 Cols) -->
      <div class="lg:col-span-5 flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <h3 class="text-sm font-bold text-foreground">{m.facebook_hub_mockup_title()}</h3>
          <div class="flex items-center gap-1 bg-muted p-1 rounded-lg border border-border">
            <button
              type="button"
              onclick={() => (previewDevice = 'desktop')}
              class="p-1.5 rounded-md transition-colors {previewDevice === 'desktop' ? 'bg-background text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground'}"
              title={m.facebook_hub_view_desktop()}
            >
              <Monitor class="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onclick={() => (previewDevice = 'mobile')}
              class="p-1.5 rounded-md transition-colors {previewDevice === 'mobile' ? 'bg-background text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground'}"
              title={m.facebook_hub_view_mobile()}
            >
              <Smartphone class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        <!-- Feed Card Mockup -->
        <div class="bg-card border border-border rounded-xl shadow-sm overflow-hidden {previewDevice === 'mobile' ? 'max-w-sm mx-auto' : 'w-full'}">
          <!-- Post Author Header -->
          <div class="p-4 flex items-center gap-3">
            {#if currentAccount?.avatarUrl}
              <img
                src={currentAccount.avatarUrl}
                alt={currentAccount.pageName || 'Fanpage'}
                class="w-10 h-10 rounded-full object-cover shadow-2xs border border-border shrink-0"
              />
            {:else}
              <div class="w-10 h-10 rounded-full bg-blue-600 text-white font-bold flex items-center justify-center text-sm shadow-2xs shrink-0">
                {currentAccount?.pageName ? currentAccount.pageName.charAt(0).toUpperCase() : 'RS'}
              </div>
            {/if}
            <div>
              <div class="font-bold text-sm text-foreground flex items-center gap-1">
                {currentAccount?.pageName || 'Facebook Fanpage'}
                <span class="text-blue-500 text-xs">✓</span>
              </div>
              <div class="text-[11px] text-muted-foreground flex items-center gap-1">
                <span>{m.common_just_now()}</span> • <span>{m.facebook_hub_sponsored()}</span> • <span>🌐</span>
              </div>
            </div>
          </div>

          <!-- Post Content Text -->
          <div class="px-4 pb-3 text-sm text-foreground whitespace-pre-wrap">
            {postMessage || m.facebook_hub_preview_placeholder()}
          </div>

          <!-- Media / Link Preview Placeholder -->
          {#if postType === 'link'}
            {@const linkHostname = (() => {
              if (!postLinkUrl) return 'EXAMPLE.COM';
              try {
                return new URL(postLinkUrl).hostname || 'EXAMPLE.COM';
              } catch {
                return 'EXAMPLE.COM';
              }
            })()}
            <div class="border-t border-border bg-muted/40 p-4">
              <div class="text-xs font-semibold text-blue-600 uppercase tracking-wider truncate">
                {linkHostname}
              </div>
              <div class="font-bold text-sm text-foreground mt-0.5">
                {m.facebook_hub_preview_offer_title()}
              </div>
              <div class="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                {m.facebook_hub_preview_offer_desc()}
              </div>
            </div>
          {:else if postType === 'photo' || postType === 'multi_photo'}
            <div class="bg-muted/60 aspect-video flex flex-col items-center justify-center text-muted-foreground border-y border-border">
              <Image class="w-10 h-10 stroke-[1.5]" />
              <span class="text-xs mt-1 font-medium">
                {postMediaPaths.length > 0 ? m.facebook_hub_preview_photos_count({ count: postMediaPaths.length }) : m.facebook_hub_preview_photo_placeholder()}
              </span>
            </div>
          {:else if postType === 'video'}
            <div class="bg-slate-950 aspect-video flex flex-col items-center justify-center text-slate-400 border-y border-border">
              <Video class="w-12 h-12 stroke-[1.5] text-white/80" />
              <span class="text-xs mt-1 font-medium text-white/70">{m.facebook_hub_preview_video_placeholder()}</span>
            </div>
          {:else if postType === 'reel'}
            <div class="bg-slate-950 aspect-[9/16] max-h-72 mx-auto flex flex-col items-center justify-center text-slate-400 border-y border-border rounded-lg my-2">
              <Film class="w-10 h-10 stroke-[1.5] text-white/80" />
              <span class="text-xs mt-1 font-medium text-white/70">{m.facebook_hub_preview_reel_placeholder()}</span>
            </div>
          {/if}

          <!-- Post Actions Bar -->
          <div class="px-4 py-2.5 border-t border-border flex items-center justify-between text-muted-foreground text-xs font-medium">
            <button type="button" class="flex items-center gap-1.5 hover:text-blue-600 transition-colors">
              <ThumbsUp class="w-4 h-4" /> {m.facebook_hub_like_btn()}
            </button>
            <button type="button" class="flex items-center gap-1.5 hover:text-foreground transition-colors">
              <MessageCircle class="w-4 h-4" /> {m.facebook_hub_comment_btn()}
            </button>
            <button type="button" class="flex items-center gap-1.5 hover:text-foreground transition-colors">
              <Share2 class="w-4 h-4" /> {m.facebook_hub_share_btn()}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Scheduled Posts Management Table -->
    <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-4 mt-6">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-base font-bold text-foreground">{m.facebook_hub_scheduled_posts_title()}</h3>
          <p class="text-xs text-muted-foreground">{m.facebook_hub_scheduled_posts_desc()}</p>
        </div>
        <button
          type="button"
          onclick={loadScheduledPosts}
          class="p-2 rounded-lg border border-input hover:bg-accent text-foreground transition-colors"
        >
          <RefreshCw class="w-4 h-4 {isLoadingPosts ? 'animate-spin' : ''}" />
        </button>
      </div>

      <div class="overflow-x-auto">
        <table class="w-full text-left text-xs border-collapse">
          <thead>
            <tr class="border-b border-border text-muted-foreground font-semibold">
              <th class="py-2.5 px-3">{m.facebook_hub_col_id()}</th>
              <th class="py-2.5 px-3">{m.facebook_hub_col_type()}</th>
              <th class="py-2.5 px-3">{m.facebook_hub_col_excerpt()}</th>
              <th class="py-2.5 px-3">{m.facebook_hub_col_mode()}</th>
              <th class="py-2.5 px-3">{m.facebook_hub_col_target_time()}</th>
              <th class="py-2.5 px-3">{m.facebook_hub_col_status()}</th>
              <th class="py-2.5 px-3 text-right">{m.facebook_hub_col_actions()}</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            {#if scheduledPosts.length === 0}
              <tr>
                <td colspan="7" class="py-8 text-center text-muted-foreground">
                  {m.facebook_hub_no_posts()}
                </td>
              </tr>
            {:else}
              {#each scheduledPosts as post (post.id)}
                <tr class="hover:bg-muted/30 transition-colors">
                  <td class="py-2.5 px-3 font-mono text-[11px] text-muted-foreground">{post.id}</td>
                  <td class="py-2.5 px-3">
                    <span class="px-2 py-0.5 rounded-md bg-muted text-foreground font-semibold uppercase text-[10px]">
                      {post.post_type}
                    </span>
                  </td>
                  <td class="py-2.5 px-3 max-w-[280px] truncate text-foreground font-medium">
                    {post.message}
                  </td>
                  <td class="py-2.5 px-3 font-mono text-[11px] text-muted-foreground">
                    {post.scheduling_mode}
                  </td>
                  <td class="py-2.5 px-3 text-muted-foreground">
                    {post.scheduled_publish_time ? new Date(post.scheduled_publish_time).toLocaleString() : m.facebook_hub_time_immediate()}
                  </td>
                  <td class="py-2.5 px-3">
                    {#if post.status === 'published'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/10 text-emerald-600 border border-emerald-500/20">
                        {m.facebook_hub_status_published()}
                      </span>
                    {:else if post.status === 'scheduled_on_meta'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-blue-500/10 text-blue-600 border border-blue-500/20">
                        {m.facebook_hub_status_scheduled_meta()}
                      </span>
                    {:else if post.status === 'queued'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-amber-500/10 text-amber-600 border border-amber-500/20">
                        {m.facebook_hub_status_queued()}
                      </span>
                    {:else if post.status === 'failed'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-destructive/10 text-destructive border border-destructive/20" title={post.error_message || ''}>
                        {m.facebook_hub_status_failed()}
                      </span>
                    {:else}
                      <span class="px-2 py-0.5 rounded-md bg-muted text-muted-foreground text-[10px]">
                        {post.status}
                      </span>
                    {/if}
                  </td>
                  <td class="py-2.5 px-3 text-right">
                    <div class="inline-flex items-center gap-1.5">
                      {#if post.status === 'queued' || post.status === 'draft'}
                        <button
                          type="button"
                          onclick={() => handlePublishNow(post.id)}
                          class="px-2 py-1 bg-blue-600 text-white rounded text-[11px] font-medium hover:bg-blue-700"
                        >
                          {m.facebook_hub_publish_now_btn()}
                        </button>
                        <button
                          type="button"
                          onclick={() => handleCancelPost(post.id)}
                          class="px-2 py-1 bg-destructive/10 text-destructive rounded text-[11px] font-medium hover:bg-destructive/20"
                        >
                          {m.common_cancel()}
                        </button>
                      {/if}
                    </div>
                  </td>
                </tr>
              {/each}
            {/if}
          </tbody>
        </table>
      </div>
    </div>
  {/if}

  <!-- Tab 2: Comment Automations -->
  {#if activeSubTab === 'automations'}
    <div class="flex flex-col gap-6">
      <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <div>
            <h3 class="text-base font-bold text-foreground">{m.facebook_hub_automations_title()}</h3>
            <p class="text-xs text-muted-foreground">{m.facebook_hub_automations_desc()}</p>
          </div>
          <button
            type="button"
            onclick={openCreateRuleModal}
            class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-blue-600 text-white font-semibold text-xs hover:bg-blue-700 transition-colors shadow-2xs"
          >
            <Plus class="w-4 h-4" />
            {m.facebook_hub_add_rule_btn()}
          </button>
        </div>

        <div class="overflow-x-auto">
          <table class="w-full text-left text-xs border-collapse">
            <thead>
              <tr class="border-b border-border text-muted-foreground font-semibold">
                <th class="py-2.5 px-3">{m.facebook_hub_rule_name_col()}</th>
                <th class="py-2.5 px-3">{m.facebook_hub_rule_keywords_col()}</th>
                <th class="py-2.5 px-3">{m.facebook_hub_rule_match_mode_col()}</th>
                <th class="py-2.5 px-3">{m.facebook_hub_rule_auto_like_col()}</th>
                <th class="py-2.5 px-3">{m.facebook_hub_rule_private_reply_col()}</th>
                <th class="py-2.5 px-3">{m.facebook_hub_rule_status_col()}</th>
                <th class="py-2.5 px-3 text-right">{m.facebook_hub_col_actions()}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              {#if automationRules.length === 0}
                <tr>
                  <td colspan="7" class="py-8 text-center text-muted-foreground">
                    {m.facebook_hub_no_rules()}
                  </td>
                </tr>
              {:else}
                {#each automationRules as rule (rule.id)}
                  <tr class="hover:bg-muted/30 transition-colors">
                    <td class="py-2.5 px-3 font-semibold text-foreground">{rule.rule_name}</td>
                    <td class="py-2.5 px-3">
                      <div class="flex flex-wrap gap-1">
                        {#each rule.keywords as kw (kw)}
                          <span class="px-1.5 py-0.5 rounded bg-muted text-[10px] text-foreground font-mono">{kw}</span>
                        {/each}
                      </div>
                    </td>
                    <td class="py-2.5 px-3 uppercase text-[10px] font-mono text-muted-foreground">{rule.match_mode}</td>
                    <td class="py-2.5 px-3">
                      {#if rule.auto_like}
                        <Check class="w-4 h-4 text-emerald-500" />
                      {:else}
                        <X class="w-4 h-4 text-muted-foreground" />
                      {/if}
                    </td>
                    <td class="py-2.5 px-3 max-w-[200px] truncate text-muted-foreground">
                      {rule.private_reply_template || m.facebook_hub_rule_none()}
                    </td>
                    <td class="py-2.5 px-3">
                      {#if rule.is_enabled}
                        <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/10 text-emerald-600">{m.facebook_hub_rule_enabled()}</span>
                      {:else}
                        <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-muted text-muted-foreground">{m.facebook_hub_rule_disabled()}</span>
                      {/if}
                    </td>
                    <td class="py-2.5 px-3 text-right">
                      <div class="inline-flex items-center gap-1">
                        <button
                          type="button"
                          onclick={() => openEditRuleModal(rule)}
                          class="px-2 py-1 rounded hover:bg-accent text-xs text-foreground font-medium"
                        >
                          {m.common_edit()}
                        </button>
                        <button
                          type="button"
                          onclick={() => handleDeleteRule(rule.id)}
                          class="p-1 rounded hover:bg-destructive/10 text-destructive"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>
      </div>

      <!-- Activity Logs -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <div>
            <h3 class="text-base font-bold text-foreground">{m.facebook_hub_audit_logs_title()}</h3>
            <p class="text-xs text-muted-foreground">{m.facebook_hub_audit_logs_desc()}</p>
          </div>
          <button
            type="button"
            onclick={loadAutomations}
            class="p-2 rounded-lg border border-input hover:bg-accent text-foreground transition-colors"
          >
            <RefreshCw class="w-4 h-4 {isLoadingAutomations ? 'animate-spin' : ''}" />
          </button>
        </div>

        <div class="overflow-x-auto">
          <table class="w-full text-left text-xs border-collapse">
            <thead>
              <tr class="border-b border-border text-muted-foreground font-semibold">
                <th class="py-2 px-3">{m.facebook_hub_log_col_comment_id()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_sender()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_comment()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_like()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_public_reply()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_private_send()}</th>
                <th class="py-2 px-3">{m.facebook_hub_log_col_time()}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border font-mono text-[11px]">
              {#if commentLogs.length === 0}
                <tr>
                  <td colspan="7" class="py-6 text-center text-muted-foreground font-sans text-xs">
                    {m.facebook_hub_no_logs()}
                  </td>
                </tr>
              {:else}
                {#each commentLogs as log (log.id ?? log.comment_id)}
                  <tr class="hover:bg-muted/30 transition-colors">
                    <td class="py-2 px-3 text-muted-foreground">{log.comment_id}</td>
                    <td class="py-2 px-3 font-sans font-medium text-foreground">{log.sender_name || log.sender_psid}</td>
                    <td class="py-2 px-3 font-sans max-w-[220px] truncate text-foreground">{log.comment_text}</td>
                    <td class="py-2 px-3">
                      <span class="px-1.5 py-0.5 rounded text-[10px] {log.like_status === 'success' ? 'bg-emerald-500/10 text-emerald-600 font-bold' : 'text-muted-foreground'}">
                        {log.like_status}
                      </span>
                    </td>
                    <td class="py-2 px-3">
                      <span class="px-1.5 py-0.5 rounded text-[10px] {log.public_reply_status === 'success' ? 'bg-emerald-500/10 text-emerald-600 font-bold' : 'text-muted-foreground'}">
                        {log.public_reply_status}
                      </span>
                    </td>
                    <td class="py-2 px-3">
                      <span class="px-1.5 py-0.5 rounded text-[10px] {log.private_reply_status === 'success' ? 'bg-emerald-500/10 text-emerald-600 font-bold' : log.private_reply_status === 'failed' ? 'bg-destructive/10 text-destructive font-bold' : 'bg-amber-500/10 text-amber-600'}">
                        {log.private_reply_status}
                      </span>
                    </td>
                    <td class="py-2 px-3 text-muted-foreground font-sans text-[11px]">{new Date(log.created_at).toLocaleTimeString()}</td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  {/if}

  <!-- Tab 3: Compliance & Multi-Device -->
  {#if activeSubTab === 'compliance'}
    <div class="flex flex-col gap-6 max-w-4xl">
      <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-4">
        <h3 class="text-base font-bold text-foreground">{m.facebook_hub_compliance_title()}</h3>
        <p class="text-xs text-muted-foreground leading-relaxed">
          {m.facebook_hub_compliance_desc()}
        </p>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-2">
          <div class="p-4 rounded-lg bg-muted/40 border border-border flex flex-col gap-2">
            <div class="flex items-center gap-2 font-semibold text-xs text-foreground">
              <ShieldCheck class="w-4 h-4 text-emerald-500" />
              {m.facebook_hub_compliance_hmac_title()}
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">
              {m.facebook_hub_compliance_hmac_desc()}
            </p>
          </div>

          <div class="p-4 rounded-lg bg-muted/40 border border-border flex flex-col gap-2">
            <div class="flex items-center gap-2 font-semibold text-xs text-foreground">
              <RefreshCw class="w-4 h-4 text-blue-500" />
              {m.facebook_hub_compliance_fanout_title()}
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">
              {m.facebook_hub_compliance_fanout_desc()}
            </p>
          </div>
        </div>

        <div class="border-t border-border pt-4 mt-2 flex flex-col gap-3">
          <h4 class="text-xs font-bold text-foreground">{m.facebook_hub_compliance_endpoint_title()}</h4>
          <p class="text-xs text-muted-foreground">
            {m.facebook_hub_compliance_endpoint_desc()}
          </p>
          <div class="p-2.5 rounded-lg bg-muted font-mono text-xs text-foreground select-all border border-border">
            https://&lt;project&gt;.supabase.co/functions/v1/meta-data-deletion/deletion-status?code=&lt;CONFIRMATION_CODE&gt;
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<!-- Rule Creation / Edit Modal -->
{#if showRuleModal}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-fade-in">
    <div class="bg-card border border-border rounded-xl shadow-xl w-full max-w-lg p-6 flex flex-col gap-5">
      <div class="flex items-center justify-between">
        <h3 class="text-base font-bold text-foreground">
          {editingRuleId ? m.facebook_hub_rule_modal_edit_title() : m.facebook_hub_rule_modal_create_title()}
        </h3>
        <button
          type="button"
          onclick={() => (showRuleModal = false)}
          class="text-muted-foreground hover:text-foreground"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex flex-col gap-4 text-xs">
        <div class="flex flex-col gap-1.5">
          <label for="rule-name-input" class="font-semibold text-foreground">{m.facebook_hub_rule_modal_name_label()}</label>
          <input
            id="rule-name-input"
            type="text"
            bind:value={ruleName}
            placeholder={m.facebook_hub_rule_modal_name_placeholder()}
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="rule-keywords-input" class="font-semibold text-foreground">{m.facebook_hub_rule_modal_keywords_label()}</label>
          <input
            id="rule-keywords-input"
            type="text"
            bind:value={ruleKeywords}
            placeholder={m.facebook_hub_rule_modal_keywords_placeholder()}
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="flex flex-col gap-1.5">
            <label for="rule-match-mode-select" class="font-semibold text-foreground">{m.facebook_hub_rule_modal_match_mode_label()}</label>
            <select
              id="rule-match-mode-select"
              bind:value={ruleMatchMode}
              class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            >
              <option value="contains">{m.facebook_hub_rule_match_contains()}</option>
              <option value="all">{m.facebook_hub_rule_match_all()}</option>
              <option value="exact">{m.facebook_hub_rule_match_exact()}</option>
              <option value="regex">{m.facebook_hub_rule_match_regex()}</option>
            </select>
          </div>

          <div class="flex items-center gap-2 pt-6">
            <input
              type="checkbox"
              id="auto-like-check"
              bind:checked={ruleAutoLike}
              class="rounded border-input text-blue-600 focus:ring-blue-500"
            />
            <label for="auto-like-check" class="font-semibold text-foreground cursor-pointer">
              {m.facebook_hub_rule_modal_auto_like_label()}
            </label>
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <label for="rule-public-reply-input" class="font-semibold text-foreground">{m.facebook_hub_rule_modal_public_reply_label()}</label>
            <span class="text-[10px] text-blue-600 bg-blue-500/10 px-1.5 py-0.5 rounded font-mono">{m.facebook_hub_rule_modal_use_name_hint({ tag: '{{name}}' })}</span>
          </div>
          <textarea
            id="rule-public-reply-input"
            bind:value={rulePublicReply}
            rows="2"
            placeholder={m.facebook_hub_rule_modal_public_reply_placeholder({ name: '{{name}}' })}
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          ></textarea>
        </div>

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <label for="rule-private-reply-input" class="font-semibold text-foreground">{m.facebook_hub_rule_modal_private_reply_label()}</label>
            <span class="text-[10px] text-blue-600 bg-blue-500/10 px-1.5 py-0.5 rounded font-mono">{m.facebook_hub_rule_modal_use_name_hint({ tag: '{{name}}' })}</span>
          </div>
          <textarea
            id="rule-private-reply-input"
            bind:value={rulePrivateReply}
            rows="3"
            placeholder={m.facebook_hub_rule_modal_private_reply_placeholder({ name: '{{name}}' })}
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          ></textarea>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 border-t border-border pt-4">
        <button
          type="button"
          onclick={() => (showRuleModal = false)}
          class="px-4 py-2 rounded-lg border border-input hover:bg-accent text-foreground text-xs font-semibold"
        >
          {m.common_cancel()}
        </button>
        <button
          type="button"
          onclick={handleSaveRule}
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 text-xs font-semibold"
        >
          {m.facebook_hub_rule_modal_save_btn()}
        </button>
      </div>
    </div>
  </div>
{/if}
