/// <reference types="bun-types" />
import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import type {
  BatchImportGroupMembersResult,
  GroupMemberDetail,
  GroupMemberScanResult
} from '$lib/services/conversationService';

const localMembers: GroupMemberDetail[] = [
  {
    identity_id: 1,
    external_id: 'uid-a',
    display_name: 'Member A',
    avatar_url: null,
    participant_role: 'admin',
    lead_id: null,
    lead_name: null,
    categories: []
  }
];

const switchedMembers: GroupMemberDetail[] = [
  {
    identity_id: 2,
    external_id: 'uid-b',
    display_name: 'Member B',
    avatar_url: null,
    participant_role: 'member',
    lead_id: null,
    lead_name: null,
    categories: []
  }
];

function scanResult(overrides: Partial<GroupMemberScanResult> = {}): GroupMemberScanResult {
  return {
    members: localMembers,
    discovered_count: 1,
    profile_hydrated_count: 1,
    membership_complete: false,
    pages_attempted: 2,
    pages_succeeded: 1,
    messages_scanned: 12,
    stop_reason: 'board_page_failed',
    warning: 'Một số nguồn provider chưa hoàn tất.',
    diagnostics: {
      direct_snapshot: {
        attempted: true,
        complete: false,
        member_count: 1,
        warning: null
      },
      board: {
        attempted: true,
        pages_attempted: 2,
        pages_succeeded: 1,
        items_inspected: 4,
        polls_attempted: 1,
        polls_succeeded: 0,
        committed_uid_count: 1,
        stop_reason: 'board_page_failed',
        warnings: [
          {
            source: 'poll',
            code: 'poll_detail_failed',
            context: 'poll_id=9',
            message: 'Không thể đọc chi tiết bình chọn'
          }
        ]
      },
      history_backfill: {
        provider_account_key: 'account-a',
        scope: 'account_group_messages',
        outcome: 'timed_out',
        joined_existing_run: false,
        pages_requested: 5,
        pages_received: 4,
        start_cursor: null,
        durable_cursor: 'cursor-4',
        stop_reason: 'timeout',
        warning: 'Backfill đạt giới hạn thời gian'
      },
      local_evidence: {
        messages_scanned: 12,
        participant_uid_count: 1,
        sender_uid_count: 1,
        mention_uid_count: 1,
        reaction_uid_count: 1,
        quote_owner_uid_count: 1,
        total_uid_count: 1,
        committed_uid_count: 1,
        rejected_uid_count: 0
      },
      profile_hydration: {
        attempted: 1,
        succeeded: 1,
        failed: 0,
        warnings: []
      },
      partial_result: true,
      warnings: [
        {
          stage: 'history_backfill',
          code: 'timed_out',
          context: 'timeout',
          message: 'Backfill đạt giới hạn thời gian'
        }
      ]
    },
    ...overrides
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

const mockGetLocalMembers = mock(async (_conversationId: number) => localMembers);
const mockDeepScan = mock(async (_conversationId: number) => scanResult());
const mockBatchImport = mock(
  async (_request: unknown): Promise<BatchImportGroupMembersResult> => ({
    imported_count: 1,
    updated_count: 0,
    skipped_count: 0,
    assigned_category_ids: []
  })
);
const mockInvoke = mock(async (command: string, _args?: unknown) =>
  command === 'get_categories' ? [] : []
);

mock.module('$lib/services/conversationService', () => ({
  getGroupMembersUnrestricted: (conversationId: number) => mockGetLocalMembers(conversationId),
  syncAndDeepScanGroupMembers: (conversationId: number) => mockDeepScan(conversationId),
  batchImportGroupMembersToLeads: (request: unknown) => mockBatchImport(request)
}));

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: unknown) => mockInvoke(command, args)
}));

const { default: GroupMembersModal } = await import('../GroupMembersModal.svelte');
const renderedViews: Array<{
  rerender: (props: { open?: boolean; conversationId?: number | null; groupTitle?: string }) =>
    | void
    | Promise<void>;
}> = [];

function renderModal(props: Parameters<typeof render<typeof GroupMembersModal>>[1]) {
  const view = render(GroupMembersModal, props);
  renderedViews.push(view);
  return view;
}

