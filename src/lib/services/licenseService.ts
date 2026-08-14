import { getSupabaseClient } from './supabaseClient';

export interface HardwareInfo {
  hardware_id: string;
  hostname: string;
  os_info: string;
}

export interface Subscription {
  id: string;
  user_id: string;
  plan_id: 'basic' | 'pro' | 'enterprise';
  status: 'active' | 'expired' | 'canceled';
  max_devices: number;
  expires_at: string;
  created_at: string;
}

export interface UserDevice {
  id: string;
  user_id: string;
  hardware_id: string;
  device_name: string;
  os_info: string;
  status: 'active' | 'blocked';
  last_active: string;
  created_at: string;
}

export interface DeviceVerificationResult {
  allowed: boolean;
  reason: 'device_registered' | 'existing_device' | 'subscription_expired' | 'device_limit_exceeded' | 'device_blocked' | 'unauthenticated' | 'error';
  device_id?: string;
  plan_id?: string;
  expires_at?: string;
  max_devices?: number;
  current_devices?: number;
}

export interface PaymentTransaction {
  id: string;
  order_code: number;
  user_id: string;
  amount: number;
  provider: 'payos' | 'sepay' | 'vietqr';
  plan_id: string;
  duration_days: number;
  status: 'pending' | 'completed' | 'failed' | 'canceled';
  payment_url?: string;
  qr_code_url?: string;
  created_at: string;
}

// 1. Get Hardware ID from Rust Tauri Command
export async function getHardwareInfo(): Promise<HardwareInfo> {
  try {
    if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
      const { invoke } = await import('@tauri-apps/api/core');
      const res = await invoke<HardwareInfo>('get_hardware_id');
      if (res && typeof res === 'object' && 'hardware_id' in res) {
        return res;
      }
    }
  } catch (err) {
    console.warn('Tauri invoke failed, using dev fallback:', err);
  }
  return {
    hardware_id: 'HWID-LOCAL-DEV-0000-0001',
    hostname: 'DevMachine',
    os_info: 'Linux (Web Dev)'
  };
}





// 2. Auth Helpers
export async function getCurrentUser() {
  const supabase = getSupabaseClient();
  const { data: { user } } = await supabase.auth.getUser();
  return user;
}

