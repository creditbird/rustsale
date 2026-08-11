-- ====================================================================
-- RUSTSALE V2 - SUPABASE DATABASE SCHEMA (Idempotent & Re-runnable)
-- License Management, HWID Device Locking & VietQR/PayOS Payment Webhook
-- ====================================================================

-- Enable UUID & Crypto Extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 1. SUBSCRIPTIONS TABLE
CREATE TABLE IF NOT EXISTS public.subscriptions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    plan_id VARCHAR(50) NOT NULL DEFAULT 'basic', -- 'basic', 'pro', 'enterprise'
    status VARCHAR(20) NOT NULL DEFAULT 'active',  -- 'active', 'expired', 'canceled'
    max_devices INT NOT NULL DEFAULT 1,            -- Default 1 device limit for basic plan
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '30 days'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_user_subscription UNIQUE (user_id)
);

-- 2. USER DEVICES (HWID LOCKING) TABLE
CREATE TABLE IF NOT EXISTS public.user_devices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    hardware_id VARCHAR(128) NOT NULL,
    device_name VARCHAR(100) NOT NULL DEFAULT 'Unknown Device',
    os_info VARCHAR(100) DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'active', -- 'active', 'blocked'
    last_active TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_user_hwid UNIQUE (user_id, hardware_id)
);

-- 3. PAYMENT TRANSACTIONS (VietQR / PayOS) TABLE
CREATE TABLE IF NOT EXISTS public.payment_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_code BIGINT NOT NULL UNIQUE,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    amount NUMERIC(12, 2) NOT NULL,
    provider VARCHAR(20) NOT NULL DEFAULT 'payos', -- 'payos', 'vietqr'
    plan_id VARCHAR(50) NOT NULL DEFAULT 'pro',
    duration_days INT NOT NULL DEFAULT 30,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- 'pending', 'completed', 'failed', 'canceled'
    payment_url TEXT DEFAULT '',
    qr_code_url TEXT DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 4. SUBSCRIPTION PLANS TABLE (Dynamic Pricing & Management via Supabase Dashboard)
