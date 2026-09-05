import { beforeEach, describe, expect, mock, test } from 'bun:test';

let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokedCommands.push({ command, args });
    if (command === 'classify_customer_intent') {
      return {
        intent: 'asking_price',
        confidence: 0.95,
        detected_language: 'vi',
        matched_keywords: ['báo giá', 'giá bao nhiêu'],
        reasoning: 'Customer asked for price',
        is_heuristic_match: true
      };
    }
    if (command === 'generate_smart_reply_suggestions') {
      return {
        lead_id: args?.leadId ?? 1,
        conversation_id: args?.conversationId ?? null,
        intent: {
          intent: 'asking_price',
          confidence: 0.95,
          detected_language: 'vi',
          matched_keywords: [],
          reasoning: 'Price quote requested',
          is_heuristic_match: true
        },
        evidence_summary: 'Lead requested software pricing',
        variants: [
          {
            variant_type: 'brief',
            title: 'Ngắn gọn / Thân thiện',
            content: 'Dạ em chào anh, giá phần mềm là 5.000.000đ ạ.',
            suggested_action: 'Gửi bảng giá',
            confidence: 0.95,
            policy_decision: 'AUTO_APPROVE',
            policy_reasons: []
          },
          {
            variant_type: 'detailed',
            title: 'Chi tiết / Đầy đủ',
            content: 'Dạ em gửi anh bảng báo giá chi tiết kèm hợp đồng mẫu qua email.',
            suggested_action: 'Gửi báo giá PDF',
            confidence: 0.90,
            policy_decision: 'AUTO_APPROVE',
            policy_reasons: []
          },
          {
            variant_type: 'promotional',
            title: 'Kêu gọi hành động',
            content: 'Dạ hôm nay bên em có ưu đãi giảm 10%, anh xem thử nhé!',
            suggested_action: 'Mời hẹn demo',
            confidence: 0.88,
            policy_decision: 'REQUIRE_REVIEW',
            policy_reasons: ['Contains promotional discount claim']
          }
        ],
        generated_at: '2026-09-02T12:00:00Z'
      };
    }
    if (command === 'get_pending_ai_proposals') {
      return [
        {
          id: 1,
          lead_id: 10,
          platform: 'zalo',
          proposed_content: 'Chào anh, bên em xin phép gửi báo giá.',
          reasoning: 'Customer asked for quote',
          status: 'PENDING',
          created_at: '2026-09-02T12:00:00Z'
        }
      ];
    }
    if (command === 'get_lead_evidence_context') {
      return {
        lead_id: args?.leadId ?? 10,
        lead_name: 'Nguyễn Văn A',
        lead_company: 'Công ty ABC',
        lead_phone: '0901234567',
        lead_email: 'a@abc.com',
        lead_status: 'New',
        lead_value: 10000000,
        lead_currency: 'VND',
        recent_messages: [],
        known_products: [],
        categories: ['Công nghệ'],
        verified_channels: ['zalo']
      };
    }
    return undefined;
  }
}));

import {
  classifyCustomerIntent,
  generateSmartReplySuggestions,
  getPendingAiProposals,
  reviewAiProposal,
  getLeadEvidenceContext,
  getIntentBadgeColor
} from '../aiAgentService';

beforeEach(() => {
  invokedCommands = [];
});

describe('aiAgentService', () => {
  test('classifyCustomerIntent classifies customer text into intent object', async () => {
    const res = await classifyCustomerIntent('Giá gói phần mềm này bao nhiêu shop?');
    expect(res.intent).toBe('asking_price');
    expect(res.confidence).toBe(0.95);
    expect(res.matched_keywords).toContain('báo giá');
    expect(invokedCommands).toEqual([
      {
        command: 'classify_customer_intent',
        args: { text: 'Giá gói phần mềm này bao nhiêu shop?' }
      }
    ]);
  });

  test('generateSmartReplySuggestions returns 3 verified reply variants', async () => {
    const res = await generateSmartReplySuggestions(10, 5, 'Nhấn mạnh ưu đãi');
    expect(res.lead_id).toBe(10);
    expect(res.variants.length).toBe(3);
    expect(res.variants[0].variant_type).toBe('brief');
    expect(res.variants[1].variant_type).toBe('detailed');
    expect(res.variants[2].variant_type).toBe('promotional');
    expect(invokedCommands).toEqual([
      {
        command: 'generate_smart_reply_suggestions',
        args: {
          leadId: 10,
          conversationId: 5,
          customPrompt: 'Nhấn mạnh ưu đãi'
        }
      }
    ]);
  });

  test('getPendingAiProposals retrieves proposals awaiting review', async () => {
    const proposals = await getPendingAiProposals();
    expect(proposals.length).toBe(1);
    expect(proposals[0].status).toBe('PENDING');
    expect(invokedCommands).toEqual([
      {
        command: 'get_pending_ai_proposals',
        args: undefined
      }
    ]);
  });

  test('reviewAiProposal approves a proposal', async () => {
    await reviewAiProposal(1, 'APPROVED', 'Sale Rep 01');
    expect(invokedCommands).toEqual([
      {
        command: 'review_ai_proposal',
        args: {
          proposalId: 1,
          status: 'APPROVED',
          reviewer: 'Sale Rep 01'
        }
      }
    ]);
  });

  test('reviewAiProposal rejects a proposal', async () => {
    await reviewAiProposal(2, 'REJECTED', 'Manager');
    expect(invokedCommands).toEqual([
      {
        command: 'review_ai_proposal',
        args: {
          proposalId: 2,
          status: 'REJECTED',
          reviewer: 'Manager'
        }
      }
    ]);
  });

  test('getLeadEvidenceContext extracts CRM ledger facts', async () => {
    const ctx = await getLeadEvidenceContext(10);
    expect(ctx.lead_id).toBe(10);
    expect(ctx.lead_name).toBe('Nguyễn Văn A');
    expect(ctx.lead_company).toBe('Công ty ABC');
    expect(invokedCommands).toEqual([
      {
        command: 'get_lead_evidence_context',
        args: { leadId: 10 }
      }
    ]);
  });

  test('getIntentBadgeColor returns correct styling and Vietnamese labels for all intents', () => {
    expect(getIntentBadgeColor('asking_price').labelVi).toBe('Hỏi giá / Báo giá');
    expect(getIntentBadgeColor('product_inquiry').labelVi).toBe('Tư vấn sản phẩm');
    expect(getIntentBadgeColor('schedule_meeting').labelVi).toBe('Hẹn gặp / Lịch tư vấn');
    expect(getIntentBadgeColor('complaint').labelVi).toBe('Khiếu nại / Góp ý');
    expect(getIntentBadgeColor('unsubscribe').labelVi).toBe('Hủy nhận tin');
    expect(getIntentBadgeColor('general_inquiry').labelVi).toBe('Chào hỏi / Hỏi chung');
    expect(getIntentBadgeColor('unknown').labelVi).toBe('Chào hỏi / Hỏi chung');
  });
});
