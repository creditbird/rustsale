import { invoke } from '@tauri-apps/api/core';

export interface FacebookScheduledPost {
  id: number;
  draft_group_id: string;
  channel_account_id: number;
  page_id: string;
  post_type: 'status' | 'link' | 'photo' | 'multi_photo' | 'video' | 'reel';
  message: string;
  link_url: string | null;
  media_paths: string[];
  scheduling_mode: 'meta' | 'local_queued';
  scheduled_publish_time: string | null;
  remote_post_id: string | null;
  status: 'draft' | 'queued' | 'publishing' | 'scheduled_on_meta' | 'published' | 'failed' | 'delivery_unknown' | 'overdue_prompt';
  error_code: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateFacebookPostPayload {
  draft_group_id?: string;
  channel_account_id: number;
  page_id: string;
  post_type: string;
  message: string;
  link_url?: string | null;
  media_paths?: string[];
  scheduled_publish_time?: string | null;
}

export interface CommentAutomationRule {
  id: number;
  channel_account_id: number;
  page_id: string;
  rule_name: string;
  post_ids: string[];
  keywords: string[];
  match_mode: 'contains' | 'exact' | 'regex' | 'all';
  auto_like: boolean;
  public_reply_template: string | null;
  private_reply_template: string | null;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateCommentAutomationPayload {
  channel_account_id: number;
  page_id: string;
  rule_name: string;
  post_ids?: string[];
  keywords?: string[];
  match_mode?: string;
  auto_like?: boolean;
  public_reply_template?: string | null;
  private_reply_template?: string | null;
  is_enabled?: boolean;
}

export interface UpdateCommentAutomationPayload {
  id: number;
  rule_name: string;
  post_ids?: string[];
  keywords?: string[];
  match_mode?: string;
  auto_like?: boolean;
  public_reply_template?: string | null;
  private_reply_template?: string | null;
  is_enabled?: boolean;
}

export interface CommentLog {
  id: number;
  comment_id: string;
  page_id: string;
  post_id: string | null;
  sender_psid: string;
  sender_name: string | null;
  comment_text: string | null;
  automation_id: number | null;
  like_status: string;
  public_reply_status: string;
  private_reply_status: string;
  skip_reason: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageHealth {
  page_id: string;
  token_status: string;
  granted_scopes: string[];
  active_scheduled_count: number;
  active_automations_count: number;
  updated_at: string;
}

export async function scheduleFacebookPost(request: CreateFacebookPostPayload): Promise<number> {
  return await invoke<number>('schedule_facebook_post', { request });
}

export async function publishFacebookPostNow(postId: number): Promise<boolean> {
  return await invoke<boolean>('publish_facebook_post_now', { postId });
}

export async function listFacebookScheduledPosts(pageId?: string): Promise<FacebookScheduledPost[]> {
  return await invoke<FacebookScheduledPost[]>('list_facebook_scheduled_posts', { pageId });
}

export async function cancelFacebookScheduledPost(postId: number): Promise<boolean> {
  return await invoke<boolean>('cancel_facebook_scheduled_post', { postId });
}

export async function createCommentAutomation(req: CreateCommentAutomationPayload): Promise<number> {
  return await invoke<number>('create_comment_automation', { req });
}

export async function listCommentAutomations(pageId?: string): Promise<CommentAutomationRule[]> {
  return await invoke<CommentAutomationRule[]>('list_comment_automations', { pageId });
}

export async function updateCommentAutomation(req: UpdateCommentAutomationPayload): Promise<boolean> {
  return await invoke<boolean>('update_comment_automation', { req });
}

export async function deleteCommentAutomation(id: number): Promise<boolean> {
  return await invoke<boolean>('delete_comment_automation', { id });
}

export async function listCommentLogs(pageId?: string, limit?: number): Promise<CommentLog[]> {
  return await invoke<CommentLog[]>('list_comment_logs', { pageId, limit });
}

export async function getFacebookPageHealth(pageId: string): Promise<PageHealth> {
  return await invoke<PageHealth>('get_facebook_page_health', { pageId });
}
