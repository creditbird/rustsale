import { invoke } from '@tauri-apps/api/core';

export type ZaloAuthStatus = 'unknown' | 'checking' | 'valid' | 'expired' | 'error';
export type AccountRuntimeStatus =
  | 'not_applicable'
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'invalid';

export interface ManagedZaloAccount {
  accountKey: string;
  displayName: string | null;
  avatar: string | null;
  authStatus: ZaloAuthStatus;
  authCheckedAt: string | null;
  authError: string | null;
  runtimeStatus: AccountRuntimeStatus;
  lastConnectedAt: string | null;
  lastError: string | null;
}

export interface ZaloSessionStatus {
  imei: string;
  is_valid: boolean;
  display_name?: string | null;
  avatar?: string | null;
  error_message?: string | null;
}

export interface ManagedSmtpAccount {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  fromEmail: string;
  fromName: string;
  security: string;
}

export interface SmtpServerInput {
  id?: number;
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  from_email: string;
  from_name: string;
  security: string;
  created_at: string;
}

export interface ManagedFacebookAccount {
  channelAccountId: number;
  pageId: string;
  pageName: string | null;
  avatarUrl: string | null;
  tokenStatus: 'valid' | 'expired' | 'invalid' | 'revoked';
  grantedScopes: string[];
  runtimeStatus: AccountRuntimeStatus;
  lastConnectedAt: string | null;
  lastError: string | null;
  updatedAt: string;
}

export const listManagedFacebookAccounts = () =>
  invoke<ManagedFacebookAccount[]>('get_managed_facebook_accounts');

export const deleteFacebookAccount = (pageId: string) =>
  invoke<void>('delete_facebook_account', { pageId });

export const listManagedZaloAccounts = () =>
  invoke<ManagedZaloAccount[]>('get_managed_zalo_accounts');

export const listManagedSmtpAccounts = () =>
  invoke<ManagedSmtpAccount[]>('get_managed_smtp_accounts');

export const startZaloQrLogin = () => invoke<void>('start_zalo_qr_login');

export const verifyZaloAccount = (imei: string) =>
  invoke<ZaloSessionStatus>('verify_zalo_session', { imei });

export const reconnectZaloAccount = (providerAccountKey: string) =>
  invoke<void>('restart_conversation_account', { providerAccountKey });

export const deleteZaloAccount = (imei: string) =>
  invoke<void>('delete_zalo_session', { imei });

export const saveSmtpAccount = (server: SmtpServerInput) =>
  invoke<number>('save_smtp_server', { server });

export const deleteSmtpAccount = (id: number) =>
  invoke<void>('delete_smtp_server', { id });

export const testSmtpConnection = (host: string, port: number) =>
  invoke<string>('test_smtp_connection', { host, port });
