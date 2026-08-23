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

  let prevTab = '';
  $effect(() => {
    if (activeTab === 'facebook_hub' && prevTab !== 'facebook_hub') {
      prevTab = activeTab;
      loadAccounts();
    } else if (activeTab !== 'facebook_hub') {
      prevTab = activeTab;
    }
  });

  async function loadAccounts() {
    if (isLoadingAccounts) return;
    isLoadingAccounts = true;
    try {
      connectedAccounts = await listManagedFacebookAccounts();
      const savedPageId = localStorage.getItem('rustsale_facebook_hub_selected_page_id');
      if (savedPageId && connectedAccounts.some((a) => a.pageId === savedPageId)) {
        selectedPageId = savedPageId;
      } else if (connectedAccounts.length > 0) {
        selectedPageId = connectedAccounts[0].pageId;
        localStorage.setItem('rustsale_facebook_hub_selected_page_id', selectedPageId);
      } else {
        selectedPageId = '';
      }

      if (selectedPageId) {
        await Promise.all([
          loadPageHealth(),
          loadScheduledPosts(),
          loadAutomations()
        ]);
      } else {
        pageHealth = null;
        scheduledPosts = [];
        automationRules = [];
        commentLogs = [];
      }
    } catch (e) {
      console.error('Failed to load connected facebook accounts:', e);
      connectedAccounts = [];
      selectedPageId = '';
    } finally {
      isLoadingAccounts = false;
    }
  }

  async function handlePageSelect(pageId: string) {
    selectedPageId = pageId;
    localStorage.setItem('rustsale_facebook_hub_selected_page_id', selectedPageId);
    await Promise.all([
      loadPageHealth(),
      loadScheduledPosts(),
      loadAutomations()
    ]);
  }

  function handlePageSelectChange(e: Event) {
    const target = (e.target || e.currentTarget) as HTMLSelectElement;
    if (target?.value) {
      handlePageSelect(target.value);
    }
  }

  async function loadPageHealth() {
    if (!selectedPageId) return;
    isLoadingHealth = true;
    try {
      pageHealth = await getFacebookPageHealth(selectedPageId);
    } catch (e) {
      console.error('Failed to load page health:', e);
      pageHealth = null;
    } finally {
      isLoadingHealth = false;
    }
  }

  async function loadScheduledPosts() {
    isLoadingPosts = true;
    try {
      const res = await listFacebookScheduledPosts(selectedPageId || undefined);
      scheduledPosts = Array.isArray(res) ? res : [];
    } catch (e) {
      console.error('Failed to load scheduled posts:', e);
      scheduledPosts = [];
    } finally {
      isLoadingPosts = false;
    }
  }

  async function loadAutomations() {
    isLoadingAutomations = true;
    try {
      const rules = await listCommentAutomations(selectedPageId || undefined);
      automationRules = Array.isArray(rules) ? rules : [];
      const logs = await listCommentLogs(selectedPageId || undefined, 50);
      commentLogs = Array.isArray(logs) ? logs : [];
    } catch (e) {
      console.error('Failed to load automations:', e);
      automationRules = [];
      commentLogs = [];
    } finally {
      isLoadingAutomations = false;
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
    if (!selectedPageId) {
      postError = 'Vui lòng kết nối và chọn Fanpage Facebook trước khi đăng bài.';
      return;
    }
    if (!postMessage.trim()) {
      postError = 'Message content cannot be empty';
      return;
    }

    isSubmittingPost = true;
    try {
      const scheduledPublishTime = isScheduled && scheduledDateTime
        ? new Date(scheduledDateTime).toISOString()
        : null;

      const postId = await scheduleFacebookPost({
        channel_account_id: currentAccount?.channelAccountId || 1,
        page_id: selectedPageId,
        post_type: postType,
        message: postMessage,
        link_url: postType === 'link' ? postLinkUrl : null,
        media_paths: postMediaPaths,
        scheduled_publish_time: scheduledPublishTime
      });

      if (!isScheduled) {
        await publishFacebookPostNow(postId);
        postSuccess = 'Post submitted for immediate publishing!';
      } else {
        postSuccess = `Post successfully scheduled (ID: ${postId})`;
      }

      // Reset form
      postMessage = '';
      postLinkUrl = '';
      postMediaPaths = [];
      isScheduled = false;
      scheduledDateTime = '';
      await loadScheduledPosts();
    } catch (e) {
      postError = errToString(e);
    } finally {
      isSubmittingPost = false;
    }
  }

  async function handleCancelPost(id: number) {
    try {
      await cancelFacebookScheduledPost(id);
      await loadScheduledPosts();
    } catch (e) {
      alert(`Failed to cancel: ${errToString(e)}`);
    }
  }

  async function handlePublishNow(id: number) {
    try {
      await publishFacebookPostNow(id);
      await loadScheduledPosts();
    } catch (e) {
      alert(`Failed to publish: ${errToString(e)}`);
    }
  }

  function openCreateRuleModal() {
    editingRuleId = null;
    ruleName = '';
    rulePostIds = '';
    ruleKeywords = '';
    ruleMatchMode = 'contains';
    ruleAutoLike = true;
    rulePublicReply = 'Chào {{name}}, cảm ơn bạn đã quan tâm!';
    rulePrivateReply = 'Chào {{name}}, shop đã gửi thông tin chi tiết qua tin nhắn cho bạn ạ!';
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
    if (!selectedPageId) {
      alert('Vui lòng kết nối và chọn Fanpage Facebook trước khi lưu quy tắc.');
      return;
    }
    if (!ruleName.trim()) {
      alert('Rule name is required');
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
          channel_account_id: currentAccount?.channelAccountId || 1,
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
      await loadAutomations();
    } catch (e) {
      alert(`Failed to save rule: ${errToString(e)}`);
    }
  }

  async function handleDeleteRule(id: number) {
    if (confirm('Are you sure you want to delete this automation rule?')) {
      try {
        await deleteCommentAutomation(id);
        await loadAutomations();
      } catch (e) {
        alert(`Failed to delete rule: ${errToString(e)}`);
      }
    }
  }

  function errToString(e: unknown): string {
    if (typeof e === 'string') return e;
    if (e instanceof Error) return e.message;
    return JSON.stringify(e);
  }

  onMount(() => {
    loadAccounts();
  });
</script>

<div class="flex flex-col gap-6 w-full max-w-7xl mx-auto pb-12">
  {#if connectedAccounts.length === 0 && !isLoadingAccounts}
    <!-- Zero State Banner -->
    <div class="bg-card border border-border rounded-2xl p-8 shadow-xs flex flex-col items-center justify-center text-center gap-4 py-12">
      <div class="w-16 h-16 rounded-2xl bg-blue-600/10 text-blue-600 flex items-center justify-center font-bold text-2xl">
        f
      </div>
      <div class="max-w-md flex flex-col gap-1.5">
        <h3 class="text-base font-bold text-foreground">Chưa kết nối Facebook Fanpage</h3>
        <p class="text-xs text-muted-foreground leading-relaxed">
          Kết nối Fanpage của bạn trong mục Cài đặt Hệ thống để quản lý bài viết, lên lịch tiếp thị và kích hoạt kịch bản trả lời bình luận tự động.
        </p>
      </div>
      <button
        type="button"
        onclick={() => onOpenSettings?.()}
        class="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-blue-600 text-white hover:bg-blue-700 text-xs font-semibold shadow-xs transition-colors"
      >
        <Settings2 class="w-4 h-4" />
        Đi tới Cài đặt & Kết nối Fanpage
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
                <CheckCircle2 class="w-3 h-3" /> Token Active (Keyring Vault)
              </span>
            {:else}
              <span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-semibold bg-amber-500/10 text-amber-600 border border-amber-500/20">
                <AlertTriangle class="w-3 h-3" /> Token Missing / Expired
              </span>
            {/if}
          </div>
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
            {#if connectedAccounts.length > 1}
              <div class="flex items-center gap-1.5">
                <span class="font-medium text-foreground">Chọn Page:</span>
                <select
                  aria-label="Chọn Fanpage Facebook"
                  value={selectedPageId}
                  onchange={handlePageSelectChange}
                  class="px-2 py-0.5 rounded-md border border-input bg-background text-xs text-foreground font-medium focus:outline-hidden focus:ring-1 focus:ring-ring"
                >
                  {#each connectedAccounts as acc}
                    <option value={acc.pageId}>{acc.pageName || acc.pageId} (ID: {acc.pageId})</option>
                  {/each}
                </select>
              </div>
            {:else}
              <span>ID: <span class="font-mono text-foreground font-medium">{currentAccount?.pageId || selectedPageId}</span></span>
            {/if}
            <span>•</span>
            <span>Active Automations: <span class="font-semibold text-foreground">{pageHealth?.active_automations_count ?? (automationRules || []).filter(r => r?.is_enabled).length}</span></span>
            <span>•</span>
            <span>Pending Posts: <span class="font-semibold text-foreground">{pageHealth?.active_scheduled_count ?? (scheduledPosts || []).filter(p => p?.status === 'queued' || p?.status === 'scheduled_on_meta').length}</span></span>
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
          title="Làm mới trạng thái"
        >
          <RefreshCw class="w-3.5 h-3.5 {isLoadingHealth || isLoadingAccounts ? 'animate-spin' : ''}" />
          Refresh
        </button>
        <button
          type="button"
          onclick={() => onOpenSettings?.()}
          class="inline-flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-lg border border-border bg-muted/40 hover:bg-muted text-foreground transition-colors shadow-2xs"
          title="Quản lý tài khoản Facebook trong Cài đặt"
        >
          <Settings2 class="w-3.5 h-3.5 text-muted-foreground" />
          Settings
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
      Post Composer & Scheduler
    </button>
    <button
      type="button"
      onclick={() => (activeSubTab = 'automations')}
      class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-semibold border-b-2 transition-all {activeSubTab === 'automations' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
    >
      <Bot class="w-4 h-4" />
      Comment Automations
    </button>
    <button
      type="button"
      onclick={() => (activeSubTab = 'compliance')}
      class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-semibold border-b-2 transition-all {activeSubTab === 'compliance' ? 'border-blue-600 text-blue-600' : 'border-transparent text-muted-foreground hover:text-foreground'}"
    >
      <ShieldCheck class="w-4 h-4" />
      Compliance & Multi-Device
    </button>
  </div>

  <!-- Tab 1: Post Composer & Live Preview -->
  {#if activeSubTab === 'composer'}
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
      <!-- Composer Form (Left 7 Cols) -->
      <div class="lg:col-span-7 flex flex-col gap-6">
        <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-5">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-bold text-foreground">Create Facebook Post</h3>
            <div class="flex items-center gap-1.5 bg-muted/60 p-1 rounded-lg border border-border">
              <button
                type="button"
                onclick={() => (postType = 'status')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'status' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Status
              </button>
              <button
                type="button"
                onclick={() => (postType = 'link')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'link' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Link
              </button>
              <button
                type="button"
                onclick={() => (postType = 'photo')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'photo' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Photo
              </button>
              <button
                type="button"
                onclick={() => (postType = 'multi_photo')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'multi_photo' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Multi
              </button>
              <button
                type="button"
                onclick={() => (postType = 'video')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'video' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Video
              </button>
              <button
                type="button"
                onclick={() => (postType = 'reel')}
                class="px-2.5 py-1 text-xs font-medium rounded-md transition-colors {postType === 'reel' ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
              >
                Reel
              </button>
            </div>
          </div>

          <!-- Message Textarea -->
          <div class="flex flex-col gap-1.5">
            <label for="post-msg-input" class="text-xs font-semibold text-foreground">Post Content</label>
            <textarea
              id="post-msg-input"
              bind:value={postMessage}
              oninput={(e) => {
                const target = (e.target || e.currentTarget) as HTMLTextAreaElement;
                if (target) postMessage = target.value;
              }}
              rows="4"
              placeholder="What's on your mind? Type your post message here..."
              class="w-full rounded-lg border border-input bg-background p-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            ></textarea>
          </div>

          <!-- Link URL Input (If link type) -->
          {#if postType === 'link'}
            <div class="flex flex-col gap-1.5">
              <label for="post-link-input" class="text-xs font-semibold text-foreground">Destination Link URL</label>
              <div class="relative">
                <LinkIcon class="w-4 h-4 absolute left-3 top-3 text-muted-foreground" />
                <input
                  id="post-link-input"
                  type="url"
                  bind:value={postLinkUrl}
                  placeholder="https://example.com/product"
                  class="w-full pl-9 pr-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                />
              </div>
            </div>
          {/if}

          <!-- Media Paths / URLs (If photo/multi/video/reel) -->
          {#if postType !== 'status' && postType !== 'link'}
            <div class="flex flex-col gap-2">
              <label for="post-media-input" class="text-xs font-semibold text-foreground">Media Files / URLs</label>
              <div class="flex gap-2">
                <input
                  id="post-media-input"
                  type="text"
                  bind:value={mediaInput}
                  placeholder="Paste local file path or image/video URL..."
                  class="flex-1 px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                  onkeydown={(e) => e.key === 'Enter' && addMediaPath()}
                />
                <button
                  type="button"
                  onclick={addMediaPath}
                  class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-medium rounded-lg hover:bg-secondary/80"
                >
                  Add Media
                </button>
              </div>

              {#if postMediaPaths.length > 0}
                <div class="flex flex-wrap gap-2 mt-1">
                  {#each postMediaPaths as path, index}
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
                  Schedule for future publishing
                </label>
              </div>
              {#if isScheduled}
                <span class="text-xs text-muted-foreground">
                  (10 min to 30 days published via Meta; >30d queued locally)
                </span>
              {/if}
            </div>

            {#if isScheduled}
              <div class="flex flex-col gap-1.5">
                <label for="schedule-dt-input" class="text-xs font-semibold text-foreground">Target Date & Time</label>
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
              Submitting...
            {:else if isScheduled}
              <Clock class="w-4 h-4" />
              Schedule Post
            {:else}
              <Send class="w-4 h-4" />
              Publish Now
            {/if}
          </button>
        </div>
      </div>

      <!-- Live Feed Preview (Right 5 Cols) -->
      <div class="lg:col-span-5 flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <h3 class="text-sm font-bold text-foreground">Live Feed Mockup</h3>
          <div class="flex items-center gap-1 bg-muted p-1 rounded-lg border border-border">
            <button
              type="button"
              onclick={() => (previewDevice = 'desktop')}
              class="p-1.5 rounded-md transition-colors {previewDevice === 'desktop' ? 'bg-background text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground'}"
              title="Desktop View"
            >
              <Monitor class="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onclick={() => (previewDevice = 'mobile')}
              class="p-1.5 rounded-md transition-colors {previewDevice === 'mobile' ? 'bg-background text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground'}"
              title="Mobile View"
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
                <span>Just now</span> • <span>Sponsored</span> • <span>🌐</span>
              </div>
            </div>
          </div>

          <!-- Post Content Text -->
          <div class="px-4 pb-3 text-sm text-foreground whitespace-pre-wrap">
            {postMessage || 'Your post message preview will appear here in real time...'}
          </div>

          <!-- Media / Link Preview Placeholder -->
          {#if postType === 'link'}
            <div class="border-t border-border bg-muted/40 p-4">
              <div class="text-xs font-semibold text-blue-600 uppercase tracking-wider truncate">
                {postLinkUrl ? new URL(postLinkUrl).hostname : 'EXAMPLE.COM'}
              </div>
              <div class="font-bold text-sm text-foreground mt-0.5">
                Special Offer & Details
              </div>
              <div class="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                Click to explore our latest collection and promotional offers.
              </div>
            </div>
          {:else if postType === 'photo' || postType === 'multi_photo'}
            <div class="bg-muted/60 aspect-video flex flex-col items-center justify-center text-muted-foreground border-y border-border">
              <Image class="w-10 h-10 stroke-[1.5]" />
              <span class="text-xs mt-1 font-medium">
                {postMediaPaths.length > 0 ? `${postMediaPaths.length} Photo(s) Attached` : 'Photo Preview Placeholder'}
              </span>
            </div>
          {:else if postType === 'video'}
            <div class="bg-slate-950 aspect-video flex flex-col items-center justify-center text-slate-400 border-y border-border">
              <Video class="w-12 h-12 stroke-[1.5] text-white/80" />
              <span class="text-xs mt-1 font-medium text-white/70">HD Video Player Preview</span>
            </div>
          {:else if postType === 'reel'}
            <div class="bg-slate-950 aspect-[9/16] max-h-72 mx-auto flex flex-col items-center justify-center text-slate-400 border-y border-border rounded-lg my-2">
              <Film class="w-10 h-10 stroke-[1.5] text-white/80" />
              <span class="text-xs mt-1 font-medium text-white/70">9:16 Vertical Reel Preview</span>
            </div>
          {/if}

          <!-- Post Actions Bar -->
          <div class="px-4 py-2.5 border-t border-border flex items-center justify-between text-muted-foreground text-xs font-medium">
            <button type="button" class="flex items-center gap-1.5 hover:text-blue-600 transition-colors">
              <ThumbsUp class="w-4 h-4" /> Like
            </button>
            <button type="button" class="flex items-center gap-1.5 hover:text-foreground transition-colors">
              <MessageCircle class="w-4 h-4" /> Comment
            </button>
            <button type="button" class="flex items-center gap-1.5 hover:text-foreground transition-colors">
              <Share2 class="w-4 h-4" /> Share
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Scheduled Posts Management Table -->
    <div class="bg-card border border-border rounded-xl p-6 shadow-2xs flex flex-col gap-4 mt-6">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-base font-bold text-foreground">Scheduled & Recent Posts</h3>
          <p class="text-xs text-muted-foreground">Manage pending queue, monitor Meta status, and perform instant overrides</p>
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
              <th class="py-2.5 px-3">ID</th>
              <th class="py-2.5 px-3">Type</th>
              <th class="py-2.5 px-3">Message Excerpt</th>
              <th class="py-2.5 px-3">Mode</th>
              <th class="py-2.5 px-3">Target Time</th>
              <th class="py-2.5 px-3">Status</th>
              <th class="py-2.5 px-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            {#if scheduledPosts.length === 0}
              <tr>
                <td colspan="7" class="py-8 text-center text-muted-foreground">
                  No scheduled or recent posts found.
                </td>
              </tr>
            {:else}
              {#each scheduledPosts as post}
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
                    {post.scheduled_publish_time ? new Date(post.scheduled_publish_time).toLocaleString() : 'Immediate'}
                  </td>
                  <td class="py-2.5 px-3">
                    {#if post.status === 'published'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/10 text-emerald-600 border border-emerald-500/20">
                        Published
                      </span>
                    {:else if post.status === 'scheduled_on_meta'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-blue-500/10 text-blue-600 border border-blue-500/20">
                        Meta Scheduled
                      </span>
                    {:else if post.status === 'queued'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-amber-500/10 text-amber-600 border border-amber-500/20">
                        Local Queued
                      </span>
                    {:else if post.status === 'failed'}
                      <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-destructive/10 text-destructive border border-destructive/20" title={post.error_message || ''}>
                        Failed
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
                          Publish Now
                        </button>
                        <button
                          type="button"
                          onclick={() => handleCancelPost(post.id)}
                          class="px-2 py-1 bg-destructive/10 text-destructive rounded text-[11px] font-medium hover:bg-destructive/20"
                        >
                          Cancel
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
            <h3 class="text-base font-bold text-foreground">Active Automation Rules</h3>
            <p class="text-xs text-muted-foreground">Automatically like, public reply, and private message customers based on comment keywords</p>
          </div>
          <button
            type="button"
            onclick={openCreateRuleModal}
            class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-blue-600 text-white font-semibold text-xs hover:bg-blue-700 transition-colors shadow-2xs"
          >
            <Plus class="w-4 h-4" />
            Add Rule
          </button>
        </div>

        <div class="overflow-x-auto">
          <table class="w-full text-left text-xs border-collapse">
            <thead>
              <tr class="border-b border-border text-muted-foreground font-semibold">
                <th class="py-2.5 px-3">Rule Name</th>
                <th class="py-2.5 px-3">Keywords</th>
                <th class="py-2.5 px-3">Match Mode</th>
                <th class="py-2.5 px-3">Auto-Like</th>
                <th class="py-2.5 px-3">Private Reply</th>
                <th class="py-2.5 px-3">Status</th>
                <th class="py-2.5 px-3 text-right">Actions</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              {#if automationRules.length === 0}
                <tr>
                  <td colspan="7" class="py-8 text-center text-muted-foreground">
                    No comment automation rules configured. Click "Add Rule" to get started!
                  </td>
                </tr>
              {:else}
                {#each automationRules as rule}
                  <tr class="hover:bg-muted/30 transition-colors">
                    <td class="py-2.5 px-3 font-semibold text-foreground">{rule.rule_name}</td>
                    <td class="py-2.5 px-3">
                      <div class="flex flex-wrap gap-1">
                        {#each rule.keywords as kw}
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
                      {rule.private_reply_template || 'None'}
                    </td>
                    <td class="py-2.5 px-3">
                      {#if rule.is_enabled}
                        <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/10 text-emerald-600">Enabled</span>
                      {:else}
                        <span class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-muted text-muted-foreground">Disabled</span>
                      {/if}
                    </td>
                    <td class="py-2.5 px-3 text-right">
                      <div class="inline-flex items-center gap-1">
                        <button
                          type="button"
                          onclick={() => openEditRuleModal(rule)}
                          class="px-2 py-1 rounded hover:bg-accent text-xs text-foreground font-medium"
                        >
                          Edit
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
            <h3 class="text-base font-bold text-foreground">Comment Activity Audit Logs</h3>
            <p class="text-xs text-muted-foreground">Real-time audit log of processed comments, auto-likes, and private send delivery statuses</p>
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
                <th class="py-2 px-3">Comment ID</th>
                <th class="py-2 px-3">Sender</th>
                <th class="py-2 px-3">Comment</th>
                <th class="py-2 px-3">Like</th>
                <th class="py-2 px-3">Public Reply</th>
                <th class="py-2 px-3">Private Send</th>
                <th class="py-2 px-3">Time</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border font-mono text-[11px]">
              {#if commentLogs.length === 0}
                <tr>
                  <td colspan="7" class="py-6 text-center text-muted-foreground font-sans text-xs">
                    No recent comment automation logs.
                  </td>
                </tr>
              {:else}
                {#each commentLogs as log}
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
        <h3 class="text-base font-bold text-foreground">Meta Data Deletion & Multi-Device Architecture</h3>
        <p class="text-xs text-muted-foreground leading-relaxed">
          RustSale Pro is architected to ensure total compliance with Meta User Data Deletion policies across local-first multi-device deployments.
        </p>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-2">
          <div class="p-4 rounded-lg bg-muted/40 border border-border flex flex-col gap-2">
            <div class="flex items-center gap-2 font-semibold text-xs text-foreground">
              <ShieldCheck class="w-4 h-4 text-emerald-500" />
              HMAC Signed-Request Gateway
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">
              Meta deletion callbacks are securely received by Supabase Edge Functions with HMAC-SHA256 signature verification and registered into <code class="text-[11px] bg-muted px-1 rounded">meta_data_deletion_requests</code>.
            </p>
          </div>

          <div class="p-4 rounded-lg bg-muted/40 border border-border flex flex-col gap-2">
            <div class="flex items-center gap-2 font-semibold text-xs text-foreground">
              <RefreshCw class="w-4 h-4 text-blue-500" />
              Multi-Device Worker Task Fan-Out
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">
              Enrolled desktop nodes claim device deletion tasks via lease tokens, transactionally scrub SQLite tables & disk spools, and complete requests atomically.
            </p>
          </div>
        </div>

        <div class="border-t border-border pt-4 mt-2 flex flex-col gap-3">
          <h4 class="text-xs font-bold text-foreground">Deletion Callback Status Endpoint</h4>
          <p class="text-xs text-muted-foreground">
            Meta users can verify the real-time completion status of their data deletion request via the public confirmation link:
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
          {editingRuleId ? 'Edit Automation Rule' : 'Create Comment Automation Rule'}
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
          <label for="rule-name-input" class="font-semibold text-foreground">Rule Name</label>
          <input
            id="rule-name-input"
            type="text"
            bind:value={ruleName}
            placeholder="e.g. Price Inquiry Auto-Responder"
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="rule-keywords-input" class="font-semibold text-foreground">Trigger Keywords (comma separated)</label>
          <input
            id="rule-keywords-input"
            type="text"
            bind:value={ruleKeywords}
            placeholder="e.g. giá, bao nhiêu, inbox, tư vấn"
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="flex flex-col gap-1.5">
            <label for="rule-match-mode-select" class="font-semibold text-foreground">Match Mode</label>
            <select
              id="rule-match-mode-select"
              bind:value={ruleMatchMode}
              class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
            >
              <option value="contains">Contains (Any)</option>
              <option value="all">Contains All</option>
              <option value="exact">Exact Match</option>
              <option value="regex">Regular Expression</option>
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
              Auto-Like Comment
            </label>
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <label for="rule-public-reply-input" class="font-semibold text-foreground">Public Reply Template (Optional)</label>
            <span class="text-[10px] text-blue-600 bg-blue-500/10 px-1.5 py-0.5 rounded font-mono">Use {"{{name}}"}</span>
          </div>
          <textarea
            id="rule-public-reply-input"
            bind:value={rulePublicReply}
            rows="2"
            placeholder="Chào {{name}}, shop đã gửi báo giá qua tin nhắn cho bạn nhé!"
            class="px-3 py-2 rounded-lg border border-input bg-background text-sm text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          ></textarea>
        </div>

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <label for="rule-private-reply-input" class="font-semibold text-foreground">1-Reply Private Message Template</label>
            <span class="text-[10px] text-blue-600 bg-blue-500/10 px-1.5 py-0.5 rounded font-mono">Use {"{{name}}"}</span>
          </div>
          <textarea
            id="rule-private-reply-input"
            bind:value={rulePrivateReply}
            rows="3"
            placeholder="Chào {{name}}, cảm ơn bạn đã quan tâm đến sản phẩm của shop! Giá sản phẩm là..."
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
          Cancel
        </button>
        <button
          type="button"
          onclick={handleSaveRule}
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 text-xs font-semibold"
        >
          Save Rule
        </button>
      </div>
    </div>
  </div>
{/if}
