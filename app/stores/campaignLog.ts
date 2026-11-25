/**
 * The core writes its campaign log in English (`Message sent to …`, `Processed 1/2 leads`, …).
 * Outreach screens show the same lines in Vietnamese, following the code → sentence table the
 * Conversation screen uses (`MESSAGE_FOR_CODE` in `stores/core.ts`): a sentence for the person,
 * never the raw engine text. Unknown lines are left as they are, so an already Vietnamese line
 * from the UI passes through untouched.
 */

interface LogRule {
  pattern: RegExp;
  say: (...parts: string[]) => string;
}

function zaloStatusVi(status: string): string {
  switch (status) {
    case "BLOCKED_STRANGER":
      return "người lạ đã chặn";
    case "VERIFIED_NOT_EXIST":
    case "NOT_FOUND":
      return "không có trên Zalo";
    case "INVALID_PHONE":
      return "số điện thoại không hợp lệ";
    case "RATE_LIMITED":
      return "bị giới hạn tần suất";
    case "LOOKUP_FAILED":
      return "tra cứu thất bại";
    case "SEND_FAILED":
      return "gửi thất bại";
    default:
      return "đã đánh dấu trước đó";
  }
}

const RULES: LogRule[] = [
  // Zalo: run-level notices.
  {
    pattern: /^Automatically skipped (\d+) lead\(s\) missing phone number before starting campaign$/,
    say: (n) => `Đã tự bỏ qua ${n} lead thiếu số điện thoại trước khi chạy chiến dịch.`,
  },
  {
    pattern: /^No leads with valid phone numbers or Zalo UIDs to outreach\.$/,
    say: () => "Không có lead nào có số điện thoại hoặc UID Zalo hợp lệ để gửi.",
  },
  {
    pattern: /^No Zalo account selected for the outreach\.$/,
    say: () => "Chưa chọn tài khoản Zalo nào để gửi.",
  },
  {
    pattern: /^No Zalo account selected; nothing was sent\.$/,
    say: () => "Chưa chọn tài khoản Zalo nên chưa gửi tin nào.",
  },
  {
    pattern: /^No valid IMEI provided$/,
    say: () => "Chưa có tài khoản Zalo hợp lệ để chạy chiến dịch.",
  },
  {
    pattern: /^No leads to outreach after splitting across accounts\.$/,
    say: () => "Không còn lead nào để gửi sau khi chia việc cho các tài khoản.",
  },
  {
    pattern: /^No eligible leads found \(missing phone numbers\)\.$/,
    say: () => "Không có lead nào đủ điều kiện gửi vì thiếu số điện thoại.",
  },
  {
    pattern: /^No leads to send; nothing was sent\.$/,
    say: () => "Không có lead nào để gửi nên chưa gửi tin nào.",
  },
  {
    pattern: /^No valid sessions available for campaign$/,
    say: () => "Không có phiên Zalo nào dùng được cho chiến dịch.",
  },
  { pattern: /^No valid sessions$/, say: () => "Không có phiên Zalo nào dùng được." },
  {
    pattern: /^All sessions exhausted\. Stopping campaign\.$/,
    say: () => "Đã hết phiên Zalo dùng được nên dừng chiến dịch.",
  },
  {
    pattern: /^Starting campaign\.\.\.$/,
    say: () => "Đang bắt đầu chiến dịch…",
  },
  {
    pattern: /^No leads found in category\.$/,
    say: () => "Danh mục này không có lead nào.",
  },
  {
    pattern: /^No eligible leads to process\.$/,
    say: () => "Không có lead nào đủ điều kiện để xử lý.",
  },
  {
    pattern: /^Campaign completed\. Total sent: (\d+)$/,
    say: (n) => `Chiến dịch xong: đã gửi ${n} tin.`,
  },
  {
    pattern: /^Outreach complete! Sent (\d+) messages out of (\d+) leads\. \(Mock\)$/,
    say: (sent, total) => `Gửi xong: ${sent}/${total} lead đã nhận tin (bản mô phỏng).`,
  },
  {
    pattern: /^Outreach complete! Sent (\d+) messages out of (\d+) leads\.$/,
    say: (sent, total) => `Gửi xong: ${sent}/${total} lead đã nhận tin.`,
  },
  {
    pattern: /^Outreach paused due to Zalo rate limit\. Sent (\d+) messages out of (\d+) leads\.$/,
    say: (sent, total) => `Tạm dừng do giới hạn tần suất Zalo: đã gửi ${sent}/${total} lead.`,
  },
  {
    pattern: /^Campaign stopped due to error: [\s\S]+$/,
    say: () => "Chiến dịch dừng vì lỗi; xem lịch sử gửi để biết chi tiết.",
  },
  {
    pattern: /^Campaign failed: [\s\S]+$/,
    say: () => "Chiến dịch bị lỗi; xem lịch sử gửi để biết chi tiết.",
  },
  {
    pattern: /^Partition (\d+) failed: [\s\S]+$/,
    say: (n) => `Phần gửi ${n} bị lỗi; chiến dịch dừng phần này.`,
  },

  // Zalo: one lead.
  {
    pattern: /^Message sent to (.+) \((.+)\) \(Mock\)$/,
    say: (name, uid) => `Đã gửi tin cho ${name} (UID ${uid}) (bản mô phỏng).`,
  },
  {
    pattern: /^Message sent to (.+) \((.+)\)$/,
    say: (name, uid) => `Đã gửi tin cho ${name} (UID ${uid}).`,
  },
  {
    pattern: /^Failed to send Zalo message to (.+) \((.+)\): [\s\S]+$/,
    say: (name, uid) => `Gửi tin Zalo cho ${name} (UID ${uid}) thất bại.`,
  },
  {
    pattern: /^Failed sending to (.+) using session (.+): [\s\S]+$/,
    say: (name, session) => `Gửi cho ${name} qua tài khoản ${session} thất bại.`,
  },
  {
    pattern: /^Sent to (.+) using session (.+)$/,
    say: (name, session) => `Đã gửi cho ${name} qua tài khoản ${session}.`,
  },
  {
    pattern: /^Delivery succeeded for (.+) but local persistence failed: [\s\S]+$/,
    say: (name) => `Đã gửi cho ${name} nhưng chưa lưu được vào máy; xem lịch sử gửi.`,
  },
  {
    pattern: /^Delivery succeeded but local persistence failed: [\s\S]+$/,
    say: () => "Đã gửi tin nhưng chưa lưu được vào máy; xem lịch sử gửi.",
  },
  {
    pattern: /^Skipped Zalo outreach for (.+) \(UID: (.+)\): Previously marked as BLOCKED_STRANGER$/,
    say: (name, uid) => `Bỏ qua ${name} (UID ${uid}) vì trước đó bị đánh dấu là người lạ.`,
  },
  {
    pattern: /^Skipped Zalo lookup for (.+) \((.+)\): Previously marked as (.+)$/,
    say: (name, phone, status) => `Bỏ qua tra cứu Zalo cho ${name} (${phone}) vì trước đó đã đánh dấu ${zaloStatusVi(status)}.`,
  },
  {
    pattern: /^Using cached Zalo UID for (.+) \(UID: (.+)\)$/,
    say: (name, uid) => `Dùng UID Zalo đã lưu cho ${name} (UID ${uid}).`,
  },
  {
    pattern: /^Looking up Zalo UID for (.+) \((.+)\)$/,
    say: (name, phone) => `Đang tra UID Zalo cho ${name} (${phone})…`,
  },
  {
    pattern: /^Failed to look up phone (.+) for (.+): [\s\S]+$/,
    say: (phone, name) => `Không tra được số ${phone} của ${name} trên Zalo.`,
  },
  {
    pattern: /^Invalid Zalo UID returned for (.+) \(phone: (.+)\): [\s\S]+$/,
    say: (name, phone) => `Zalo trả về UID không hợp lệ cho ${name} (${phone}).`,
  },
  {
    pattern: /^Phone number (.+) for (.+) not found on Zalo$/,
    say: (phone, name) => `Không tìm thấy số ${phone} của ${name} trên Zalo.`,
  },
  {
    pattern: /^Phone number (.+) not found on Zalo \(Mock\)$/,
    say: (phone) => `Không tìm thấy số ${phone} trên Zalo (bản mô phỏng).`,
  },
  {
    pattern: /^Lead (.+) \((\d+)\) is missing phone number$/,
    say: (name, id) => `Lead ${name} (ID ${id}) thiếu số điện thoại.`,
  },
  {
    pattern: /^Lead (\d+) is missing phone number$/,
    say: (id) => `Lead ${id} thiếu số điện thoại.`,
  },
  {
    pattern: /^Skipping non-mobile number: (.+)$/,
    say: (phone) => `Bỏ qua số không phải di động: ${phone}.`,
  },
  {
    pattern: /^Failed to load lead (\d+): [\s\S]+$/,
    say: (id) => `Không đọc được lead ${id} nên bỏ qua.`,
  },
  {
    pattern: /^Lead (.+) not found on Zalo\.$/,
    say: (name) => `Không tìm thấy lead ${name} trên Zalo.`,
  },
  {
    pattern: /^\[Burst Limit\] Sent (\d+) messages\. Pausing (\d+) seconds to protect Zalo account\.\.\.$/,
    say: (n, seconds) => `Đã gửi ${n} tin; tạm nghỉ ${seconds} giây để bảo vệ tài khoản Zalo…`,
  },

  // Zalo: session and template trouble.
  {
    pattern: /^Session not found for IMEI: (.+)$/,
    say: (imei) => `Không tìm thấy phiên đăng nhập Zalo cho tài khoản ${imei}.`,
  },
  {
    pattern: /^Session not found(?: [\s\S]*)?$/,
    say: () => "Không tìm thấy phiên đăng nhập Zalo.",
  },
  {
    pattern: /^Database error loading session: [\s\S]+$/,
    say: () => "Lỗi đọc phiên Zalo đã lưu; thử lại sau.",
  },
  {
    pattern: /^Failed to parse session cookies: [\s\S]+$/,
    say: () => "Phiên Zalo đã lưu bị hỏng; hãy đăng nhập lại tài khoản.",
  },
  {
    pattern: /^Failed to load outreach template: [\s\S]+$/,
    say: () => "Không đọc được mẫu tin; thử lại sau.",
  },
  {
    pattern: /^Zalo login failed: [\s\S]+$/,
    say: () => "Đăng nhập Zalo thất bại; hãy kết nối lại tài khoản trong Cài đặt.",
  },
  {
    pattern: /^\[Zalo Rate Limit Warning\] Zalo (?:lookup\/message|lookup|message) limit reached[\s\S]*$/,
    say: () => "Zalo tạm giới hạn tần suất; chiến dịch tạm dừng để bảo vệ tài khoản.",
  },

  // Shared counters.
  {
    pattern: /^Processed lead (\d+)\/(\d+) \(Mock\)$/,
    say: (n, total) => `Đã xử lý ${n}/${total} lead (bản mô phỏng).`,
  },
  {
    pattern: /^Processed lead (\d+)\/(\d+)$/,
    say: (n, total) => `Đã xử lý ${n}/${total} lead.`,
  },
  {
    pattern: /^Processed (\d+)\/(\d+) leads$/,
    say: (n, total) => `Đã xử lý ${n}/${total} lead.`,
  },

  // Email outreach.
  {
    pattern: /^Starting email outreach\.\.\.$/,
    say: () => "Đang bắt đầu gửi email…",
  },
  {
    pattern: /^No leads selected; no emails were sent\.$/,
    say: () => "Chưa chọn lead nào nên chưa gửi email nào.",
  },
  {
    pattern: /^No leads selected\.$/,
    say: () => "Chưa chọn lead nào.",
  },
  {
    pattern: /^Email outreach completed\. (\d+) emails sent\.$/,
    say: (n) => `Gửi email xong: ${n} email đã gửi.`,
  },
  {
    pattern: /^Successfully sent email to (.+) \((.+)\)$/,
    say: (name, email) => `Đã gửi email cho ${name} (${email}).`,
  },
  {
    pattern: /^Failed sending email to (.+) \((.+)\): [\s\S]+$/,
    say: (name, email) => `Gửi email cho ${name} (${email}) thất bại.`,
  },
  {
    pattern: /^Skipping lead '(.+)' \(ID (\d+)\): No valid email address$/,
    say: (name, id) => `Bỏ qua ${name} (ID ${id}) vì chưa có email hợp lệ.`,
  },
  {
    pattern: /^Lead ID (\d+) not found: [\s\S]+$/,
    say: (id) => `Không tìm thấy lead ${id} nên bỏ qua.`,
  },
  {
    pattern: /^Invalid recipient email address for (.+): [\s\S]+$/,
    say: (name) => `Email của ${name} không hợp lệ.`,
  },
  {
    pattern: /^Invalid from header: [\s\S]+$/,
    say: () => "Địa chỉ người gửi không hợp lệ; kiểm tra tài khoản SMTP.",
  },
  {
    pattern: /^SMTP (?:TLS|STARTTLS) relay error: [\s\S]+$/,
    say: () => "Không kết nối TLS được tới máy chủ SMTP; kiểm tra tài khoản gửi.",
  },
  {
    pattern: /^SMTP server with ID (\d+) not found$/,
    say: (id) => `Không tìm thấy cấu hình SMTP #${id}.`,
  },
  {
    pattern: /^Email template with ID (\d+) not found$/,
    say: (id) => `Không tìm thấy mẫu email #${id}.`,
  },
  {
    pattern: /^Failed to build email message: [\s\S]+$/,
    say: () => "Không dựng được email; kiểm tra mẫu và tài khoản gửi.",
  },

  // Generic core/network/DB errors surfaced in run logs.
  {
    pattern: /^(?:Connection error:\s*)?Connection refused[\s\S]*$/i,
    say: () => "Không kết nối được tới máy chủ; kiểm tra cấu hình và mạng.",
  },
  {
    pattern: /^(?:Connection error:\s*)?(?:Connection timed out|timed out|operation timed out)[\s\S]*$/i,
    say: () => "Kết nối tới máy chủ quá thời gian chờ; thử lại sau.",
  },
  {
    pattern: /^(?:Connection error:\s*)?(?:dns error|failed to lookup address|no such host)[\s\S]*$/i,
    say: () => "Không phân giải được tên miền máy chủ; kiểm tra cấu hình mạng.",
  },
  {
    pattern: /^Database (?:lock |transaction |query |write )?error:?[\s\S]*$/i,
    say: () => "Lỗi cơ sở dữ liệu khi xử lý chiến dịch; thử lại sau.",
  },
];

/** The Vietnamese sentence for a core campaign log line, or the line itself when it is not one. */
export function vietnameseCampaignLog(message: string): string {
  const text = message.trim();
  if (text === "") return "";
  for (const rule of RULES) {
    const found = rule.pattern.exec(text);
    if (found !== null) return rule.say(...found.slice(1));
  }
  return message;
}
