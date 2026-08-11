# Hướng dẫn chi tiết: Tích hợp Webhook Tự động Thanh toán SePay với Supabase

Tài liệu này hướng dẫn bạn cách thiết lập **SePay Webhook** để khi khách hàng quét mã VietQR và chuyển khoản thành công, hệ thống Supabase sẽ **tự động xác nhận giao dịch, gia hạn thời hạn sử dụng và nâng hạn mức số lượng máy (HWID)** cho khách hàng ngay lập tức.

---

## 🛠️ PHƯƠNG PHÁP 1: Dùng Supabase Edge Function (Khuyên dùng - Đã có HMAC SHA256 Signature Verification)

### Mã nguồn Edge Function bảo mật (Dán trực tiếp vào Supabase Web Editor)

```typescript
import { createClient } from 'https://esm.sh/@supabase/supabase-js@2'
import crypto from 'node:crypto'

// Định nghĩa CORS Headers chuẩn Deno/TypeScript
const corsHeaders: Record<string, string> = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type, x-sepay-signature, x-sepay-timestamp',
  'Access-Control-Allow-Methods': 'POST, OPTIONS'
}

Deno.serve(async (req: Request) => {
  // 1. Xử lý CORS Preflight request
  if (req.method === 'OPTIONS') {
    return new Response('ok', { headers: corsHeaders })
  }

  try {
    // Đọc raw body dưới dạng text để tính toán chữ ký HMAC chính xác tuyệt đối
    const rawBody = await req.text()

    // 2. Xác thực chữ ký bảo mật HMAC-SHA256 từ SePay (nếu có cấu hình SEPAY_SECRET_KEY trong Supabase Secrets)
    const secret = Deno.env.get('SEPAY_SECRET_KEY')
    if (secret) {
      const signature = req.headers.get('x-sepay-signature') || ''
      const timestamp = req.headers.get('x-sepay-timestamp') || ''
      
      const expected = 'sha256=' + crypto.createHmac('sha256', secret)
        .update(timestamp + '.' + rawBody)
        .digest('hex')

      if (signature !== expected) {
        console.error('Invalid SePay Signature. Expected:', expected, 'Received:', signature)
        return new Response(
          JSON.stringify({ success: false, message: 'Invalid signature' }), 
          { status: 401, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
        )
      }
    }

    // Parse JSON payload từ rawBody
    const payload = JSON.parse(rawBody)
    console.log('SePay Webhook Received:', JSON.stringify(payload))

    // 3. Chỉ xử lý giao dịch tiền vào (transferType = "in")
    if (payload?.transferType && payload.transferType !== 'in') {
      console.log('Ignored outgoing transaction (transferType != in)')
      return new Response(
        JSON.stringify({ success: true, message: 'Ignored outgoing transaction' }), 
        { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      )
    }

    // 4. Trích xuất mã đơn hàng (orderCode) từ code, content hoặc description
    // Nội dung chuyển khoản thường có dạng: "RUSTSALE 442368189" hoặc "SEVN442368189" hoặc "442368189"
    const textToSearch = `${payload?.code || ''} ${payload?.content || ''} ${payload?.description || ''}`
    
    const regexExact = new RegExp('\\b\\d{6,12}\\b')
    const regexFallback = new RegExp('\\d{6,12}')
    const match = textToSearch.match(regexExact) || textToSearch.match(regexFallback)

    if (!match) {
      console.warn('Could not extract orderCode from SePay payload:', textToSearch)
      return new Response(
        JSON.stringify({ success: true, message: 'No valid orderCode found in payload' }), 
        { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
      )
    }

    const orderCode = Number(match[0])
    const amount = Number(payload?.transferAmount || 0)

    // 5. Khởi tạo Supabase Admin Client
    const url = Deno.env.get('SUPABASE_URL') || ''
    const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY') || ''
    const supabase = createClient(url, key)

    // 6. Gọi stored procedure process_payment_webhook trong Database
    const { data: resData, error } = await supabase.rpc('process_payment_webhook', {
      p_order_code: orderCode,
      p_amount: amount,
      p_status: 'PAID'
    })

    if (error) {
      console.error('Supabase RPC Error:', error)
    } else {
      console.log('Process Payment Success:', resData)
    }

    // 7. Trả về HTTP 200 OK cho SePay
    return new Response(
      JSON.stringify({ success: true, result: resData }), 
      { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    )
  } catch (err) {
    console.error('SePay Webhook Exception:', err)
    return new Response(
      JSON.stringify({ success: true, warning: String(err) }), 
      { status: 200, headers: { ...corsHeaders, 'Content-Type': 'application/json' } }
    )
  }
})
```

---

## 🔐 Cấu hình Secret Key trên Supabase Secrets

Thêm Secret Key vào Supabase Secrets (hoặc qua Supabase Dashboard -> Project Settings -> Edge Functions -> Secrets):

```bash
bunx supabase secrets set SEPAY_SECRET_KEY=your_secret_key_here
```

---

## 🔗 Điền Webhook URL & Secret Key vào SePay Dashboard

1. Đăng nhập **[SePay Dashboard](https://my.sepay.vn)** -> Vào **Cấu hình Webhooks**.
2. Nhập URL Edge Function: `https://<YOUR_SUPABASE_PROJECT_REF>.supabase.co/functions/v1/sepay-webhook` (hoặc `/payos-webhook`).
3. Điền **Secret Key** của bạn.
4. Kiểu dữ liệu (Payload pattern): **JSON**.
5. Bấm **Gọi thử (Test Webhook)** -> SePay sẽ tính chữ ký và gửi thử 1 payload mẫu, nhận phản hồi `200 OK`.