CREATE TABLE IF NOT EXISTS public.subscription_plans (
    id VARCHAR(50) PRIMARY KEY, -- 'pro', 'enterprise', 'agency', etc.
    name VARCHAR(100) NOT NULL,
    price NUMERIC(12, 2) NOT NULL,
    duration_days INT NOT NULL DEFAULT 30,
    max_devices INT NOT NULL DEFAULT 3,
    description TEXT DEFAULT '',
    badge_text VARCHAR(50) DEFAULT '',
    is_active BOOLEAN NOT NULL DEFAULT true,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed initial plans if table is empty
INSERT INTO public.subscription_plans (id, name, price, duration_days, max_devices, description, badge_text, sort_order)
VALUES 
    ('pro', 'Gói PRO', 199000, 30, 3, 'Gồm: 3 Máy + Toàn bộ tính năng Scraper & Outreach Zalo/Email.', 'Gói PRO', 1),
    ('enterprise', 'Gói ENTERPRISE', 499000, 30, 10, 'Gồm: 10 Máy + Ưu tiên tốc độ Scraper & Hỗ trợ 24/7.', 'Gói ENTERPRISE', 2)
ON CONFLICT (id) DO NOTHING;

-- ====================================================================
-- ROW LEVEL SECURITY (RLS) POLICIES
-- ====================================================================

ALTER TABLE public.subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.user_devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.payment_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.subscription_plans ENABLE ROW LEVEL SECURITY;

-- Subscription Plans RLS (Public read for active plans)
DROP POLICY IF EXISTS "Anyone can view active subscription plans" ON public.subscription_plans;
CREATE POLICY "Anyone can view active subscription plans"
    ON public.subscription_plans FOR SELECT
    USING (is_active = true);

-- Subscriptions RLS
DROP POLICY IF EXISTS "Users can view their own subscription" ON public.subscriptions;
CREATE POLICY "Users can view their own subscription"
    ON public.subscriptions FOR SELECT
    USING (auth.uid() = user_id);

-- User Devices RLS
DROP POLICY IF EXISTS "Users can view their own devices" ON public.user_devices;
CREATE POLICY "Users can view their own devices"
    ON public.user_devices FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can insert their own devices" ON public.user_devices;
CREATE POLICY "Users can insert their own devices"
    ON public.user_devices FOR INSERT
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can delete/revoke their own devices" ON public.user_devices;
CREATE POLICY "Users can delete/revoke their own devices"
    ON public.user_devices FOR DELETE
    USING (auth.uid() = user_id);

-- Payment Transactions RLS
DROP POLICY IF EXISTS "Users can view their own transactions" ON public.payment_transactions;
CREATE POLICY "Users can view their own transactions"
    ON public.payment_transactions FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can insert their own transactions" ON public.payment_transactions;
CREATE POLICY "Users can insert their own transactions"
    ON public.payment_transactions FOR INSERT
    WITH CHECK (auth.uid() = user_id);

-- ====================================================================
-- DATABASE FUNCTIONS & STORED PROCEDURES
-- ====================================================================

-- Function 1: Automatically create subscription entry when a new user registers
CREATE OR REPLACE FUNCTION public.handle_new_user_subscription()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO public.subscriptions (user_id, plan_id, max_devices, expires_at)
    VALUES (NEW.id, 'basic', 1, NOW() + INTERVAL '7 days') -- 7-day free trial on register
    ON CONFLICT (user_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Trigger on user creation
DROP TRIGGER IF EXISTS trigger_on_auth_user_created ON auth.users;
CREATE TRIGGER trigger_on_auth_user_created
    AFTER INSERT ON auth.users
    FOR EACH ROW EXECUTE FUNCTION public.handle_new_user_subscription();

-- Function 2: Check or Register Device with HWID limit enforcement
DROP FUNCTION IF EXISTS public.check_or_register_device;
CREATE OR REPLACE FUNCTION public.check_or_register_device(
    p_hardware_id VARCHAR(128),
    p_device_name VARCHAR(100),
    p_os_info VARCHAR(100)
)
RETURNS JSON AS $$
DECLARE
    v_user_id UUID;
    v_sub RECORD;
    v_device RECORD;
    v_device_count INT;
BEGIN
    v_user_id := auth.uid();
    IF v_user_id IS NULL THEN
        RETURN json_build_object('allowed', false, 'reason', 'unauthenticated');
    END IF;

    -- Fetch user subscription
    SELECT * INTO v_sub FROM public.subscriptions WHERE user_id = v_user_id;
    
    IF NOT FOUND THEN
        -- Create default sub if missing
        INSERT INTO public.subscriptions (user_id, plan_id, max_devices, expires_at)
        VALUES (v_user_id, 'basic', 1, NOW() + INTERVAL '7 days')
        RETURNING * INTO v_sub;
    END IF;

    -- Check if subscription expired
    IF v_sub.expires_at < NOW() OR v_sub.status != 'active' THEN
        RETURN json_build_object(
            'allowed', false,
            'reason', 'subscription_expired',
            'expires_at', v_sub.expires_at,
            'plan_id', v_sub.plan_id
        );
    END IF;

    -- Check existing device
    SELECT * INTO v_device FROM public.user_devices 
    WHERE user_id = v_user_id AND hardware_id = p_hardware_id;

    IF FOUND THEN
        IF v_device.status = 'blocked' THEN
            RETURN json_build_object('allowed', false, 'reason', 'device_blocked');
        END IF;

        -- Update last_active timestamp
        UPDATE public.user_devices 
        SET last_active = NOW(), device_name = p_device_name, os_info = p_os_info
        WHERE id = v_device.id;

        RETURN json_build_object(
            'allowed', true,
            'reason', 'existing_device',
            'device_id', v_device.id,
            'plan_id', v_sub.plan_id,
            'expires_at', v_sub.expires_at,
            'max_devices', v_sub.max_devices
        );
    END IF;

    -- Device not registered yet: check current active device count
    SELECT COUNT(*) INTO v_device_count 
    FROM public.user_devices 
    WHERE user_id = v_user_id AND status = 'active';

    IF v_device_count >= v_sub.max_devices THEN
        RETURN json_build_object(
            'allowed', false,
            'reason', 'device_limit_exceeded',
            'current_devices', v_device_count,
            'max_devices', v_sub.max_devices,
            'plan_id', v_sub.plan_id
        );
    END IF;

    -- Register new device
    INSERT INTO public.user_devices (user_id, hardware_id, device_name, os_info, status, last_active)
    VALUES (v_user_id, p_hardware_id, p_device_name, p_os_info, 'active', NOW())
    RETURNING * INTO v_device;

    RETURN json_build_object(
        'allowed', true,
        'reason', 'device_registered',
        'device_id', v_device.id,
        'plan_id', v_sub.plan_id,
        'expires_at', v_sub.expires_at,
        'max_devices', v_sub.max_devices
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Function 3: Webhook logic processing for PayOS / VietQR Payment Completion
DROP FUNCTION IF EXISTS public.process_payment_webhook;
CREATE OR REPLACE FUNCTION public.process_payment_webhook(
    p_order_code BIGINT,
    p_amount NUMERIC(12,2),
    p_status VARCHAR(20)
)
RETURNS JSON AS $$
DECLARE
    v_tx RECORD;
    v_sub RECORD;
    v_new_expires TIMESTAMPTZ;
    v_max_devices INT;
BEGIN
    SELECT * INTO v_tx FROM public.payment_transactions WHERE order_code = p_order_code;
    
    IF NOT FOUND THEN
        RETURN json_build_object('success', false, 'reason', 'transaction_not_found');
    END IF;

    IF v_tx.status = 'completed' THEN
        RETURN json_build_object('success', true, 'reason', 'already_processed');
    END IF;

    IF p_status = 'PAID' OR p_status = 'completed' THEN
        -- Update transaction status
        UPDATE public.payment_transactions 
        SET status = 'completed', updated_at = NOW() 
        WHERE id = v_tx.id;

        -- Fetch existing subscription
        SELECT * INTO v_sub FROM public.subscriptions WHERE user_id = v_tx.user_id;

        IF v_sub.expires_at > NOW() THEN
            v_new_expires := v_sub.expires_at + (v_tx.duration_days || ' days')::INTERVAL;
        ELSE
            v_new_expires := NOW() + (v_tx.duration_days || ' days')::INTERVAL;
        END IF;

        -- Dynamically lookup max_devices from subscription_plans table
        SELECT max_devices INTO v_max_devices FROM public.subscription_plans WHERE id = v_tx.plan_id;
        IF v_max_devices IS NULL THEN
            v_max_devices := CASE 
                WHEN v_tx.plan_id = 'enterprise' THEN 10
                WHEN v_tx.plan_id = 'pro' THEN 3
                ELSE 1
            END;
        END IF;

        -- Update subscription plan & expiration
        UPDATE public.subscriptions 
        SET 
            plan_id = v_tx.plan_id,
            status = 'active',
            max_devices = v_max_devices,
            expires_at = v_new_expires,
            updated_at = NOW()
        WHERE user_id = v_tx.user_id;

        RETURN json_build_object(
            'success', true, 
            'reason', 'subscription_extended',
            'order_code', p_order_code,
            'new_expires_at', v_new_expires
        );
    ELSE
        UPDATE public.payment_transactions 
        SET status = 'failed', updated_at = NOW() 
        WHERE id = v_tx.id;

        RETURN json_build_object('success', false, 'reason', 'payment_failed');
    END IF;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Function 4: Direct Webhook Endpoint receiver for PayOS PostgREST RPC
-- Matches official PayOS payload structure (code, desc, success, data, signature)
DROP FUNCTION IF EXISTS public.payos_webhook_receiver;
CREATE OR REPLACE FUNCTION public.payos_webhook_receiver(
    code TEXT DEFAULT NULL,
    "desc" TEXT DEFAULT NULL,
    success BOOLEAN DEFAULT NULL,
    data JSON DEFAULT NULL,
    signature TEXT DEFAULT NULL
)
RETURNS JSON AS $$
DECLARE
    v_order_code BIGINT;
    v_amount NUMERIC(12,2);
    v_pay_code TEXT;
    v_result JSON;
BEGIN
    -- Extract values from PayOS payload data object
    IF data IS NOT NULL THEN
        v_order_code := (data->>'orderCode')::BIGINT;
        v_amount := (data->>'amount')::NUMERIC(12,2);
        v_pay_code := COALESCE(data->>'code', code, '00');
    ELSE
        v_pay_code := COALESCE(code, '00');
    END IF;

    IF v_order_code IS NULL THEN
        RETURN json_build_object('success', false, 'reason', 'missing_order_code');
    END IF;

    -- Check if payment code is successful ("00") or success flag is true
    IF v_pay_code = '00' OR success = TRUE THEN
        v_result := public.process_payment_webhook(v_order_code, COALESCE(v_amount, 0), 'PAID');
    ELSE
        v_result := public.process_payment_webhook(v_order_code, COALESCE(v_amount, 0), 'FAILED');
    END IF;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Grant execution permission for Webhook API endpoint
GRANT EXECUTE ON FUNCTION public.payos_webhook_receiver TO anon, authenticated, service_role;

-- Function 5: Direct Webhook Endpoint receiver for SePay PostgREST RPC
-- Matches official SePay payload structure (id, gateway, transactionDate, accountNumber, code, content, transferType, transferAmount, etc.)
DROP FUNCTION IF EXISTS public.sepay_webhook_receiver;
CREATE OR REPLACE FUNCTION public.sepay_webhook_receiver(
    id BIGINT DEFAULT NULL,
    gateway TEXT DEFAULT NULL,
    "transactionDate" TEXT DEFAULT NULL,
    "accountNumber" TEXT DEFAULT NULL,
    "subAccount" TEXT DEFAULT NULL,
    code TEXT DEFAULT NULL,
    content TEXT DEFAULT NULL,
    "transferType" TEXT DEFAULT NULL,
    description TEXT DEFAULT NULL,
    "transferAmount" NUMERIC(12,2) DEFAULT NULL,
    accumulated NUMERIC(12,2) DEFAULT NULL,
    "referenceCode" TEXT DEFAULT NULL
)
RETURNS JSON AS $$
DECLARE
    v_order_code BIGINT;
    v_amount NUMERIC(12,2);
    v_search_text TEXT;
    v_match TEXT[];
    v_result JSON;
BEGIN
    -- Chỉ xử lý tiền vào (transferType = 'in')
    IF "transferType" IS NOT NULL AND "transferType" != 'in' THEN
        RETURN json_build_object('success', true, 'reason', 'ignored_outgoing');
    END IF;

    v_amount := COALESCE("transferAmount", 0);
    v_search_text := COALESCE(code, '') || ' ' || COALESCE(content, '') || ' ' || COALESCE(description, '');
    
    -- Trích xuất dãy số orderCode 6-12 chữ số
    v_match := regexp_matches(v_search_text, '\d{6,12}');
    
    IF v_match IS NULL OR array_length(v_match, 1) < 1 THEN
        RETURN json_build_object('success', false, 'reason', 'missing_order_code');
    END IF;

    v_order_code := v_match[1]::BIGINT;

    -- Gọi hàm xử lý nâng gói / gia hạn hạn dùng
    v_result := public.process_payment_webhook(v_order_code, v_amount, 'PAID');

    RETURN v_result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

GRANT EXECUTE ON FUNCTION public.sepay_webhook_receiver TO anon, authenticated, service_role;

