import { invoke } from '@tauri-apps/api/core';

export interface Campaign {
  id?: number;
  name: string;
  description?: string | null;
  channel_type: string;
  status: 'ACTIVE' | 'PAUSED' | 'DRAFT' | string;
  audience_filter_json?: string | null;
  schedule_config_json?: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface CampaignStep {
  id?: number;
  campaign_id?: number;
  step_order: number;
  channel: 'zalo' | 'whatsapp' | 'email' | 'facebook' | string;
  delay_minutes: number;
  condition_type: 'ALWAYS' | 'IF_NOT_REPLIED' | 'IF_DEAL_STAGE_IS' | 'IF_INTENT_IS' | string;
  template_content: string;
  ai_prompt_override?: string | null;
  created_at?: string;
  template_id?: number | null;
}

export interface CampaignExecution {
  id?: number;
  campaign_id: number;
  step_id: number;
  lead_id: number;
  target_phone?: string | null;
  target_account_id?: string | null;
  status: 'PENDING' | 'SENT' | 'DELIVERED' | 'REPLIED' | 'FAILED' | 'SKIPPED' | string;
  error_message?: string | null;
  scheduled_at: string;
  executed_at?: string | null;
  created_at?: string;
}

export interface StepAnalytics {
  step_id: number;
  step_order: number;
  channel: string;
  pending: number;
  sent: number;
  delivered: number;
  replied: number;
  failed: number;
  skipped: number;
}

export interface CampaignAnalytics {
  campaign_id: number;
  campaign_name: string;
  campaign_status: string;
  total_enrolled: number;
  total_steps: number;
  total_executions: number;
  pending_count: number;
  sent_count: number;
  delivered_count: number;
  replied_count: number;
  failed_count: number;
  skipped_count: number;
  converted_count: number;
  response_rate: number;
  conversion_rate: number;
  step_breakdown: StepAnalytics[];
}

export interface SaveCampaignPayload {
  campaign: Campaign;
  steps: CampaignStep[];
}

export async function getAllUnifiedCampaigns(): Promise<Campaign[]> {
  return invoke<Campaign[]>('get_all_unified_campaigns');
}

export async function saveUnifiedCampaign(
  campaign: Campaign,
  steps: CampaignStep[]
): Promise<number> {
  return invoke<number>('save_unified_campaign', { campaign, steps });
}

export async function deleteUnifiedCampaign(campaignId: number): Promise<void> {
  return invoke<void>('delete_unified_campaign', { campaignId });
}

export async function startUnifiedCampaign(campaignId: number): Promise<void> {
  return invoke<void>('start_unified_campaign', { campaignId });
}

export async function pauseUnifiedCampaign(campaignId: number): Promise<void> {
  return invoke<void>('pause_unified_campaign', { campaignId });
}

export async function enrollLeadsIntoCampaign(
  campaignId: number,
  leadIds: number[]
): Promise<number> {
  return invoke<number>('enroll_leads_into_campaign', { campaignId, leadIds });
}

export async function getUnifiedCampaignAnalytics(
  campaignId: number
): Promise<CampaignAnalytics> {
  return invoke<CampaignAnalytics>('get_unified_campaign_analytics', { campaignId });
}
