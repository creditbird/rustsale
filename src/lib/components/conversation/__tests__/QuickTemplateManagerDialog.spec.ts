import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { ConversationQuickTemplate } from '$lib/services/conversationService';
import QuickTemplateManagerDialog from '../QuickTemplateManagerDialog.svelte';

const sampleTemplates: ConversationQuickTemplate[] = [
  {
    id: 1,
    shortcut: 'gia',
    title: 'Bảng giá',
    bodyText: 'Nội dung bảng giá',
    category: 'Bán hàng',
    isActive: true,
    usageCount: 10,
    lastUsedAt: null,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z'
  },
  {
    id: 2,
    shortcut: 'chao',
    title: 'Lời chào',
    bodyText: 'Nội dung lời chào',
    category: null,
    isActive: false,
    usageCount: 5,
    lastUsedAt: null,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z'
  }
];

describe('QuickTemplateManagerDialog', () => {
  afterEach(cleanup);

  it('renders open manager dialog with item count and template list', () => {
    render(QuickTemplateManagerDialog, {
      open: true,
      templates: sampleTemplates
    });

    expect(screen.getByText('Manage Quick Templates')).toBeTruthy();
    expect(screen.getByText('2 / 500 templates')).toBeTruthy();
    expect(screen.getByText('Bảng giá')).toBeTruthy();
    expect(screen.getByText('Lời chào')).toBeTruthy();
  });

  it('filters templates live via search input', async () => {
    render(QuickTemplateManagerDialog, {
      open: true,
      templates: sampleTemplates
    });

    const searchInput = screen.getByTestId('template-search-input');
    await fireEvent.input(searchInput, { target: { value: 'chao' } });

    expect(screen.queryByText('Bảng giá')).toBeNull();
    expect(screen.getByText('Lời chào')).toBeTruthy();
  });

  it('toggles template active status', async () => {
    const onToggleActive = mock();
    render(QuickTemplateManagerDialog, {
      open: true,
      templates: sampleTemplates,
      onToggleActive
    });

    const toggleBtn = screen.getByTestId('toggle-active-btn-1');
    await fireEvent.click(toggleBtn);

    expect(onToggleActive).toHaveBeenCalledTimes(1);
    expect(onToggleActive).toHaveBeenCalledWith(sampleTemplates[0], false);
  });

  it('deletes a template after confirmation or direct click', async () => {
    const onDelete = mock();
    render(QuickTemplateManagerDialog, {
      open: true,
      templates: sampleTemplates,
      onDelete
    });

    const deleteBtn = screen.getByTestId('delete-template-btn-1');
    await fireEvent.click(deleteBtn);

    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onDelete).toHaveBeenCalledWith(1);
  });

  it('displays 500-item limit warning and disables create button when count >= 500', () => {
    const manyTemplates = Array.from({ length: 500 }, (_, i) => ({
      ...sampleTemplates[0],
      id: i + 1,
      shortcut: `s${i}`
    }));

    render(QuickTemplateManagerDialog, {
      open: true,
      templates: manyTemplates
    });

    expect(screen.getByTestId('limit-warning-banner')).toBeTruthy();
    const addBtn = screen.getByTestId('add-template-btn') as HTMLButtonElement;
    expect(addBtn.disabled).toBe(true);
  });
});
