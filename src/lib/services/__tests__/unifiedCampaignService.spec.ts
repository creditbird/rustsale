import { beforeEach, describe, expect, mock, test } from 'bun:test';

let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokedCommands.push({ command, args });
    if (command === 'get_all_unified_campaigns') {
      return [
        {
          id: 1,
          name: 'Nurture Lead Sequence',
          description: 'Omnichannel lead nurture',
          channel_type: 'omnichannel',
          status: 'ACTIVE',
          created_at: '2026-09-02T10:00:00Z',
          updated_at: '2026-09-02T10:00:00Z'
        }
      ];
    }
    if (command === 'save_unified_campaign') {
      return 101;
    }
    if (command === 'enroll_leads_into_campaign') {
      const ids = (args?.leadIds as number[]) || [];
      return ids.length;
    }
    if (command === 'get_unified_campaign_analytics') {
      return {
        campaign_id: args?.campaignId ?? 1,
        campaign_name: 'Nurture Lead Sequence',
        campaign_status: 'ACTIVE',
        total_enrolled: 50,
        total_steps: 3,
        total_executions: 120,
        pending_count: 20,
        sent_count: 50,
        delivered_count: 45,
        replied_count: 15,
        failed_count: 5,
        skipped_count: 0,
        converted_count: 8,
        response_rate: 30.0,
        conversion_rate: 16.0,
        step_breakdown: [
          {
            step_id: 1,
            step_order: 1,
            channel: 'zalo',
            pending: 0,
            sent: 50,
            delivered: 48,
            replied: 10,
            failed: 2,
            skipped: 0
          }
        ]
      };
    }
    return undefined;
  }
}));

import {
  getAllUnifiedCampaigns,
  saveUnifiedCampaign,
  deleteUnifiedCampaign,
  startUnifiedCampaign,
  pauseUnifiedCampaign,
  enrollLeadsIntoCampaign,
  getUnifiedCampaignAnalytics,
  type Campaign,
  type CampaignStep
} from '../unifiedCampaignService';

beforeEach(() => {
  invokedCommands = [];
});

describe('unifiedCampaignService', () => {
  test('getAllUnifiedCampaigns fetches campaigns list', async () => {
    const list = await getAllUnifiedCampaigns();
    expect(list.length).toBe(1);
    expect(list[0].name).toBe('Nurture Lead Sequence');
    expect(invokedCommands).toEqual([
      { command: 'get_all_unified_campaigns', args: undefined }
    ]);
  });

  test('saveUnifiedCampaign sends campaign payload and steps to Rust core', async () => {
    const campaign: Campaign = {
      name: 'Black Friday Campaign',
      channel_type: 'omnichannel',
      status: 'DRAFT'
    };
    const steps: CampaignStep[] = [
      {
        step_order: 1,
        channel: 'zalo',
        delay_minutes: 0,
        condition_type: 'ALWAYS',
        template_content: 'Chào {{name}}, ưu đãi 20% hôm nay!'
      },
      {
        step_order: 2,
        channel: 'whatsapp',
        delay_minutes: 1440,
        condition_type: 'IF_NOT_REPLIED',
        template_content: 'Chào {{name}}, bạn đã nhận được mã giảm giá chưa?'
      }
    ];

    const campaignId = await saveUnifiedCampaign(campaign, steps);
    expect(campaignId).toBe(101);
    expect(invokedCommands).toEqual([
      {
        command: 'save_unified_campaign',
        args: { campaign, steps }
      }
    ]);
  });

  test('deleteUnifiedCampaign invokes delete command with ID', async () => {
    await deleteUnifiedCampaign(42);
    expect(invokedCommands).toEqual([
      {
        command: 'delete_unified_campaign',
        args: { campaignId: 42 }
      }
    ]);
  });

  test('startUnifiedCampaign activates sequence execution', async () => {
    await startUnifiedCampaign(10);
    expect(invokedCommands).toEqual([
      {
        command: 'start_unified_campaign',
        args: { campaignId: 10 }
      }
    ]);
  });

  test('pauseUnifiedCampaign stops sequence dispatching', async () => {
    await pauseUnifiedCampaign(10);
    expect(invokedCommands).toEqual([
      {
        command: 'pause_unified_campaign',
        args: { campaignId: 10 }
      }
    ]);
  });

  test('enrollLeadsIntoCampaign enrolls multiple lead IDs', async () => {
    const enrolled = await enrollLeadsIntoCampaign(10, [101, 102, 103, 104]);
    expect(enrolled).toBe(4);
    expect(invokedCommands).toEqual([
      {
        command: 'enroll_leads_into_campaign',
        args: { campaignId: 10, leadIds: [101, 102, 103, 104] }
      }
    ]);
  });

  test('getUnifiedCampaignAnalytics retrieves funnel metrics', async () => {
    const analytics = await getUnifiedCampaignAnalytics(1);
    expect(analytics.campaign_id).toBe(1);
    expect(analytics.total_enrolled).toBe(50);
    expect(analytics.response_rate).toBe(30.0);
    expect(analytics.conversion_rate).toBe(16.0);
    expect(analytics.step_breakdown.length).toBe(1);
    expect(invokedCommands).toEqual([
      {
        command: 'get_unified_campaign_analytics',
        args: { campaignId: 1 }
      }
    ]);
  });
});
