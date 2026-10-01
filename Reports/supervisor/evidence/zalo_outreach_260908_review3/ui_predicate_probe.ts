  function sanitizeZaloUid(uid: string | null | undefined): string | null {
    if (!uid) return null;
    const trimmed = uid.replace(/[\t\r\n]/g, '').trim();
    const lower = trimmed.toLowerCase();
    if (!trimmed || lower === '0' || lower === '-0' || lower === '-1' || lower === 'null' || lower === 'undefined') {
      return null;
    }
    if (/^0+$/.test(trimmed)) {
      return null;
    }
    return trimmed;
  }
  function normalizePhoneDigits(phone: string): string {
    let phonePart = phone;
    const match = phone.search(/[^0-9\s+\-().,\[\]]/);
    if (match !== -1) {
      phonePart = phone.slice(0, match);
    }
    let digits = phonePart.replace(/\D/g, '');
    if (digits.startsWith('0084')) {
      return '0' + digits.slice(4);
    } else if (digits.startsWith('84')) {
      return '0' + digits.slice(2);
    }
    return digits;
  }
  function isVnMobile(phone: string | null | undefined): boolean {
    if (!phone) return false;
    const normalized = normalizePhoneDigits(phone);
    if (normalized.length !== 10) return false;
    const prefix = normalized.slice(0, 2);
    return ['03', '05', '07', '08', '09'].includes(prefix);
  }
const cases = [["1", null], ["+84-90-123", null], ["+84 901234567", null], ["\t0901234567", null], ["090[123]4567", null], ["090,1234567", null], ["", " -1 "], ["", "cached-uid"], ["", "0\t0"], ["", "\u00a00\u00a0"]];
for(const [phone,uid] of cases) console.log(JSON.stringify({phone,uid,ui:!!sanitizeZaloUid(uid)||isVnMobile(phone)}));
