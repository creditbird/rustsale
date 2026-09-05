import { invoke } from '@tauri-apps/api/core';

export type CustomerIntent =
  | 'asking_price'
  | 'product_inquiry'
  | 'schedule_meeting'
  | 'complaint'
  | 'unsubscribe'
  | 'general_inquiry';

export interface IntentResult {
  intent: CustomerIntent;
  confidence: number;
  detected_language: string;
  matched_keywords: string[];
  reasoning: string;
  is_heuristic_match: boolean;
}

export interface SmartReplyVariant {
  variant_type: 'brief' | 'detailed' | 'promotional' | string;
  title: string;
  content: string;
  suggested_action?: string | null;
  confidence: number;
  policy_decision: 'AUTO_APPROVE' | 'REQUIRE_REVIEW' | 'REJECT_UNSAFE' | string;
  policy_reasons: string[];
}

export type SmartReplySuggestion = SmartReplyVariant;

export interface SmartReplyOutput {
  lead_id: number;
  conversation_id?: number | null;
  intent: IntentResult;
  evidence_summary: string;
  variants: SmartReplyVariant[];
  generated_at: string;
}

export interface AiActionProposal {
  id?: number;
  lead_id?: number | null;
  conversation_id?: number | null;
  platform: string;
  proposed_content: string;
  reasoning: string;
  status: 'PENDING' | 'APPROVED' | 'REJECTED' | 'APPLIED' | string;
  reviewed_by?: string | null;
  reviewed_at?: string | null;
  created_at?: string;
  // Augmented UI fields if joined
  lead_name?: string;
  lead_phone?: string;
}

export interface EvidenceMessage {
  id: number;
  direction: string;
  sender_name?: string | null;
  text_content: string;
  timestamp: string;
}

export interface ProductEvidence {
  name: string;
  price: number;
  currency: string;
  category?: string | null;
  description?: string | null;
}

export interface LeadEvidenceContext {
  lead_id: number;
  lead_name: string;
  lead_company: string;
  lead_phone: string;
  lead_email: string;
  lead_status: string;
  lead_value: number;
  lead_currency: string;
  lead_about?: string | null;
  lead_address?: string | null;
  deal_stage?: string | null;
  deal_value?: number | null;
  deal_title?: string | null;
  journey_stage?: string | null;
  active_campaign_name?: string | null;
  recent_messages: EvidenceMessage[];
  known_products: ProductEvidence[];
  relationship_status?: string | null;
  categories: string[];
  verified_channels: string[];
}

export async function classifyCustomerIntent(text: string): Promise<IntentResult> {
  return invoke<IntentResult>('classify_customer_intent', { text });
}

export async function generateSmartReplySuggestions(
  leadId: number,
  conversationId?: number | null,
  customPrompt?: string | null
): Promise<SmartReplyOutput> {
  return invoke<SmartReplyOutput>('generate_smart_reply_suggestions', {
    leadId,
    conversationId: conversationId ?? null,
    customPrompt: customPrompt ?? null
  });
}

export async function getPendingAiProposals(): Promise<AiActionProposal[]> {
  return invoke<AiActionProposal[]>('get_pending_ai_proposals');
}

export async function reviewAiProposal(
  proposalId: number,
  status: 'APPROVED' | 'REJECTED' | 'APPLIED' | 'PENDING' | string,
  reviewer: string = 'User'
): Promise<void> {
  return invoke<void>('review_ai_proposal', {
    proposalId,
    status,
    reviewer
  });
}

export async function getLeadEvidenceContext(leadId: number): Promise<LeadEvidenceContext> {
  return invoke<LeadEvidenceContext>('get_lead_evidence_context', { leadId });
}

export function getIntentBadgeColor(intent: CustomerIntent | string): {
  bg: string;
  text: string;
  border: string;
  labelVi: string;
} {
  switch (intent) {
    case 'asking_price':
      return {
        bg: 'bg-emerald-500/10 dark:bg-emerald-500/20',
        text: 'text-emerald-700 dark:text-emerald-300',
        border: 'border-emerald-500/30',
        labelVi: 'Hỏi giá / Báo giá'
      };
    case 'product_inquiry':
      return {
        bg: 'bg-blue-500/10 dark:bg-blue-500/20',
        text: 'text-blue-700 dark:text-blue-300',
        border: 'border-blue-500/30',
        labelVi: 'Tư vấn sản phẩm'
      };
    case 'schedule_meeting':
      return {
        bg: 'bg-purple-500/10 dark:bg-purple-500/20',
        text: 'text-purple-700 dark:text-purple-300',
        border: 'border-purple-500/30',
        labelVi: 'Hẹn gặp / Lịch tư vấn'
      };
    case 'complaint':
      return {
        bg: 'bg-rose-500/10 dark:bg-rose-500/20',
        text: 'text-rose-700 dark:text-rose-300',
        border: 'border-rose-500/30',
        labelVi: 'Khiếu nại / Góp ý'
      };
    case 'unsubscribe':
      return {
        bg: 'bg-amber-500/10 dark:bg-amber-500/20',
        text: 'text-amber-700 dark:text-amber-300',
        border: 'border-amber-500/30',
        labelVi: 'Hủy nhận tin'
      };
    case 'general_inquiry':
    default:
      return {
        bg: 'bg-zinc-500/10 dark:bg-zinc-500/20',
        text: 'text-zinc-700 dark:text-zinc-300',
        border: 'border-zinc-500/30',
        labelVi: 'Chào hỏi / Hỏi chung'
      };
  }
}
