import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { expect, test, describe, beforeEach, afterEach, mock } from 'bun:test';
import SequenceBuilder from '../campaign/SequenceBuilder.svelte';
import AICopilotBar from '../conversation/AICopilotBar.svelte';
import type { CampaignStep } from '$lib/services/unifiedCampaignService';

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    if (command === 'classify_customer_intent') {
      return {
        intent: 'asking_price',
        confidence: 0.95,
        detected_language: 'vi',
        matched_keywords: ['giá'],
        reasoning: 'Hỏi giá gói phần mềm',
        is_heuristic_match: true
      };
    }
    if (command === 'generate_smart_reply_suggestions') {
      return {
        lead_id: 1,
        conversation_id: 10,
        intent: {
          intent: 'asking_price',
          confidence: 0.95,
          detected_language: 'vi',
          matched_keywords: [],
          reasoning: 'Hỏi giá',
          is_heuristic_match: true
        },
        evidence_summary: 'Báo giá niêm yết',
        variants: [
          {
            variant_type: 'brief',
            title: 'Ngắn gọn / Thân thiện',
            content: 'Dạ giá là 5tr ạ',
            suggested_action: 'Gửi bảng giá',
            confidence: 0.95,
            policy_decision: 'AUTO_APPROVE',
            policy_reasons: []
          }
        ],
        generated_at: '2026-09-02T12:00:00Z'
      };
    }
    return undefined;
  }
}));

afterEach(() => {
  cleanup();
});

describe('SequenceBuilder Component', () => {
  test('renders empty sequence state when steps array is empty', () => {
    let steps: CampaignStep[] = [];
    render(SequenceBuilder, { props: { steps } });
    expect(screen.getByText('Chưa có bước nào trong kịch bản')).toBeTruthy();
    expect(screen.getByText('Tạo Bước Đầu Tiên')).toBeTruthy();
  });

  test('renders step cards with channels and delay presets', () => {
    let steps: CampaignStep[] = [
      {
        step_order: 1,
        channel: 'zalo',
        delay_minutes: 0,
        condition_type: 'ALWAYS',
        template_content: 'Xin chào {{name}} từ {{company}}'
      },
      {
        step_order: 2,
        channel: 'whatsapp',
        delay_minutes: 1440,
        condition_type: 'IF_NOT_REPLIED',
        template_content: 'Chào bạn, bạn đã xem tin nhắn chưa?'
      }
    ];

    render(SequenceBuilder, { props: { steps } });
    expect(screen.getByText('Bước 1')).toBeTruthy();
    expect(screen.getByText('Bước 2')).toBeTruthy();
    expect(screen.getByText('Ngay khi bắt đầu')).toBeTruthy();
    expect(screen.getByText('Sau 1 ngày')).toBeTruthy();
  });
});

describe('AICopilotBar Component', () => {
  test('renders AICopilotBar with auto-classified intent badge for inbound message', async () => {
    render(AICopilotBar, {
      props: {
        leadId: 1,
        conversationId: 10,
        lastMessageText: 'Gói này giá bao nhiêu bạn ơi?'
      }
    });

    expect(screen.getByText('AI Copilot')).toBeTruthy();
    expect(screen.getByText('Gợi Ý Trả Lời AI')).toBeTruthy();
  });
});
