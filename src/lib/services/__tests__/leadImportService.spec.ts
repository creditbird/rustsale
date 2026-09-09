import { describe, it, expect } from 'bun:test';
import {
  autoDetectColumnMapping,
  buildErrorRowsCsv,
  isTauriEnvironment,
  type ImportErrorRow,
} from '../leadImportService';

describe('leadImportService', () => {
  it('detects Vietnamese column headers correctly', () => {
    const headers = [
      'Họ và tên (*)',
      'Số điện thoại',
      'Thư điện tử (Email)',
      'Tên công ty',
      'Giá trị hợp đồng',
      'Địa chỉ liên hệ',
      'Ghi chú bổ sung',
      'Trạng thái lead',
      'Người phụ trách'
    ];

    const mapping = autoDetectColumnMapping(headers);
    expect(mapping.name_col).toBe(0);
    expect(mapping.phone_col).toBe(1);
    expect(mapping.email_col).toBe(2);
    expect(mapping.company_col).toBe(3);
    expect(mapping.value_col).toBe(4);
    expect(mapping.address_col).toBe(5);
    expect(mapping.about_col).toBe(6);
    expect(mapping.status_col).toBe(7);
    expect(mapping.owner_col).toBe(8);
  });

  it('detects English column headers correctly', () => {
    const headers = [
      'Customer Full Name',
      'Company Name',
      'Mobile Phone',
      'Email Address',
      'Deal Value',
      'Status'
    ];

    const mapping = autoDetectColumnMapping(headers);
    expect(mapping.name_col).toBe(0);
    expect(mapping.company_col).toBe(1);
    expect(mapping.phone_col).toBe(2);
    expect(mapping.email_col).toBe(3);
    expect(mapping.value_col).toBe(4);
    expect(mapping.status_col).toBe(5);
    expect(mapping.address_col).toBeNull();
    expect(mapping.about_col).toBeNull();
  });

  it('generates error rows CSV with UTF-8 BOM and correct columns', () => {
    const headers = ['Tên', 'SĐT', 'Email'];
    const errorRows: ImportErrorRow[] = [
      {
        row_number: 2,
        raw_data: ['', '0912345678', 'test@example.com'],
        reason: 'Họ và tên không được để trống'
      },
      {
        row_number: 5,
        raw_data: ['Nguyen B', '123'],
        reason: 'Số điện thoại không đúng định dạng di động VN'
      }
    ];

    const csv = buildErrorRowsCsv(headers, errorRows);
    expect(csv.startsWith('\uFEFF')).toBe(true);
    expect(csv).toContain('"Tên","SĐT","Email","Lý do từ chối (Rejection Reason)"');
    expect(csv).toContain('Họ và tên không được để trống');
    expect(csv).toContain('Số điện thoại không đúng định dạng di động VN');
  });

  it('checks runtime environment detection', () => {
    expect(typeof isTauriEnvironment()).toBe('boolean');
  });

  it('neutralizes spreadsheet formulas in error rows to prevent CSV injection (R7)', () => {
    const headers = ['Tên', 'Ghi chú', 'SĐT'];
    const errorRows: ImportErrorRow[] = [
      {
        row_number: 2,
        raw_data: ['=1+1', '  @SUM(A1:A10)', '+84912345678'],
        reason: '-cmd| /C calc!A0'
      },
      {
        row_number: 3,
        raw_data: ['\t=evil()', 'Bình thường', '0912345678'],
        reason: 'Lỗi'
      }
    ];

    const csv = buildErrorRowsCsv(headers, errorRows);
    expect(csv.startsWith('\uFEFF')).toBe(true);
    // Formulas neutralized with single quote prefix
    expect(csv).toContain("\"'=1+1\"");
    expect(csv).toContain("\"'  @SUM(A1:A10)\"");
    expect(csv).toContain("\"'+84912345678\"");
    expect(csv).toContain("\"'-cmd| /C calc!A0\"");
    expect(csv).toContain("\"'\t=evil()\"");
    // Vietnamese phone number with leading 0 preserved
    expect(csv).toContain("\"'0912345678\"");
    // Normal text without formulas
    expect(csv).toContain('"Bình thường"');
  });
});
