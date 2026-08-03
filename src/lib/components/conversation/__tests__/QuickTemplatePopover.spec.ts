import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { ConversationQuickTemplate } from '$lib/services/conversationService';
import QuickTemplatePopover from '../QuickTemplatePopover.svelte';

const sampleTemplates: ConversationQuickTemplate[] = [
  {
    id: 101,
    shortcut: 'gia',
    title: 'Bảng giá 2026',
    bodyText: 'Dưới đây là bảng giá dịch vụ năm 2026',
    category: 'Bán hàng',
    isActive: true,
    usageCount: 15,
    lastUsedAt: null,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z'
  },
  {
    id: 102,
    shortcut: 'chao',
    title: 'Lời chào thân thiện',
    bodyText: 'Dạ em chào anh/chị ạ!',
    category: 'CSKH',
    isActive: true,
    usageCount: 30,
    lastUsedAt: null,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z'
  }
];

describe('QuickTemplatePopover', () => {
  afterEach(cleanup);

  it('renders template list ranked by query', () => {
    render(QuickTemplatePopover, {
      templates: sampleTemplates,
      query: 'gia'
    });

    expect(screen.getByTestId('quick-template-popover')).toBeTruthy();
    expect(screen.getByText('Bảng giá 2026')).toBeTruthy();
    expect(screen.getByText('/gia')).toBeTruthy();
    expect(screen.queryByText('Lời chào thân thiện')).toBeNull();
  });

  it('triggers onSelect when clicking a template item', async () => {
    const onSelect = mock();
    render(QuickTemplatePopover, {
      templates: sampleTemplates,
      query: '',
      onSelect
    });

    const item = screen.getByTestId('quick-template-item-101');
    await fireEvent.click(item);

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(sampleTemplates[0]);
  });

  it('highlights the template at selectedIndex', () => {
    render(QuickTemplatePopover, {
      templates: sampleTemplates,
      query: '',
      selectedIndex: 1
    });

    const item101 = screen.getByTestId('quick-template-item-101');
    const item102 = screen.getByTestId('quick-template-item-102');

    // sampleTemplates[1] (item 102) has higher usageCount (30) so it's ranked first (index 0)
    // sampleTemplates[0] (item 101) has usageCount (15) so it's ranked second (index 1)
    expect(item101.getAttribute('aria-selected')).toBe('true');
    expect(item102.getAttribute('aria-selected')).toBe('false');
  });

  it('triggers onOpenManager when clicking manage button', async () => {
    const onOpenManager = mock();
    render(QuickTemplatePopover, {
      templates: sampleTemplates,
      query: '',
      onOpenManager
    });

    const manageBtn = screen.getByTestId('manage-quick-templates-btn');
    await fireEvent.click(manageBtn);

    expect(onOpenManager).toHaveBeenCalledTimes(1);
  });

  it('renders empty message when no templates match query', () => {
    render(QuickTemplatePopover, {
      templates: sampleTemplates,
      query: 'nonexistentquery123'
    });

    expect(screen.getByText('Không tìm thấy mẫu tin nhắn')).toBeTruthy();
  });
});
