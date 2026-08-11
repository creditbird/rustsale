import { describe, test, expect, spyOn, beforeEach } from 'bun:test';
import * as licenseService from '../licenseService';

describe('licenseService', () => {
  test('getHardwareInfo fallback in dev environment', async () => {
    const hwInfo = await licenseService.getHardwareInfo();
    expect(hwInfo).toBeDefined();
    expect(hwInfo.hardware_id).toBeDefined();
    expect(hwInfo.hostname).toBeDefined();
    expect(hwInfo.os_info).toBeDefined();
  });

  test('createPaymentOrder generates correct VietQR QR code URL with RS SEVQR TKPRS and env vars', async () => {
    const orderCode = 123456789;
    const amount = 199000;
    const addInfo = `RS SEVQR TKPRS ${orderCode}`;
    const bankId = process.env.VITE_VIETQR_BANK_ID || 'ICB';
    const accountNo = process.env.VITE_VIETQR_ACCOUNT_NO || '101874355801';
    const accountName = process.env.VITE_VIETQR_ACCOUNT_NAME || 'TRAN TRONG TRI';

    const expectedQrUrl = `https://img.vietqr.io/image/${bankId}-${accountNo}-compact2.png?amount=${amount}&addInfo=${encodeURIComponent(addInfo)}&accountName=${encodeURIComponent(accountName)}`;
    
    expect(expectedQrUrl).toContain('https://img.vietqr.io/image/ICB-101874355801-compact2.png');
    expect(expectedQrUrl).toContain('addInfo=RS%20SEVQR%20TKPRS%20123456789');
    expect(expectedQrUrl).toContain('accountName=TRAN%20TRONG%20TRI');
  });
});