export async function loginWithEmail(email: string, password: string) {
  const supabase = getSupabaseClient();
  const { data, error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw error;
  return data.user;
}

export async function registerWithEmail(email: string, password: string) {
  const supabase = getSupabaseClient();
  const { data, error } = await supabase.auth.signUp({ email, password });
  if (error) throw error;
  return data.user;
}

export async function logout() {
  const supabase = getSupabaseClient();
  const { error } = await supabase.auth.signOut();
  if (error) throw error;
}

// 3. Verify Hardware & Subscription
export async function verifyDeviceAndSubscription(): Promise<DeviceVerificationResult> {
  try {
    const user = await getCurrentUser();
    if (!user) {
      return { allowed: false, reason: 'unauthenticated' };
    }

    const hwInfo = await getHardwareInfo();
    const supabase = getSupabaseClient();

    // Call stored procedure RPC check_or_register_device
    const { data, error } = await supabase.rpc('check_or_register_device', {
      p_hardware_id: hwInfo.hardware_id,
      p_device_name: hwInfo.hostname,
      p_os_info: hwInfo.os_info
    });

    if (error) {
      console.error('RPC check_or_register_device error:', error);
      // Fallback: check tables directly if RPC not yet created
      return await verifyDeviceFallback(user.id, hwInfo);
    }

    return data as DeviceVerificationResult;
  } catch (err) {
    console.error('Failed to verify device:', err);
    return { allowed: false, reason: 'error' };
  }
}

// Fallback verification if RPC is not present
async function verifyDeviceFallback(userId: string, hwInfo: HardwareInfo): Promise<DeviceVerificationResult> {
  const supabase = getSupabaseClient();

  // Get Subscription
  const { data: sub } = await supabase
    .from('subscriptions')
    .select('*')
    .eq('user_id', userId)
    .single();

  const maxDevices = sub?.max_devices ?? 1;
  const expiresAt = sub?.expires_at ?? new Date(Date.now() + 7 * 86400000).toISOString();
  const isExpired = new Date(expiresAt).getTime() < Date.now();

  if (isExpired) {
    return { allowed: false, reason: 'subscription_expired', expires_at: expiresAt, plan_id: sub?.plan_id };
  }

  // Check existing device
  const { data: devices } = await supabase
    .from('user_devices')
    .select('*')
    .eq('user_id', userId);

  const existingDevice = devices?.find(d => d.hardware_id === hwInfo.hardware_id);
  if (existingDevice) {
    if (existingDevice.status === 'blocked') {
      return { allowed: false, reason: 'device_blocked' };
    }
    return {
      allowed: true,
      reason: 'existing_device',
      device_id: existingDevice.id,
      plan_id: sub?.plan_id,
      expires_at: expiresAt,
      max_devices: maxDevices
    };
  }

  // Check device limit
  const activeCount = devices?.filter(d => d.status === 'active').length ?? 0;
  if (activeCount >= maxDevices) {
    return {
      allowed: false,
      reason: 'device_limit_exceeded',
      current_devices: activeCount,
      max_devices: maxDevices,
      plan_id: sub?.plan_id
    };
  }

  // Register device
  const { data: newDev } = await supabase
    .from('user_devices')
    .insert({
      user_id: userId,
      hardware_id: hwInfo.hardware_id,
      device_name: hwInfo.hostname,
      os_info: hwInfo.os_info,
      status: 'active'
    })
    .select()
    .single();

  return {
    allowed: true,
    reason: 'device_registered',
    device_id: newDev?.id,
    plan_id: sub?.plan_id,
    expires_at: expiresAt,
    max_devices: maxDevices
  };
}

// 4. Manage User Devices
export async function getUserDevices(): Promise<UserDevice[]> {
  const user = await getCurrentUser();
  if (!user) return [];

  const supabase = getSupabaseClient();
  const { data, error } = await supabase
    .from('user_devices')
    .select('*')
    .eq('user_id', user.id)
    .order('created_at', { ascending: false });

  if (error) {
    console.error('Failed to get user devices:', error);
    return [];
  }
  return data as UserDevice[];
}

export async function revokeDevice(deviceId: string): Promise<boolean> {
  const supabase = getSupabaseClient();
  const { error } = await supabase
    .from('user_devices')
    .delete()
    .eq('id', deviceId);

  if (error) {
    console.error('Failed to revoke device:', error);
    return false;
  }
  return true;
}

export interface SubscriptionPlan {
  id: string;
  name: string;
  price: number;
  duration_days: number;
  max_devices: number;
  description: string;
  badge_text?: string;
  is_active: boolean;
  sort_order: number;
}

// 5. Payment & Subscription Generation (PayOS / VietQR)
export async function getSubscriptionPlans(): Promise<SubscriptionPlan[]> {
  try {
    const supabase = getSupabaseClient();
    const { data, error } = await supabase
      .from('subscription_plans')
      .select('*')
      .eq('is_active', true)
      .order('sort_order', { ascending: true });

    if (!error && data && data.length > 0) {
      return data as SubscriptionPlan[];
    }
  } catch (e) {
    console.warn('Failed to fetch subscription plans from Supabase, using fallback:', e);
  }

  // Fallback plans if database table is not ready or offline
  return [
    {
      id: 'pro',
      name: 'PRO Plan',
      price: 199000,
      duration_days: 30,
      max_devices: 3,
      description: 'Includes: 3 Devices + Full Scraper & Zalo/Email Outreach features.',
      badge_text: 'PRO Plan',
      is_active: true,
      sort_order: 1
    },
    {
      id: 'enterprise',
      name: 'ENTERPRISE Plan',
      price: 499000,
      duration_days: 30,
      max_devices: 10,
      description: 'Includes: 10 Devices + Priority Scraper speed & 24/7 Support.',
      badge_text: 'ENTERPRISE Plan',
      is_active: true,
      sort_order: 2
    }
  ];
}

export async function createPaymentOrder(planId: string, customAmount?: number, durationDays: number = 30) {
  const user = await getCurrentUser();
  if (!user) throw new Error('Unauthenticated');

  let amount = customAmount;
  if (!amount) {
    const plans = await getSubscriptionPlans();
    const plan = plans.find(p => p.id === planId);
    amount = plan ? plan.price : (planId === 'enterprise' ? 499000 : 199000);
    if (plan && plan.duration_days) {
      durationDays = plan.duration_days;
    }
  }

  const orderCode = Number(Date.now().toString().slice(-9)); // 9-digit order code

  // VietQR QuickLink generation format (SePay compatible)
  const bankId = import.meta.env.VITE_VIETQR_BANK_ID || 'ICB';
  const accountNo = import.meta.env.VITE_VIETQR_ACCOUNT_NO || '101874355801';
  const accountName = import.meta.env.VITE_VIETQR_ACCOUNT_NAME || 'TRAN TRONG TRI';
  const addInfo = `RS SEVQR TKPRS ${orderCode}`;
  const qrCodeUrl = `https://img.vietqr.io/image/${bankId}-${accountNo}-compact2.png?amount=${amount}&addInfo=${encodeURIComponent(addInfo)}&accountName=${encodeURIComponent(accountName)}`;

  const supabase = getSupabaseClient();
  const { data, error } = await supabase
    .from('payment_transactions')
    .insert({
      order_code: orderCode,
      user_id: user.id,
      amount,
      provider: 'sepay',
      plan_id: planId,
      duration_days: durationDays,
      status: 'pending',
      qr_code_url: qrCodeUrl
    })
    .select()
    .single();

  if (error) {
    console.error('Create payment transaction error:', error);
    throw new Error(error.message || 'Unable to create payment transaction.');
  }
  return data as PaymentTransaction;
}

export async function checkPaymentStatus(orderCode: number): Promise<PaymentTransaction | null> {
  const supabase = getSupabaseClient();
  const { data, error } = await supabase
    .from('payment_transactions')
    .select('*')
    .eq('order_code', orderCode)
    .single();

  if (error || !data) return null;
  return data as PaymentTransaction;
}
