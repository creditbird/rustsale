import { computed, Scroll, signal, Text, View } from "nsh";
import { Button as Base, Select } from "nsh/base";
import { Facebook } from "rustsale/native";
import { Button, Checkbox, EmptyState, ErrorState, Input, Modal, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { failure, startCore } from "../stores/core";
import { route } from "../stores/nav";
import { notify, report } from "../stores/toast";
import {
  LOG_FILTERS,
  MATCH_MODES,
  POST_TYPES,
  SUB_TABS,
  canCancelPost,
  canPublishNow,
  extractHostname,
  filterCommentLogs,
  formatDateTime,
  logStepTone,
  matchModeLabel,
  parseCsvList,
  parseScheduledTime,
  postStatusLabel,
  postStatusTone,
  postTypeLabel,
  tokenStatusLabel,
  tokenStatusTone,
  validatePostDraft,
  validateRuleDraft,
  type CommentAutomationRule,
  type CommentLog,
  type CommentMatchMode,
  type CreateCommentAutomationPayload,
  type CreateFacebookPostPayload,
  type FacebookAccount,
  type FacebookHubTab,
  type FacebookPostType,
  type FacebookScheduledPost,
  type LogStatusFilter,
  type PageHealth,
  type PreviewDevice,
  type UpdateCommentAutomationPayload,
} from "../stores/facebookHub";

const PILL_BG: Record<BadgeTone, string> = {
  neutral: "bg-muted",
  success: "bg-success/15",
  warning: "bg-warning/15",
  danger: "bg-destructive/15",
};

const PILL_FG: Record<BadgeTone, string> = {
  neutral: "text-muted-foreground",
  success: "text-success",
  warning: "text-warning",
  danger: "text-destructive",
};

/** Status pill whose tone and text stay reactive inside nsh components. */
function StatusPill(props: { label: string; tone?: BadgeTone; testID?: string }) {
  return (
    <View testID={props.testID} class={`self-start rounded-full px-2 py-0.5 ${PILL_BG[props.tone ?? "neutral"]}`}>
      <Text class={`text-xs font-medium ${PILL_FG[props.tone ?? "neutral"]}`}>{props.label}</Text>
    </View>
  );
}

function MetricCard(props: { testID: string; label: string; value: string }) {
  return (
    <View class="min-w-[160px] flex-1 gap-1 rounded-(--radius) border border-border bg-card p-4">
      <Text class="text-xs font-medium text-muted-foreground">{props.label}</Text>
      <Text testID={props.testID} class="text-xl font-semibold text-foreground">{props.value}</Text>
    </View>
  );
}

/** Facebook Hub (UI-36): composer + schedule queue, comment automations + audit logs, page health + compliance. */
export function FacebookHubScreen() {
  // ---- Sub-tab & Page accounts state
  const activeTab = signal<FacebookHubTab>("composer");
  const accounts = signal<FacebookAccount[]>([]);
  const selectedPageId = signal("");
  const ready = signal(false);
  const loadingAccounts = signal(false);
  const accountsError = signal<string | null>(null);

  const currentAccount = computed<FacebookAccount | null>(() => {
    const list = accounts();
    if (list.length === 0) return null;
    return list.find((a) => a.pageId === selectedPageId()) ?? list[0];
  });

  // ---- Per-page data state
  const pageHealth = signal<PageHealth | null>(null);
  const loadingHealth = signal(false);
  const healthError = signal<string | null>(null);

  const scheduledPosts = signal<FacebookScheduledPost[]>([]);
  const loadingPosts = signal(false);
  const postsError = signal<string | null>(null);

  const rules = signal<CommentAutomationRule[]>([]);
  const loadingRules = signal(false);
  const rulesError = signal<string | null>(null);

  const commentLogs = signal<CommentLog[]>([]);
  const loadingLogs = signal(false);
  const logsError = signal<string | null>(null);

  let pageLoadGen = 0;

  // ---- Composer state
  const postType = signal<FacebookPostType>("status");
  const postMessage = signal("");
  const postLinkUrl = signal("");
  const mediaInput = signal("");
  const mediaPaths = signal<string[]>([]);
  const isScheduled = signal(false);
  const scheduledDateTime = signal("");
  const previewDevice = signal<PreviewDevice>("desktop");
  const submittingPost = signal(false);
  const postError = signal<string | null>(null);
  const postSuccess = signal<string | null>(null);
  const actingPostId = signal<number | null>(null);
  const confirmingCancelPost = signal<FacebookScheduledPost | null>(null);

  // ---- Automations & Logs state
  const ruleModalOpen = signal(false);
  const editingRuleId = signal<number | null>(null);
  const ruleName = signal("");
  const ruleKeywords = signal("");
  const rulePostIds = signal("");
  const ruleMatchMode = signal<CommentMatchMode>("contains");
  const ruleAutoLike = signal(true);
  const rulePublicReply = signal("");
  const rulePrivateReply = signal("");
  const ruleIsEnabled = signal(true);
  const ruleError = signal<string | null>(null);
  const savingRule = signal(false);
  const togglingRuleId = signal<number | null>(null);
  const confirmingDeleteRule = signal<CommentAutomationRule | null>(null);
  const deletingRuleId = signal<number | null>(null);

  const logQuery = signal("");
  const logFilter = signal<LogStatusFilter>("all");
  const filteredLogs = computed(() => filterCommentLogs(commentLogs(), logQuery(), logFilter()));

  // ---- Derived header metrics
  const activeAutomationsCount = computed(
    () => pageHealth()?.active_automations_count ?? rules().filter((r) => r.is_enabled).length,
  );
  const pendingPostsCount = computed(
    () =>
      pageHealth()?.active_scheduled_count ??
      scheduledPosts().filter((p) => p.status === "queued" || p.status === "publishing" || p.status === "scheduled_on_meta")
        .length,
  );
  const effectiveTokenStatus = computed(() => pageHealth()?.token_status ?? currentAccount()?.tokenStatus);

  const pageSelectItems = computed(() =>
    accounts().map((a) => ({
      value: a.pageId,
      title: `${a.pageName?.trim() || a.pageId} (${a.pageId})`,
    })),
  );

  // ---- Data loaders
  const loadPageData = async (pageId: string) => {
    if (!pageId) {
      pageHealth.set(null);
      scheduledPosts.set([]);
      rules.set([]);
      commentLogs.set([]);
      return;
    }
    pageLoadGen += 1;
    const mine = pageLoadGen;
    loadingHealth.set(true);
    loadingPosts.set(true);
    loadingRules.set(true);
    loadingLogs.set(true);
    healthError.set(null);
    postsError.set(null);
    rulesError.set(null);
    logsError.set(null);

    try {
      await startCore();
      const [healthRes, postsRes, rulesRes, logsRes] = await Promise.all([
        Facebook.pageHealth(pageId)
          .then((data) => ({ ok: true as const, data: data as PageHealth }))
          .catch((e: unknown) => ({ ok: false as const, error: e })),
        Facebook.scheduledPosts(pageId)
          .then((data) => ({ ok: true as const, data: Array.isArray(data) ? (data as FacebookScheduledPost[]) : [] }))
          .catch((e: unknown) => ({ ok: false as const, error: e })),
        Facebook.commentAutomations(pageId)
          .then((data) => ({ ok: true as const, data: Array.isArray(data) ? (data as CommentAutomationRule[]) : [] }))
          .catch((e: unknown) => ({ ok: false as const, error: e })),
        Facebook.commentLogs(pageId, 50)
          .then((data) => ({ ok: true as const, data: Array.isArray(data) ? (data as CommentLog[]) : [] }))
          .catch((e: unknown) => ({ ok: false as const, error: e })),
      ]);

      if (mine !== pageLoadGen || selectedPageId() !== pageId) return;

      if (healthRes.ok) {
        pageHealth.set(healthRes.data);
      } else {
        pageHealth.set(null);
        healthError.set(failure(healthRes.error).message);
      }

      if (postsRes.ok) {
        scheduledPosts.set(postsRes.data);
      } else {
        scheduledPosts.set([]);
        postsError.set(failure(postsRes.error).message);
      }

      if (rulesRes.ok) {
        rules.set(rulesRes.data);
      } else {
        rules.set([]);
        rulesError.set(failure(rulesRes.error).message);
      }

      if (logsRes.ok) {
        commentLogs.set(logsRes.data);
      } else {
        commentLogs.set([]);
        logsError.set(failure(logsRes.error).message);
      }
    } catch (e) {
      if (mine !== pageLoadGen) return;
      const msg = failure(e).message;
      healthError.set(msg);
      postsError.set(msg);
      rulesError.set(msg);
      logsError.set(msg);
      report(e, { retry: () => { void loadPageData(pageId); } });
    } finally {
      if (mine === pageLoadGen) {
        loadingHealth.set(false);
        loadingPosts.set(false);
        loadingRules.set(false);
        loadingLogs.set(false);
      }
    }
  };

  const loadAccounts = async () => {
    if (loadingAccounts()) return;
    loadingAccounts.set(true);
    accountsError.set(null);
    try {
      await startCore();
      const list = ((await Facebook.accounts()) as FacebookAccount[]) ?? [];
      accounts.set(Array.isArray(list) ? list : []);
      const prev = selectedPageId();
      const nextPageId =
        list.some((a) => a.pageId === prev)
          ? prev
          : (list[0]?.pageId ?? "");
      selectedPageId.set(nextPageId);
      if (nextPageId !== "") {
        await loadPageData(nextPageId);
      } else {
        pageHealth.set(null);
        scheduledPosts.set([]);
        rules.set([]);
        commentLogs.set([]);
      }
    } catch (e) {
      const f = report(e, { retry: () => { void loadAccounts(); } });
      accountsError.set(f.message);
    } finally {
      loadingAccounts.set(false);
      ready.set(true);
    }
  };

  const selectPage = (pageId: string) => {
    if (!pageId || pageId === selectedPageId()) return;
    selectedPageId.set(pageId);
    postError.set(null);
    postSuccess.set(null);
    void loadPageData(pageId);
  };

  const refreshHealth = async () => {
    const pageId = selectedPageId();
    if (!pageId || loadingHealth()) return;
    loadingHealth.set(true);
    healthError.set(null);
    try {
      await startCore();
      const data = (await Facebook.pageHealth(pageId)) as PageHealth;
      if (selectedPageId() === pageId) {
        pageHealth.set(data);
      }
    } catch (e) {
      const f = report(e, { retry: () => { void refreshHealth(); } });
      if (selectedPageId() === pageId) {
        healthError.set(f.message);
      }
    } finally {
      loadingHealth.set(false);
    }
  };

  const refreshLogs = async () => {
    const pageId = selectedPageId();
    if (!pageId || loadingLogs()) return;
    loadingLogs.set(true);
    logsError.set(null);
    try {
      await startCore();
      const data = await Facebook.commentLogs(pageId, 50);
      if (selectedPageId() === pageId) {
        commentLogs.set(Array.isArray(data) ? (data as CommentLog[]) : []);
      }
    } catch (e) {
      const f = report(e, { retry: () => { void refreshLogs(); } });
      if (selectedPageId() === pageId) {
        logsError.set(f.message);
      }
    } finally {
      loadingLogs.set(false);
    }
  };

  // ---- Composer actions
  const addMediaPath = () => {
    const next = mediaInput().trim();
    if (next === "") return;
    if (!mediaPaths().includes(next)) {
      mediaPaths.set([...mediaPaths(), next]);
    }
    mediaInput.set("");
  };

  const removeMediaPath = (index: number) => {
    mediaPaths.set(mediaPaths().filter((_, i) => i !== index));
  };

  const submitPost = async () => {
    if (submittingPost()) return;
    postError.set(null);
    postSuccess.set(null);

    const acc = currentAccount();
    const problem = validatePostDraft({
      account: acc,
      postType: postType(),
      message: postMessage(),
      linkUrl: postLinkUrl(),
      isScheduled: isScheduled(),
      scheduledDateTime: scheduledDateTime(),
    });
    if (problem !== null || acc === null) {
      postError.set(problem ?? "Vui lòng kết nối và chọn Fanpage Facebook hợp lệ trước khi đăng bài.");
      return;
    }

    const scheduledIso = isScheduled() ? parseScheduledTime(scheduledDateTime()) : null;
    const payload: CreateFacebookPostPayload = {
      channel_account_id: acc.channelAccountId,
      page_id: acc.pageId,
      post_type: postType(),
      message: postMessage().trim(),
      link_url: postType() === "link" ? postLinkUrl().trim() : null,
      media_paths: postType() === "status" || postType() === "link" ? [] : mediaPaths(),
      scheduled_publish_time: scheduledIso,
    };

    submittingPost.set(true);
    try {
      await startCore();
      const postId = (await Facebook.schedulePost(payload)) as number;
      if (!isScheduled()) {
        await Facebook.publishPostNow(postId);
        postSuccess.set("Bài viết đã được đưa vào hàng đợi đăng ngay.");
        notify("Đã đưa bài viết vào hàng đợi đăng ngay", "success");
      } else {
        postSuccess.set("Đã lên lịch bài viết thành công.");
        notify("Đã lên lịch bài viết", "success");
      }
      postMessage.set("");
      postLinkUrl.set("");
      mediaPaths.set([]);
      mediaInput.set("");
      scheduledDateTime.set("");
      await loadPageData(acc.pageId);
    } catch (e) {
      const f = report(e, { retry: () => { void submitPost(); } });
      postError.set(f.message);
    } finally {
      submittingPost.set(false);
    }
  };

  const publishNow = async (postId: number) => {
    if (actingPostId() !== null) return;
    actingPostId.set(postId);
    try {
      await startCore();
      await Facebook.publishPostNow(postId);
      notify("Đã chuyển bài viết sang đăng ngay", "success");
      await loadPageData(selectedPageId());
    } catch (e) {
      report(e, { retry: () => { void publishNow(postId); } });
    } finally {
      actingPostId.set(null);
    }
  };

  const confirmCancelPost = async () => {
    const target = confirmingCancelPost();
    if (!target || actingPostId() !== null) return;
    actingPostId.set(target.id);
    try {
      await startCore();
      await Facebook.cancelScheduledPost(target.id);
      confirmingCancelPost.set(null);
      notify("Đã huỷ bài viết đã lên lịch", "success");
      await loadPageData(selectedPageId());
    } catch (e) {
      confirmingCancelPost.set(null);
      report(e, { retry: () => { confirmingCancelPost.set(target); } });
    } finally {
      actingPostId.set(null);
    }
  };

  // ---- Automation rule actions
  const openCreateRule = () => {
    editingRuleId.set(null);
    ruleName.set("");
    ruleKeywords.set("");
    rulePostIds.set("");
    ruleMatchMode.set("contains");
    ruleAutoLike.set(true);
    rulePublicReply.set("Chào {{name}}, cảm ơn bạn đã quan tâm!");
    rulePrivateReply.set("Chào {{name}}, shop đã gửi thông tin chi tiết qua tin nhắn cho bạn ạ!");
    ruleIsEnabled.set(true);
    ruleError.set(null);
    ruleModalOpen.set(true);
  };

  const openEditRule = (rule: CommentAutomationRule) => {
    editingRuleId.set(rule.id);
    ruleName.set(rule.rule_name);
    ruleKeywords.set((rule.keywords ?? []).join(", "));
    rulePostIds.set((rule.post_ids ?? []).join(", "));
    const mode = MATCH_MODES.some((m) => m.id === rule.match_mode)
      ? (rule.match_mode as CommentMatchMode)
      : "contains";
    ruleMatchMode.set(mode);
    ruleAutoLike.set(rule.auto_like);
    rulePublicReply.set(rule.public_reply_template ?? "");
    rulePrivateReply.set(rule.private_reply_template ?? "");
    ruleIsEnabled.set(rule.is_enabled);
    ruleError.set(null);
    ruleModalOpen.set(true);
  };

  const saveRule = async () => {
    if (savingRule()) return;
    ruleError.set(null);
    const acc = currentAccount();
    const problem = validateRuleDraft({
      account: acc,
      ruleName: ruleName(),
      matchMode: ruleMatchMode(),
      keywords: ruleKeywords(),
    });
    if (problem !== null || acc === null) {
      ruleError.set(problem ?? "Vui lòng kiểm tra lại thông tin quy tắc.");
      return;
    }

    const keywords = parseCsvList(ruleKeywords());
    const postIds = parseCsvList(rulePostIds());
    const publicTpl = rulePublicReply().trim() || null;
    const privateTpl = rulePrivateReply().trim() || null;

    savingRule.set(true);
    try {
      await startCore();
      const editId = editingRuleId();
      if (editId !== null) {
        const req: UpdateCommentAutomationPayload = {
          id: editId,
          rule_name: ruleName().trim(),
          post_ids: postIds,
          keywords,
          match_mode: ruleMatchMode(),
          auto_like: ruleAutoLike(),
          public_reply_template: publicTpl,
          private_reply_template: privateTpl,
          is_enabled: ruleIsEnabled(),
        };
        await Facebook.updateCommentAutomation(req);
        notify("Đã cập nhật quy tắc bình luận", "success");
      } else {
        const req: CreateCommentAutomationPayload = {
          channel_account_id: acc.channelAccountId,
          page_id: acc.pageId,
          rule_name: ruleName().trim(),
          post_ids: postIds,
          keywords,
          match_mode: ruleMatchMode(),
          auto_like: ruleAutoLike(),
          public_reply_template: publicTpl,
          private_reply_template: privateTpl,
          is_enabled: ruleIsEnabled(),
        };
        await Facebook.createCommentAutomation(req);
        notify("Đã tạo quy tắc bình luận", "success");
      }
      ruleModalOpen.set(false);
      await loadPageData(acc.pageId);
    } catch (e) {
      const f = report(e, { retry: () => { void saveRule(); } });
      ruleError.set(f.message);
    } finally {
      savingRule.set(false);
    }
  };

  const toggleRule = async (rule: CommentAutomationRule) => {
    if (togglingRuleId() !== null) return;
    togglingRuleId.set(rule.id);
    try {
      await startCore();
      const mode = MATCH_MODES.some((m) => m.id === rule.match_mode)
        ? (rule.match_mode as CommentMatchMode)
        : "contains";
      const req: UpdateCommentAutomationPayload = {
        id: rule.id,
        rule_name: rule.rule_name,
        post_ids: rule.post_ids ?? [],
        keywords: rule.keywords ?? [],
        match_mode: mode,
        auto_like: rule.auto_like,
        public_reply_template: rule.public_reply_template,
        private_reply_template: rule.private_reply_template,
        is_enabled: !rule.is_enabled,
      };
      await Facebook.updateCommentAutomation(req);
      notify(!rule.is_enabled ? "Đã bật quy tắc" : "Đã tắt quy tắc", "success");
      await loadPageData(selectedPageId());
    } catch (e) {
      report(e, { retry: () => { void toggleRule(rule); } });
    } finally {
      togglingRuleId.set(null);
    }
  };

  const confirmDeleteRule = async () => {
    const target = confirmingDeleteRule();
    if (!target || deletingRuleId() !== null) return;
    deletingRuleId.set(target.id);
    try {
      await startCore();
      await Facebook.deleteCommentAutomation(target.id);
      confirmingDeleteRule.set(null);
      notify("Đã xoá quy tắc bình luận", "success");
      await loadPageData(selectedPageId());
    } catch (e) {
      confirmingDeleteRule.set(null);
      report(e, { retry: () => { confirmingDeleteRule.set(target); } });
    } finally {
      deletingRuleId.set(null);
    }
  };

  void loadAccounts();

  return (
    <View testID="fb-hub-screen" class="flex-1 bg-background">
      {/* Initial loading */}
      <If when={!ready() && accounts().length === 0}>
        <View class="gap-4 p-6">
          <Skeleton class="h-16 w-full" />
          <View class="flex-row gap-4">
            <Skeleton class="h-56 flex-1" />
            <Skeleton class="h-56 w-80" />
          </View>
        </View>
      </If>

      {/* Initial accounts load error */}
      <If when={ready() && accountsError() !== null && accounts().length === 0}>
        <View testID="fb-accounts-error" class="flex-1">
          <ErrorState
            message={accountsError() ?? "Không tải được danh sách Fanpage Facebook."}
            onRetry={() => { void loadAccounts(); }}
          />
        </View>
      </If>

      {/* Empty state: no connected Facebook Pages */}
      <If when={ready() && accountsError() === null && accounts().length === 0}>
        <View testID="fb-empty-state" class="flex-1 items-center justify-center gap-4 p-8">
          <View class="h-14 w-14 items-center justify-center rounded-2xl bg-primary/15">
            <Text class="text-2xl font-bold text-primary">f</Text>
          </View>
          <View class="max-w-[440px] items-center gap-1.5">
            <Text class="text-lg font-semibold text-foreground">Chưa kết nối Facebook Fanpage</Text>
            <Text class="text-center text-sm text-muted-foreground">
              Kết nối Fanpage trong Cài đặt → Facebook để đăng bài, lên lịch, cấu hình tự động trả lời bình luận và theo dõi sức khoẻ Page.
            </Text>
          </View>
          <View class="flex-row gap-2">
            <Button
              testID="fb-goto-settings"
              variant="primary"
              label="Mở Cài đặt → Facebook"
              onPress={() => route.set("settings")}
            />
            <Button
              testID="fb-refresh-empty"
              label="Làm mới"
              disabled={loadingAccounts()}
              onPress={() => { void loadAccounts(); }}
            />
          </View>
        </View>
      </If>

      {/* Connected state */}
      <If when={accounts().length > 0}>
        <View class="flex-1">
          {/* Page Header & Selector */}
          <View testID="fb-page-header" class="gap-3 border-b border-border bg-card px-6 py-4">
            <View class="flex-row flex-wrap items-center justify-between gap-3">
              <View class="min-w-0 flex-1 gap-1">
                <View class="flex-row flex-wrap items-center gap-2">
                  <Text testID="fb-active-page-name" class="text-lg font-semibold text-foreground">
                    {currentAccount()?.pageName?.trim() || currentAccount()?.pageId || "Facebook Hub"}
                  </Text>
                  <StatusPill
                    testID="fb-header-token-badge"
                    label={tokenStatusLabel(effectiveTokenStatus())}
                    tone={tokenStatusTone(effectiveTokenStatus())}
                  />
                </View>
                <Text testID="fb-header-summary" class="text-xs text-muted-foreground">
                  {`ID: ${selectedPageId()} · Quy tắc đang bật: ${activeAutomationsCount()} · Bài chờ đăng: ${pendingPostsCount()}`}
                </Text>
              </View>

              <View class="flex-row flex-wrap items-center gap-2">
                <Select
                  testID="fb-page-select"
                  items={pageSelectItems()}
                  value={selectedPageId()}
                  placeholder="Chọn Fanpage"
                  onChange={({ value }) => { if (value) selectPage(value); }}
                  trigger={
                    <View
                      testID="fb-page-select-trigger"
                      class="h-9 w-56 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3"
                    >
                      <Text class="truncate text-sm text-foreground">
                        {currentAccount()?.pageName?.trim() || selectedPageId() || "Chọn Fanpage"}
                      </Text>
                      <Text class="text-xs text-muted-foreground">▾</Text>
                    </View>
                  }
                  itemClass="px-3 py-2 selected:bg-accent"
                  renderItem={(entry) => (
                    <View testID={`fb-page-select-item-${entry.value}`}>
                      <Text class="text-sm text-foreground">{entry.title}</Text>
                    </View>
                  )}
                />
                <Button
                  testID="fb-refresh"
                  size="sm"
                  label={loadingAccounts() ? "Đang tải…" : "Làm mới"}
                  disabled={loadingAccounts()}
                  onPress={() => { void loadAccounts(); }}
                />
                <Button
                  testID="fb-open-settings"
                  size="sm"
                  label="Cài đặt"
                  onPress={() => route.set("settings")}
                />
              </View>
            </View>

            {/* Sub-tabs */}
            <View testID="fb-subtabs" class="flex-row flex-wrap gap-2">
              <Each of={SUB_TABS} key={(t) => t.id}>
                {(t) => (
                  <Base
                    testID={`fb-tab-${t().id}`}
                    accessibilityLabel={t().label}
                    onPress={() => activeTab.set(t().id)}
                    class={`rounded-full border px-3 py-1.5 ${
                      activeTab() === t().id ? "border-primary bg-primary" : "border-border bg-background"
                    }`}
                  >
                    <Text class={`text-sm font-medium ${activeTab() === t().id ? "text-primary-foreground" : "text-foreground"}`}>
                      {t().label}
                    </Text>
                  </Base>
                )}
              </Each>
            </View>
          </View>

          <Scroll class="flex-1">
            <View class="gap-5 p-6">
              {/* ============ SUB-TAB 1: COMPOSER & SCHEDULED POSTS ============ */}
              <If when={activeTab() === "composer"}>
                <View class="gap-5">
                  <View class="flex-row flex-wrap gap-5">
                    {/* Left: Composer Form */}
                    <View
                      testID="fb-composer"
                      class="min-w-[380px] flex-1 gap-4 rounded-(--radius) border border-border bg-card p-5"
                    >
                      <View class="gap-2">
                        <Text class="text-base font-semibold text-foreground">Tạo bài đăng Facebook</Text>
                        <View class="flex-row flex-wrap gap-1.5">
                          <Each of={POST_TYPES} key={(pt) => pt.id}>
                            {(pt) => (
                              <Button
                                testID={`fb-post-type-${pt().id}`}
                                size="sm"
                                label={pt().label}
                                variant={postType() === pt().id ? "primary" : "secondary"}
                                onPress={() => postType.set(pt().id)}
                              />
                            )}
                          </Each>
                        </View>
                      </View>

                      <View class="gap-1.5">
                        <Text class="text-xs font-medium text-foreground">Nội dung bài đăng</Text>
                        <Input
                          testID="fb-post-message"
                          multiline
                          value={postMessage()}
                          onValue={(v) => postMessage.set(v)}
                          placeholder="Bạn đang nghĩ gì? Nhập nội dung bài đăng tại đây…"
                          class="h-28 rounded-(--radius) border border-input bg-background p-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
                        />
                      </View>

                      <If when={postType() === "link"}>
                        <View class="gap-1.5">
                          <Text class="text-xs font-medium text-foreground">URL liên kết đích</Text>
                          <Input
                            testID="fb-post-link"
                            value={postLinkUrl()}
                            onValue={(v) => postLinkUrl.set(v)}
                            placeholder="https://example.com/san-pham"
                          />
                        </View>
                      </If>

                      <If when={postType() !== "status" && postType() !== "link"}>
                        <View class="gap-2">
                          <Text class="text-xs font-medium text-foreground">Đường dẫn tệp phương tiện / URL</Text>
                          <View class="flex-row gap-2">
                            <View class="flex-1">
                              <Input
                                testID="fb-media-input"
                                value={mediaInput()}
                                onValue={(v) => mediaInput.set(v)}
                                onSubmit={addMediaPath}
                                placeholder="Dán đường dẫn cục bộ hoặc URL ảnh/video…"
                              />
                            </View>
                            <Button testID="fb-media-add" size="sm" label="Thêm tệp" onPress={addMediaPath} />
                          </View>
                          <If when={mediaPaths().length > 0}>
                            <View class="gap-1.5">
                              <Each of={mediaPaths()} key={(p) => p}>
                                {(path, idx) => (
                                  <View
                                    testID={`fb-media-item-${idx()}`}
                                    class="flex-row items-center justify-between gap-2 rounded-(--radius) border border-border bg-muted/40 px-3 py-1.5"
                                  >
                                    <Text class="flex-1 truncate text-xs text-foreground">{path()}</Text>
                                    <Button
                                      testID={`fb-media-remove-${idx()}`}
                                      size="sm"
                                      variant="ghost"
                                      label="Xoá"
                                      onPress={() => removeMediaPath(idx())}
                                    />
                                  </View>
                                )}
                              </Each>
                            </View>
                          </If>
                        </View>
                      </If>

                      <View class="gap-2 border-t border-border pt-3">
                        <Checkbox
                          testID="fb-schedule-toggle"
                          checked={isScheduled()}
                          onChange={(checked) => isScheduled.set(checked)}
                          label="Lên lịch đăng bài trong tương lai"
                        />
                        <If when={isScheduled()}>
                          <View class="gap-1.5">
                            <Text class="text-xs text-muted-foreground">
                              10 phút đến 30 ngày qua Meta; trên 30 ngày xếp hàng cục bộ.
                            </Text>
                            <Input
                              testID="fb-schedule-datetime"
                              value={scheduledDateTime()}
                              onValue={(v) => scheduledDateTime.set(v)}
                              placeholder="YYYY-MM-DD HH:MM (ví dụ: 2026-12-01 10:30)"
                            />
                          </View>
                        </If>
                      </View>

                      <If when={postError() !== null}>
                        <View testID="fb-post-error" class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
                          <Text class="text-xs text-destructive">{postError() ?? ""}</Text>
                        </View>
                      </If>

                      <If when={postSuccess() !== null}>
                        <View testID="fb-post-success" class="rounded-(--radius) border border-success/30 bg-success/10 p-3">
                          <Text class="text-xs text-success">{postSuccess() ?? ""}</Text>
                        </View>
                      </If>

                      <View class="flex-row justify-end">
                        <Button
                          testID="fb-post-submit"
                          variant="primary"
                          disabled={submittingPost()}
                          label={submittingPost() ? "Đang gửi…" : isScheduled() ? "Lên lịch Đăng" : "Đăng ngay"}
                          onPress={() => { void submitPost(); }}
                        />
                      </View>
                    </View>

                    {/* Right: Live Feed Preview */}
                    <View testID="fb-preview" class="w-[380px] gap-3">
                      <View class="flex-row items-center justify-between">
                        <Text class="text-sm font-semibold text-foreground">Xem trước Bảng tin</Text>
                        <View class="flex-row gap-1">
                          <Button
                            testID="fb-preview-desktop"
                            size="sm"
                            label="Máy tính"
                            variant={previewDevice() === "desktop" ? "primary" : "secondary"}
                            onPress={() => previewDevice.set("desktop")}
                          />
                          <Button
                            testID="fb-preview-mobile"
                            size="sm"
                            label="Di động"
                            variant={previewDevice() === "mobile" ? "primary" : "secondary"}
                            onPress={() => previewDevice.set("mobile")}
                          />
                        </View>
                      </View>

                      <View
                        testID="fb-preview-card"
                        class={`gap-3 rounded-(--radius) border border-border bg-card p-4 ${
                          previewDevice() === "mobile" ? "w-[300px] self-center" : "w-full"
                        }`}
                      >
                        <View class="flex-row items-center justify-between">
                          <View class="flex-row items-center gap-2.5">
                            <View class="h-9 w-9 items-center justify-center rounded-full bg-primary">
                              <Text class="text-sm font-semibold text-primary-foreground">
                                {(currentAccount()?.pageName?.trim() || "F").charAt(0).toUpperCase()}
                              </Text>
                            </View>
                            <View class="gap-0.5">
                              <Text class="text-sm font-semibold text-foreground">
                                {currentAccount()?.pageName?.trim() || "Facebook Fanpage"}
                              </Text>
                              <Text class="text-xs text-muted-foreground">
                                {isScheduled() && scheduledDateTime().trim() !== ""
                                  ? `Hẹn đăng · ${scheduledDateTime().trim()}`
                                  : "Vừa xong · Công khai"}
                              </Text>
                            </View>
                          </View>
                          <StatusPill
                            testID="fb-preview-device-badge"
                            label={previewDevice() === "mobile" ? "Di động" : "Máy tính"}
                            tone="neutral"
                          />
                        </View>

                        <Text testID="fb-preview-message" class="text-sm text-foreground">
                          {postMessage().trim() !== ""
                            ? postMessage()
                            : "Nội dung bài viết xem trước sẽ hiển thị tại đây theo thời gian thực…"}
                        </Text>

                        <If when={postType() === "link"}>
                          <View class="gap-1 rounded-(--radius) border border-border bg-muted/40 p-3">
                            <Text class="text-xs font-semibold text-primary">{extractHostname(postLinkUrl())}</Text>
                            <Text class="text-sm font-medium text-foreground">Liên kết đính kèm</Text>
                            <Text class="truncate text-xs text-muted-foreground">
                              {postLinkUrl().trim() || "https://example.com"}
                            </Text>
                          </View>
                        </If>

                        <If when={postType() === "photo" || postType() === "multi_photo"}>
                          <View class="h-32 items-center justify-center rounded-(--radius) border border-border bg-muted/40 p-3">
                            <Text class="text-xs font-medium text-muted-foreground">
                              {mediaPaths().length > 0
                                ? `Đã đính kèm ${mediaPaths().length} ảnh`
                                : "Khung xem trước hình ảnh"}
                            </Text>
                          </View>
                        </If>

                        <If when={postType() === "video" || postType() === "reel"}>
                          <View class="h-32 items-center justify-center rounded-(--radius) border border-border bg-muted/60 p-3">
                            <Text class="text-xs font-medium text-foreground">
                              {postType() === "reel" ? "Khung xem trước Reel dọc 9:16" : "Khung xem trước Video"}
                            </Text>
                            <If when={mediaPaths().length > 0}>
                              <Text class="text-xs text-muted-foreground">{`${mediaPaths().length} tệp phương tiện`}</Text>
                            </If>
                          </View>
                        </If>

                        <View class="flex-row justify-between border-t border-border pt-2">
                          <Text class="text-xs text-muted-foreground">Thích</Text>
                          <Text class="text-xs text-muted-foreground">Bình luận</Text>
                          <Text class="text-xs text-muted-foreground">Chia sẻ</Text>
                        </View>
                      </View>
                    </View>
                  </View>

                  {/* Scheduled Posts List */}
                  <View testID="fb-scheduled-posts" class="gap-3 rounded-(--radius) border border-border bg-card p-5">
                    <View class="flex-row items-center justify-between">
                      <View class="gap-0.5">
                        <Text class="text-base font-semibold text-foreground">Bài viết đã lên lịch & gần đây</Text>
                        <Text class="text-xs text-muted-foreground">
                          Theo dõi trạng thái đăng bài, đẩy đăng ngay hoặc huỷ bài đang chờ.
                        </Text>
                      </View>
                      <Button
                        testID="fb-posts-refresh"
                        size="sm"
                        label={loadingPosts() ? "Đang tải…" : "Làm mới"}
                        disabled={loadingPosts()}
                        onPress={() => { void loadPageData(selectedPageId()); }}
                      />
                    </View>

                    <If when={postsError() !== null}>
                      <View testID="fb-posts-error" class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
                        <Text class="text-xs text-destructive">{postsError() ?? ""}</Text>
                      </View>
                    </If>

                    <If when={loadingPosts() && scheduledPosts().length === 0}>
                      <View class="gap-2">
                        <Skeleton class="h-12 w-full" />
                        <Skeleton class="h-12 w-full" />
                      </View>
                    </If>

                    <If when={!loadingPosts() && postsError() === null && scheduledPosts().length === 0}>
                      <EmptyState
                        title="Chưa có bài viết nào"
                        hint="Soạn bài ở khung phía trên để đăng ngay hoặc hẹn giờ."
                      />
                    </If>

                    <If when={scheduledPosts().length > 0}>
                      <View class="gap-2">
                        <Each of={scheduledPosts()} key={(p) => String(p.id)}>
                          {(p) => (
                            <View
                              testID={`fb-post-row-${p().id}`}
                              class="gap-2 rounded-(--radius) border border-border bg-background p-3"
                            >
                              <View class="flex-row flex-wrap items-center justify-between gap-2">
                                <View class="flex-row flex-wrap items-center gap-2">
                                  <Text class="text-xs font-medium text-muted-foreground">{`#${p().id}`}</Text>
                                  <StatusPill label={postTypeLabel(p().post_type)} tone="neutral" />
                                  <StatusPill
                                    testID={`fb-post-status-${p().id}`}
                                    label={postStatusLabel(p().status)}
                                    tone={postStatusTone(p().status)}
                                  />
                                  <Text class="text-xs text-muted-foreground">
                                    {`${p().scheduling_mode} · ${formatDateTime(p().scheduled_publish_time)}`}
                                  </Text>
                                </View>

                                <View class="flex-row gap-2">
                                  <If when={canPublishNow(p().status)}>
                                    <Button
                                      testID={`fb-post-publish-${p().id}`}
                                      size="sm"
                                      variant="primary"
                                      label="Đăng ngay"
                                      disabled={actingPostId() !== null}
                                      onPress={() => { void publishNow(p().id); }}
                                    />
                                  </If>
                                  <If when={canCancelPost(p().status)}>
                                    <Button
                                      testID={`fb-post-cancel-${p().id}`}
                                      size="sm"
                                      variant="destructive"
                                      label="Huỷ"
                                      disabled={actingPostId() !== null}
                                      onPress={() => confirmingCancelPost.set(p())}
                                    />
                                  </If>
                                </View>
                              </View>

                              <Text class="text-sm text-foreground">{p().message}</Text>

                              <If when={p().error_message !== null && p().error_message !== ""}>
                                <Text class="text-xs text-destructive">{p().error_message ?? ""}</Text>
                              </If>
                            </View>
                          )}
                        </Each>
                      </View>
                    </If>
                  </View>
                </View>
              </If>

              {/* ============ SUB-TAB 2: COMMENT AUTOMATIONS & LOGS ============ */}
              <If when={activeTab() === "automations"}>
                <View class="gap-5">
                  {/* Automation Rules Card */}
                  <View testID="fb-rules-card" class="gap-3 rounded-(--radius) border border-border bg-card p-5">
                    <View class="flex-row flex-wrap items-center justify-between gap-2">
                      <View class="gap-0.5">
                        <Text class="text-base font-semibold text-foreground">Quy tắc Tự động hóa Bình luận</Text>
                        <Text class="text-xs text-muted-foreground">
                          Tự động thích, trả lời công khai và nhắn tin riêng theo từ khoá bình luận.
                        </Text>
                      </View>
                      <View class="flex-row gap-2">
                        <Button
                          testID="fb-rules-refresh"
                          size="sm"
                          label={loadingRules() ? "Đang tải…" : "Làm mới"}
                          disabled={loadingRules()}
                          onPress={() => { void loadPageData(selectedPageId()); }}
                        />
                        <Button
                          testID="fb-rule-add"
                          size="sm"
                          variant="primary"
                          label="Thêm Quy tắc"
                          onPress={openCreateRule}
                        />
                      </View>
                    </View>

                    <If when={rulesError() !== null}>
                      <View testID="fb-rules-error" class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
                        <Text class="text-xs text-destructive">{rulesError() ?? ""}</Text>
                      </View>
                    </If>

                    <If when={loadingRules() && rules().length === 0}>
                      <View class="gap-2">
                        <Skeleton class="h-14 w-full" />
                        <Skeleton class="h-14 w-full" />
                      </View>
                    </If>

                    <If when={!loadingRules() && rulesError() === null && rules().length === 0}>
                      <EmptyState
                        title="Chưa có quy tắc tự động hóa bình luận"
                        hint="Bấm “Thêm Quy tắc” để cấu hình tự động trả lời theo từ khoá."
                      />
                    </If>

                    <If when={rules().length > 0}>
                      <View class="gap-2">
                        <Each of={rules()} key={(r) => String(r.id)}>
                          {(r) => (
                            <View
                              testID={`fb-rule-row-${r().id}`}
                              class="gap-2 rounded-(--radius) border border-border bg-background p-3"
                            >
                              <View class="flex-row flex-wrap items-center justify-between gap-2">
                                <View class="flex-row flex-wrap items-center gap-2">
                                  <Text class="text-sm font-semibold text-foreground">{r().rule_name}</Text>
                                  <StatusPill label={matchModeLabel(r().match_mode)} tone="neutral" />
                                  <StatusPill
                                    testID={`fb-rule-status-${r().id}`}
                                    label={r().is_enabled ? "Đang bật" : "Đã tắt"}
                                    tone={r().is_enabled ? "success" : "neutral"}
                                  />
                                  <If when={r().auto_like}>
                                    <StatusPill label="Tự động thích" tone="neutral" />
                                  </If>
                                </View>

                                <View class="flex-row gap-2">
                                  <Button
                                    testID={`fb-rule-toggle-${r().id}`}
                                    size="sm"
                                    label={r().is_enabled ? "Tắt" : "Bật"}
                                    disabled={togglingRuleId() !== null}
                                    onPress={() => { void toggleRule(r()); }}
                                  />
                                  <Button
                                    testID={`fb-rule-edit-${r().id}`}
                                    size="sm"
                                    label="Sửa"
                                    onPress={() => openEditRule(r())}
                                  />
                                  <Button
                                    testID={`fb-rule-delete-${r().id}`}
                                    size="sm"
                                    variant="destructive"
                                    label="Xoá"
                                    disabled={deletingRuleId() !== null}
                                    onPress={() => confirmingDeleteRule.set(r())}
                                  />
                                </View>
                              </View>

                              <Text class="text-xs text-muted-foreground">
                                {`Từ khoá: ${(r().keywords ?? []).length > 0 ? r().keywords.join(", ") : "(Mọi bình luận)"}`}
                              </Text>

                              <If when={(r().post_ids ?? []).length > 0}>
                                <Text class="text-xs text-muted-foreground">
                                  {`Post IDs: ${r().post_ids.join(", ")}`}
                                </Text>
                              </If>

                              <If when={r().public_reply_template !== null && r().public_reply_template !== ""}>
                                <Text class="text-xs text-foreground">
                                  {`Trả lời công khai: ${r().public_reply_template ?? ""}`}
                                </Text>
                              </If>

                              <If when={r().private_reply_template !== null && r().private_reply_template !== ""}>
                                <Text class="text-xs text-foreground">
                                  {`Tin nhắn riêng: ${r().private_reply_template ?? ""}`}
                                </Text>
                              </If>
                            </View>
                          )}
                        </Each>
                      </View>
                    </If>
                  </View>

                  {/* Comment Audit Logs Card */}
                  <View testID="fb-logs-card" class="gap-3 rounded-(--radius) border border-border bg-card p-5">
                    <View class="flex-row flex-wrap items-center justify-between gap-2">
                      <View class="gap-0.5">
                        <Text class="text-base font-semibold text-foreground">Nhật ký Hoạt động Bình luận</Text>
                        <Text class="text-xs text-muted-foreground">
                          Theo dõi các bình luận đã xử lý, trạng thái thích, trả lời công khai và nhắn tin riêng.
                        </Text>
                      </View>
                      <Button
                        testID="fb-logs-refresh"
                        size="sm"
                        label={loadingLogs() ? "Đang tải…" : "Làm mới"}
                        disabled={loadingLogs()}
                        onPress={() => { void refreshLogs(); }}
                      />
                    </View>

                    <View class="flex-row flex-wrap items-center gap-2">
                      <View class="min-w-[220px] flex-1">
                        <Input
                          testID="fb-logs-search"
                          value={logQuery()}
                          onValue={(v) => logQuery.set(v)}
                          placeholder="Tìm theo người gửi, nội dung, mã bình luận…"
                        />
                      </View>
                      <View class="flex-row flex-wrap gap-1.5">
                        <Each of={LOG_FILTERS} key={(f) => f.id}>
                          {(f) => (
                            <Button
                              testID={`fb-logs-filter-${f().id}`}
                              size="sm"
                              label={f().label}
                              variant={logFilter() === f().id ? "primary" : "secondary"}
                              onPress={() => logFilter.set(f().id)}
                            />
                          )}
                        </Each>
                      </View>
                    </View>

                    <If when={logsError() !== null}>
                      <View testID="fb-logs-error" class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
                        <Text class="text-xs text-destructive">{logsError() ?? ""}</Text>
                      </View>
                    </If>

                    <If when={loadingLogs() && commentLogs().length === 0}>
                      <View class="gap-2">
                        <Skeleton class="h-12 w-full" />
                        <Skeleton class="h-12 w-full" />
                      </View>
                    </If>

                    <If when={!loadingLogs() && logsError() === null && filteredLogs().length === 0}>
                      <EmptyState
                        title="Chưa có nhật ký bình luận phù hợp"
                        hint="Khi có bình luận khớp quy tắc, nhật ký xử lý sẽ xuất hiện tại đây."
                      />
                    </If>

                    <If when={filteredLogs().length > 0}>
                      <View class="gap-2">
                        <Each of={filteredLogs()} key={(l) => String(l.id ?? l.comment_id)}>
                          {(l) => (
                            <View
                              testID={`fb-log-row-${l().id}`}
                              class="gap-1.5 rounded-(--radius) border border-border bg-background p-3"
                            >
                              <View class="flex-row flex-wrap items-center justify-between gap-2">
                                <View class="flex-row flex-wrap items-center gap-2">
                                  <Text class="text-sm font-medium text-foreground">
                                    {l().sender_name?.trim() || l().sender_psid}
                                  </Text>
                                  <Text class="text-xs text-muted-foreground">{l().comment_id}</Text>
                                </View>
                                <Text class="text-xs text-muted-foreground">{formatDateTime(l().created_at, "")}</Text>
                              </View>

                              <Text class="text-sm text-foreground">{l().comment_text ?? "(Không có nội dung)"}</Text>

                              <View class="flex-row flex-wrap gap-2">
                                <StatusPill label={`Thích: ${l().like_status}`} tone={logStepTone(l().like_status)} />
                                <StatusPill
                                  label={`Công khai: ${l().public_reply_status}`}
                                  tone={logStepTone(l().public_reply_status)}
                                />
                                <StatusPill
                                  label={`Tin riêng: ${l().private_reply_status}`}
                                  tone={logStepTone(l().private_reply_status)}
                                />
                              </View>

                              <If when={l().skip_reason !== null && l().skip_reason !== ""}>
                                <Text class="text-xs text-muted-foreground">{`Lý do bỏ qua: ${l().skip_reason ?? ""}`}</Text>
                              </If>
                              <If when={l().error_message !== null && l().error_message !== ""}>
                                <Text class="text-xs text-destructive">{l().error_message ?? ""}</Text>
                              </If>
                            </View>
                          )}
                        </Each>
                      </View>
                    </If>
                  </View>
                </View>
              </If>

              {/* ============ SUB-TAB 3: PAGE HEALTH & COMPLIANCE ============ */}
              <If when={activeTab() === "compliance"}>
                <View class="gap-5">
                  {/* Page Health Card */}
                  <View testID="fb-health-card" class="gap-4 rounded-(--radius) border border-border bg-card p-5">
                    <View class="flex-row items-center justify-between">
                      <View class="gap-0.5">
                        <Text class="text-base font-semibold text-foreground">Sức khoẻ Page & Quyền Token</Text>
                        <Text class="text-xs text-muted-foreground">
                          Thông số token, phạm vi quyền (scopes) đã cấp và tải hàng đợi của Fanpage đang chọn.
                        </Text>
                      </View>
                      <Button
                        testID="fb-health-refresh"
                        size="sm"
                        label={loadingHealth() ? "Đang tải…" : "Làm mới"}
                        disabled={loadingHealth()}
                        onPress={() => { void refreshHealth(); }}
                      />
                    </View>

                    <If when={loadingHealth() && pageHealth() === null}>
                      <View class="flex-row gap-3">
                        <Skeleton class="h-20 flex-1" />
                        <Skeleton class="h-20 flex-1" />
                        <Skeleton class="h-20 flex-1" />
                      </View>
                    </If>

                    <If when={healthError() !== null}>
                      <View testID="fb-health-error" class="gap-2 rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
                        <Text class="text-xs text-destructive">{healthError() ?? ""}</Text>
                        <View class="flex-row">
                          <Button
                            testID="fb-health-retry"
                            size="sm"
                            label="Thử lại"
                            disabled={loadingHealth()}
                            onPress={() => { void refreshHealth(); }}
                          />
                        </View>
                      </View>
                    </If>

                    <If when={pageHealth()}>
                      {(h) => (
                        <View class="gap-4">
                          <View class="flex-row flex-wrap gap-3">
                            <MetricCard testID="fb-health-page-id" label="Page ID" value={h().page_id} />
                            <View class="min-w-[160px] flex-1 gap-1.5 rounded-(--radius) border border-border bg-card p-4">
                              <Text class="text-xs font-medium text-muted-foreground">Trạng thái Token</Text>
                              <StatusPill
                                testID="fb-health-token-status"
                                label={tokenStatusLabel(h().token_status)}
                                tone={tokenStatusTone(h().token_status)}
                              />
                            </View>
                            <MetricCard
                              testID="fb-health-scheduled-count"
                              label="Bài đang chờ đăng"
                              value={String(h().active_scheduled_count)}
                            />
                            <MetricCard
                              testID="fb-health-automations-count"
                              label="Quy tắc đang bật"
                              value={String(h().active_automations_count)}
                            />
                          </View>

                          <View class="gap-2 rounded-(--radius) border border-border bg-background p-3">
                            <View class="flex-row items-center justify-between">
                              <Text class="text-xs font-semibold text-foreground">Quyền (Granted Scopes)</Text>
                              <Text testID="fb-health-updated-at" class="text-xs text-muted-foreground">
                                {`Cập nhật: ${formatDateTime(h().updated_at, "—")}`}
                              </Text>
                            </View>
                            <If when={h().granted_scopes.length === 0}>
                              <Text class="text-xs text-muted-foreground">Chưa ghi nhận scope nào từ token.</Text>
                            </If>
                            <If when={h().granted_scopes.length > 0}>
                              <View testID="fb-health-scopes" class="flex-row flex-wrap gap-1.5">
                                <Each of={h().granted_scopes} key={(sc) => sc}>
                                  {(sc) => <StatusPill label={sc()} tone="neutral" />}
                                </Each>
                              </View>
                            </If>
                          </View>
                        </View>
                      )}
                    </If>
                  </View>

                  {/* Compliance & Multi-Device Banner */}
                  <View testID="fb-compliance-card" class="gap-4 rounded-(--radius) border border-border bg-card p-5">
                    <View class="gap-1">
                      <Text class="text-base font-semibold text-foreground">
                        Tuân thủ Xóa dữ liệu Meta & Kiến trúc Đa thiết bị
                      </Text>
                      <Text class="text-xs text-muted-foreground">
                        RustSale tuân thủ chính sách Xóa Dữ Liệu Người Dùng (Data Deletion Callback) của Meta trên các triển khai đa thiết bị cục bộ.
                      </Text>
                    </View>

                    <View class="flex-row flex-wrap gap-3">
                      <View class="min-w-[260px] flex-1 gap-1.5 rounded-(--radius) border border-border bg-muted/30 p-4">
                        <Text class="text-xs font-semibold text-foreground">
                          Cổng xác thực Chữ ký HMAC Signed-Request
                        </Text>
                        <Text class="text-xs text-muted-foreground">
                          Yêu cầu xoá dữ liệu từ Meta được xác thực chữ ký HMAC-SHA256 tại Edge Function và ghi nhận vào bảng meta_data_deletion_requests.
                        </Text>
                      </View>
                      <View class="min-w-[260px] flex-1 gap-1.5 rounded-(--radius) border border-border bg-muted/30 p-4">
                        <Text class="text-xs font-semibold text-foreground">
                          Phân phối Tác vụ Xóa tới Thiết bị Cục bộ
                        </Text>
                        <Text class="text-xs text-muted-foreground">
                          Các máy tính desktop nhận nhiệm vụ xoá qua lease token, dọn sạch bản ghi SQLite và hàng đợi đĩa theo giao dịch nguyên tử.
                        </Text>
                      </View>
                    </View>

                    <View class="gap-1.5 border-t border-border pt-3">
                      <Text class="text-xs font-semibold text-foreground">
                        Điểm cuối Kiểm tra Trạng thái Xóa Dữ liệu
                      </Text>
                      <Text class="text-xs text-muted-foreground">
                        Người dùng Meta có thể kiểm tra trạng thái hoàn tất của yêu cầu xoá dữ liệu qua liên kết xác nhận công khai:
                      </Text>
                      <View class="rounded-(--radius) border border-border bg-muted/50 p-2.5">
                        <Text class="text-xs text-foreground">
                          {"https://<project>.supabase.co/functions/v1/meta-data-deletion/deletion-status?code=<CONFIRMATION_CODE>"}
                        </Text>
                      </View>
                    </View>
                  </View>
                </View>
              </If>
            </View>
          </Scroll>
        </View>
      </If>

      {/* Cancel Scheduled Post Confirmation Modal */}
      <Modal
        open={confirmingCancelPost() !== null}
        title="Huỷ bài viết đã lên lịch?"
        onClose={() => confirmingCancelPost.set(null)}
      >
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">
            {`Bài viết #${confirmingCancelPost()?.id ?? ""} sẽ bị xoá khỏi hàng đợi và không được đăng lên Fanpage.`}
          </Text>
          <View class="flex-row justify-end gap-2">
            <Button
              testID="fb-post-cancel-dismiss"
              label="Giữ lại"
              onPress={() => confirmingCancelPost.set(null)}
            />
            <Button
              testID="fb-post-cancel-confirm"
              variant="destructive"
              label="Huỷ bài viết"
              disabled={actingPostId() !== null}
              onPress={() => { void confirmCancelPost(); }}
            />
          </View>
        </View>
      </Modal>

      {/* Create / Edit Comment Automation Rule Modal */}
      <Modal
        open={ruleModalOpen()}
        title={editingRuleId() !== null ? "Chỉnh sửa Quy tắc Tự động hóa" : "Tạo Quy tắc Tự động hóa Bình luận"}
        onClose={() => ruleModalOpen.set(false)}
        class="m-auto h-[620px] w-[580px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
      >
        <View class="flex-1 gap-3">
          <Scroll class="h-[460px]">
            <View class="gap-3 pr-1">
              <View class="gap-1">
                <Text class="text-xs font-medium text-foreground">Tên Quy tắc</Text>
                <Input
                  testID="fb-rule-name"
                  value={ruleName()}
                  onValue={(v) => ruleName.set(v)}
                  placeholder="Ví dụ: Tự động trả lời báo giá"
                />
              </View>

              <View class="gap-1">
                <Text class="text-xs font-medium text-foreground">Từ khoá kích hoạt (phân cách bằng dấu phẩy)</Text>
                <Input
                  testID="fb-rule-keywords"
                  value={ruleKeywords()}
                  onValue={(v) => ruleKeywords.set(v)}
                  placeholder="giá, bao nhiêu, inbox, tư vấn"
                />
              </View>

              <View class="gap-1">
                <Text class="text-xs font-medium text-foreground">Post IDs áp dụng (tuỳ chọn, phân cách bằng dấu phẩy)</Text>
                <Input
                  testID="fb-rule-post-ids"
                  value={rulePostIds()}
                  onValue={(v) => rulePostIds.set(v)}
                  placeholder="Để trống để áp dụng cho mọi bài viết"
                />
              </View>

              <View class="gap-1.5">
                <Text class="text-xs font-medium text-foreground">Chế độ khớp từ khoá</Text>
                <View class="flex-row flex-wrap gap-1.5">
                  <Each of={MATCH_MODES} key={(m) => m.id}>
                    {(m) => (
                      <Button
                        testID={`fb-rule-mode-${m().id}`}
                        size="sm"
                        label={m().label}
                        variant={ruleMatchMode() === m().id ? "primary" : "secondary"}
                        onPress={() => ruleMatchMode.set(m().id)}
                      />
                    )}
                  </Each>
                </View>
              </View>

              <View class="flex-row flex-wrap gap-4">
                <Checkbox
                  testID="fb-rule-autolike"
                  checked={ruleAutoLike()}
                  onChange={(c) => ruleAutoLike.set(c)}
                  label="Tự động thích bình luận"
                />
                <Checkbox
                  testID="fb-rule-enabled"
                  checked={ruleIsEnabled()}
                  onChange={(c) => ruleIsEnabled.set(c)}
                  label="Bật quy tắc"
                />
              </View>

              <View class="gap-1">
                <Text class="text-xs font-medium text-foreground">
                  {"Mẫu trả lời công khai (tuỳ chọn, hỗ trợ {{name}})"}
                </Text>
                <Input
                  testID="fb-rule-public-reply"
                  multiline
                  value={rulePublicReply()}
                  onValue={(v) => rulePublicReply.set(v)}
                  placeholder="Chào {{name}}, cảm ơn bạn đã quan tâm!"
                  class="h-20 rounded-(--radius) border border-input bg-background p-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
                />
              </View>

              <View class="gap-1">
                <Text class="text-xs font-medium text-foreground">
                  {"Mẫu gửi tin nhắn riêng (tuỳ chọn, hỗ trợ {{name}})"}
                </Text>
                <Input
                  testID="fb-rule-private-reply"
                  multiline
                  value={rulePrivateReply()}
                  onValue={(v) => rulePrivateReply.set(v)}
                  placeholder="Chào {{name}}, shop đã gửi thông tin chi tiết qua tin nhắn cho bạn ạ!"
                  class="h-20 rounded-(--radius) border border-input bg-background p-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
                />
              </View>

              <If when={ruleError() !== null}>
                <View testID="fb-rule-error" class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-2.5">
                  <Text class="text-xs text-destructive">{ruleError() ?? ""}</Text>
                </View>
              </If>
            </View>
          </Scroll>

          <View class="flex-row justify-end gap-2 border-t border-border pt-3">
            <Button
              testID="fb-rule-cancel"
              label="Huỷ"
              onPress={() => ruleModalOpen.set(false)}
            />
            <Button
              testID="fb-rule-save"
              variant="primary"
              label={savingRule() ? "Đang lưu…" : "Lưu Quy tắc"}
              disabled={savingRule()}
              onPress={() => { void saveRule(); }}
            />
          </View>
        </View>
      </Modal>

      {/* Delete Comment Automation Rule Confirmation Modal */}
      <Modal
        open={confirmingDeleteRule() !== null}
        title="Xoá quy tắc bình luận?"
        onClose={() => confirmingDeleteRule.set(null)}
      >
        <View class="gap-3">
          <Text class="text-sm text-muted-foreground">
            {`Quy tắc “${confirmingDeleteRule()?.rule_name ?? ""}” sẽ bị xoá vĩnh viễn.`}
          </Text>
          <View class="flex-row justify-end gap-2">
            <Button
              testID="fb-rule-delete-dismiss"
              label="Giữ lại"
              onPress={() => confirmingDeleteRule.set(null)}
            />
            <Button
              testID="fb-rule-delete-confirm"
              variant="destructive"
              label="Xoá quy tắc"
              disabled={deletingRuleId() !== null}
              onPress={() => { void confirmDeleteRule(); }}
            />
          </View>
        </View>
      </Modal>
    </View>
  );
}