describe('GroupMembersModal', () => {
  beforeEach(() => {
    mockGetLocalMembers.mockClear();
    mockGetLocalMembers.mockImplementation(async () => localMembers);
    mockDeepScan.mockClear();
    mockDeepScan.mockImplementation(async () => scanResult());
    mockBatchImport.mockClear();
    mockBatchImport.mockImplementation(async (_request: unknown) => ({
      imported_count: 1,
      updated_count: 0,
      skipped_count: 0,
      assigned_category_ids: []
    }));
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async (command: string, _args?: unknown) =>
      command === 'get_categories' ? [] : []
    );
  });

  afterEach(async () => {
    for (const view of renderedViews.splice(0)) {
      await view.rerender({ open: false, conversationId: null, groupTitle: '' });
    }
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 5));
    cleanup();
    document.body.innerHTML = '';
  });

  test('opening performs local reads only and exposes named dialog semantics', async () => {
    renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP'
    });

    await waitFor(() => expect(mockGetLocalMembers).toHaveBeenCalledWith(30));
    expect(mockDeepScan).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'Nhóm VIP' })).toBeTruthy();
    expect(screen.getByText('Member A')).toBeTruthy();
  });

  test('Deep Scan is explicit, singleflight, and renders account-wide structured diagnostics', async () => {
    const pendingScan = deferred<GroupMemberScanResult>();
    mockDeepScan.mockImplementationOnce(() => pendingScan.promise);
    renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP'
    });
    await screen.findByText('Member A');

    const scanButton = screen.getByRole('button', { name: /Deep Scan/ });
    await fireEvent.click(scanButton);
    await fireEvent.click(scanButton);

    expect(mockDeepScan).toHaveBeenCalledTimes(1);
    expect((scanButton as HTMLButtonElement).disabled).toBe(true);

    pendingScan.resolve(scanResult());
    await waitFor(() => expect(screen.getByText(/Lịch sử WebSocket toàn tài khoản/)).toBeTruthy());
    expect(screen.getByText('Không thể đọc chi tiết bình chọn')).toBeTruthy();
    expect(screen.getByText('Backfill đạt giới hạn thời gian')).toBeTruthy();
  });

  test('distinguishes a complete provider snapshot from observed UID evidence', async () => {
    const completeResult = scanResult({ membership_complete: true });
    completeResult.diagnostics.direct_snapshot.complete = true;
    completeResult.diagnostics.partial_result = false;
    completeResult.diagnostics.warnings = [];
    completeResult.diagnostics.board.warnings = [];
    mockDeepScan.mockImplementationOnce(async () => completeResult);
    renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP'
    });
    await screen.findByText('Member A');

    await fireEvent.click(screen.getByRole('button', { name: /Deep Scan/ }));

    await waitFor(() =>
      expect(screen.getByText(/Danh sách thành viên đầy đủ từ snapshot nhà cung cấp/)).toBeTruthy()
    );
  });

  test('ignores stale local responses after switching conversations', async () => {
    const firstRequest = deferred<GroupMemberDetail[]>();
    mockGetLocalMembers.mockImplementation((conversationId: number) =>
      conversationId === 30 ? firstRequest.promise : Promise.resolve(switchedMembers)
    );
    mockDeepScan.mockImplementation(() => new Promise<GroupMemberScanResult>(() => {}));
    const view = renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm cũ'
    });

    await view.rerender({
      open: true,
      conversationId: 31,
      groupTitle: 'Nhóm mới'
    });
    await screen.findByText('Member B');

    firstRequest.resolve(localMembers);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.queryByText('Member A')).toBeNull();
    expect(screen.getByText('Member B')).toBeTruthy();
  });

  test('ignores a stale Deep Scan response after switching conversations', async () => {
    const pendingScan = deferred<GroupMemberScanResult>();
    mockGetLocalMembers.mockImplementation((conversationId: number) =>
      Promise.resolve(conversationId === 30 ? localMembers : switchedMembers)
    );
    mockDeepScan.mockImplementationOnce(() => pendingScan.promise);
    const view = renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm cũ'
    });
    await screen.findByText('Member A');
    await fireEvent.click(screen.getByRole('button', { name: /Deep Scan/ }));

    await view.rerender({
      open: true,
      conversationId: 31,
      groupTitle: 'Nhóm mới'
    });
    await screen.findByText('Member B');
    pendingScan.resolve(scanResult());
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.queryByText('Member A')).toBeNull();
    expect(screen.getByText('Member B')).toBeTruthy();
  });

  test('post-import refresh stays local and never starts Deep Scan', async () => {
    renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP'
    });
    await screen.findByText('Member A');
    mockDeepScan.mockClear();

    await fireEvent.click(screen.getByRole('button', { name: /Thêm vào Leads & Categories/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Xác Nhận Import' }));

    await waitFor(() => expect(mockBatchImport).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mockGetLocalMembers).toHaveBeenCalledTimes(2));
    expect(mockDeepScan).not.toHaveBeenCalled();
  });

  test('announces loading and errors through live regions', async () => {
    const pending = deferred<GroupMemberDetail[]>();
    mockGetLocalMembers.mockImplementationOnce(() => pending.promise);
    renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP'
    });

    expect(screen.getByRole('status').textContent).toContain('Đang tải');
    pending.reject(new Error('local read failed'));

    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('local read failed'));
  });

  test('Escape closes the dialog and restores focus to the opener', async () => {
    const opener = document.createElement('button');
    opener.textContent = 'Mở thành viên';
    document.body.append(opener);
    opener.focus();
    const onClose = mock();
    const view = renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP',
      onClose
    });
    await screen.findByText('Member A');

    await fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    await view.rerender({
      open: false,
      conversationId: 30,
      groupTitle: 'Nhóm VIP',
      onClose
    });
    await tick();

    expect(document.activeElement).toBe(opener);
  });

  test('in-flight import completion is ignored after the modal closes', async () => {
    const pendingImport = deferred<BatchImportGroupMembersResult>();
    mockBatchImport.mockImplementationOnce(() => pendingImport.promise);
    const onLeadsImported = mock();
    const view = renderModal({
      open: true,
      conversationId: 30,
      groupTitle: 'Nhóm VIP',
      onLeadsImported
    });
    await screen.findByText('Member A');
    await fireEvent.click(screen.getByRole('button', { name: /Thêm vào Leads & Categories/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Xác Nhận Import' }));
    await waitFor(() => expect(mockBatchImport).toHaveBeenCalledTimes(1));

    await view.rerender({
      open: false,
      conversationId: 30,
      groupTitle: 'Nhóm VIP',
      onLeadsImported
    });
    pendingImport.resolve({
      imported_count: 1,
      updated_count: 0,
      skipped_count: 0,
      assigned_category_ids: []
    });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onLeadsImported).not.toHaveBeenCalled();
    expect(mockGetLocalMembers).toHaveBeenCalledTimes(1);
  });
});
