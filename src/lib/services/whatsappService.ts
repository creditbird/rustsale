import { invoke } from '@tauri-apps/api/core';

export const EVENT_WHATSAPP_QR_CODE = 'whatsapp-qr-code';
export const EVENT_WHATSAPP_LOGIN_SUCCESS = 'whatsapp-login-success';
export const EVENT_WHATSAPP_LOGIN_ERROR = 'whatsapp-login-error';

export interface ConnectedWhatsAppAccount {
  id: number;
  provider_account_key: string;
  display_name: string | null;
  phone_number: string | null;
  // Optional camelCase aliases for interoperability
  providerAccountKey?: string;
  displayName?: string | null;
  phoneNumber?: string | null;
}

export interface WhatsAppQrCodePayload {
  attemptId: string;
  qrRaw: string;
  expiresInSec: number;
  sequence: number;
  attempt_id?: string;
  qr_raw?: string;
  expires_in_sec?: number;
}

export interface WhatsAppLoginSuccessPayload {
  attemptId: string;
  accountId: number;
  accountKey: string;
  phoneNumber: string;
  pushName?: string | null;
  lid?: string | null;
  avatarUrl?: string | null;
  attempt_id?: string;
  account_id?: number;
  account_key?: string;
  phone_number?: string;
  push_name?: string | null;
  avatar_url?: string | null;
}

export interface WhatsAppLoginErrorPayload {
  attemptId: string;
  error: string;
  attempt_id?: string;
}

export interface CreateWhatsAppCampaignRequest {
  name: string;
  template_id?: number | null;
  assigned_account_ids: number[];
  rate_limit_min_delay_secs?: number | null;
  rate_limit_max_delay_secs?: number | null;
  burst_limit_per_batch?: number | null;
  burst_pause_secs?: number | null;
  quiet_hours_enabled?: boolean | null;
  quiet_hours_start?: string | null;
  quiet_hours_end?: string | null;
  timezone?: string | null;
}

export interface CampaignRecipientInput {
  lead_id?: number | null;
  recipient_phone_raw: string;
  template?: string | null;
  lead_vars?: Record<string, string> | null;
}

export interface WhatsAppCampaignRecord {
  id: number;
  name: string;
  template_id: number | null;
  status: string;
  total_recipients: number;
  sent_count: number;
  delivered_count: number;
  read_count: number;
  failed_count: number;
  skipped_count: number;
  assigned_account_ids: number[];
  rate_limit_min_delay_secs: number;
  rate_limit_max_delay_secs: number;
  burst_limit_per_batch: number;
  burst_pause_secs: number;
  quiet_hours_enabled: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
  timezone: string;
  last_heartbeat_at: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CampaignRecipientRecord {
  id: number;
  campaign_id: number;
  lead_id: number | null;
  recipient_phone_raw: string;
  recipient_phone_e164: string;
  recipient_jid: string;
  assigned_channel_account_id: number | null;
  status: string;
  skip_reason: string | null;
  rendered_text: string | null;
  rendered_image_url: string | null;
  message_id: number | null;
  outbox_job_id: string | null;
  attempt_count: number;
  last_attempt_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export const listManagedWhatsAppAccounts = (): Promise<ConnectedWhatsAppAccount[]> =>
  invoke<ConnectedWhatsAppAccount[]>('get_managed_whatsapp_accounts');

export const startWhatsAppQrLogin = (attemptId: string): Promise<void> =>
  invoke<void>('start_whatsapp_qr_login', { attemptId });

export const cancelWhatsAppQrLogin = (attemptId: string): Promise<void> =>
  invoke<void>('cancel_whatsapp_qr_login', { attemptId });

export const disconnectWhatsAppAccount = (channelAccountId: number): Promise<void> =>
  invoke<void>('disconnect_whatsapp_account', { channelAccountId });

export const createWhatsAppCampaign = (req: CreateWhatsAppCampaignRequest): Promise<number> =>
  invoke<number>('create_whatsapp_campaign', { req });

export const populateWhatsAppCampaignRecipients = (
  campaignId: number,
  recipients: CampaignRecipientInput[],
  template?: string
): Promise<number> =>
  invoke<number>('populate_whatsapp_campaign_recipients', {
    campaignId,
    recipients,
    template: template ?? null
  });

export const startWhatsAppCampaign = (campaignId: number): Promise<void> =>
  invoke<void>('start_whatsapp_campaign', { campaignId });

export const pauseWhatsAppCampaign = (campaignId: number): Promise<void> =>
  invoke<void>('pause_whatsapp_campaign', { campaignId });

export const resumeWhatsAppCampaign = (campaignId: number): Promise<void> =>
  invoke<void>('resume_whatsapp_campaign', { campaignId });

export const cancelWhatsAppCampaign = (campaignId: number): Promise<void> =>
  invoke<void>('cancel_whatsapp_campaign', { campaignId });

export const getWhatsAppCampaign = (campaignId: number): Promise<WhatsAppCampaignRecord> =>
  invoke<WhatsAppCampaignRecord>('get_whatsapp_campaign', { campaignId });

export const listWhatsAppCampaigns = (): Promise<WhatsAppCampaignRecord[]> =>
  invoke<WhatsAppCampaignRecord[]>('list_whatsapp_campaigns');

export const getWhatsAppCampaignRecipients = (
  campaignId: number,
  limit?: number,
  offset?: number
): Promise<CampaignRecipientRecord[]> =>
  invoke<CampaignRecipientRecord[]>('get_whatsapp_campaign_recipients', {
    campaignId,
    limit: limit ?? null,
    offset: offset ?? null
  });

/**
 * Parses single-level or nested Spintax syntax: `{Option A|Option B|Option C}`.
 */
export function parseSpintax(input: string): string {
  let text = input;
  const regex = /\{([^{}]+)\}/;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(text)) !== null) {
    const options = match[1].split('|');
    const chosen = options.length > 0 ? options[Math.floor(Math.random() * options.length)] : '';
    text = text.slice(0, match.index) + chosen + text.slice(match.index + match[0].length);
  }
  return text;
}

/**
 * Replaces lead variable tokens like `[name]`, `[phone]`, `[company]`, `[email]`, `[notes]`
 * and evaluates Spintax `{Option 1|Option 2}` variations.
 */
export function renderSpintaxTemplate(
  template: string,
  leadVars: Record<string, string> = {}
): string {
  let text = template;
  for (const [key, val] of Object.entries(leadVars)) {
    const regex = new RegExp(`\\[${key}\\]`, 'gi');
    text = text.replace(regex, val);
  }
  return parseSpintax(text);
}

/**
 * Normalizes raw phone strings to standard E.164 (+<country_code><subscriber_number>).
 */
export function normalizeWhatsAppPhone(raw: string, defaultCountry: string = '84'): string | null {
  const digits = raw.replace(/[^\d+]/g, '');
  if (!digits) return null;

  let cleaned = digits;
  if (cleaned.startsWith('+')) {
    cleaned = cleaned.slice(1);
  } else if (cleaned.startsWith('00')) {
    cleaned = cleaned.slice(2);
  } else if (cleaned.startsWith('0')) {
    cleaned = defaultCountry + cleaned.slice(1);
  } else if (!cleaned.startsWith(defaultCountry) && cleaned.length <= 10) {
    cleaned = defaultCountry + cleaned;
  }

  if (cleaned.length >= 8 && cleaned.length <= 15) {
    return '+' + cleaned;
  }
  return null;
}

