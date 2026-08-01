import { beforeEach, describe, expect, mock, test } from 'bun:test';

let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokedCommands.push({ command, args });
    if (command === 'get_managed_zalo_accounts' || command === 'get_managed_smtp_accounts') {
      return [];
    }
    if (command === 'verify_zalo_session') {
      return { imei: args?.imei, is_valid: true, display_name: 'Hello World' };
    }
    if (command === 'save_smtp_server') return 7;
    if (command === 'test_smtp_connection') return 'Connected';
    return undefined;
  }
}));

import {
  deleteSmtpAccount,
  deleteZaloAccount,
  listManagedSmtpAccounts,
  listManagedZaloAccounts,
  reconnectZaloAccount,
  saveSmtpAccount,
  startZaloQrLogin,
  testSmtpConnection,
  verifyZaloAccount
} from '../accountSettingsService';

beforeEach(() => {
  invokedCommands = [];
});

describe('accountSettingsService', () => {
  test('lists redacted managed accounts', async () => {
    expect(await listManagedZaloAccounts()).toEqual([]);
    expect(await listManagedSmtpAccounts()).toEqual([]);
    expect(invokedCommands).toEqual([
      { command: 'get_managed_zalo_accounts', args: undefined },
      { command: 'get_managed_smtp_accounts', args: undefined }
    ]);
  });

  test('wraps Zalo account lifecycle commands', async () => {
    await startZaloQrLogin();
    await verifyZaloAccount('imei-1');
    await reconnectZaloAccount('imei-1');
    await deleteZaloAccount('imei-1');

    expect(invokedCommands).toEqual([
      { command: 'start_zalo_qr_login', args: undefined },
      { command: 'verify_zalo_session', args: { imei: 'imei-1' } },
      {
        command: 'restart_conversation_account',
        args: { providerAccountKey: 'imei-1' }
      },
      { command: 'delete_zalo_session', args: { imei: 'imei-1' } }
    ]);
  });

  test('wraps SMTP lifecycle commands without requiring an existing password', async () => {
    const server = {
      id: 7,
      name: 'Primary SMTP',
      host: 'smtp.example.com',
      port: 587,
      username: 'sender@example.com',
      password: '',
      from_email: 'sender@example.com',
      from_name: 'Sales',
      security: 'STARTTLS',
      created_at: ''
    };

    expect(await saveSmtpAccount(server)).toBe(7);
    expect(await testSmtpConnection(server.host, server.port)).toBe('Connected');
    await deleteSmtpAccount(7);

    expect(invokedCommands).toEqual([
      { command: 'save_smtp_server', args: { server } },
      {
        command: 'test_smtp_connection',
        args: { host: 'smtp.example.com', port: 587 }
      },
      { command: 'delete_smtp_server', args: { id: 7 } }
    ]);
  });
});
