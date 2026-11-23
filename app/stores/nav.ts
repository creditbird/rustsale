import { signal } from "nsh";

export type RouteId =
  | "dashboard" | "scraper" | "leads" | "categories" | "conversation"
  | "campaign_center" | "template_center" | "ai_approval" | "facebook_hub"
  | "outreach" | "whatsapp_outreach" | "email_outreach" | "pipeline"
  | "contacts" | "settings";

/** The sidebar's items, in v2's order. */
export const NAV: { id: RouteId; label: string; glyph: string }[] = [
  { id: "dashboard", label: "Tổng quan", glyph: "▦" },
  { id: "scraper", label: "Thu thập Maps", glyph: "⌖" },
  { id: "leads", label: "Khách hàng", glyph: "☺" },
  { id: "categories", label: "Danh mục", glyph: "#" },
  { id: "conversation", label: "Hội thoại", glyph: "✉" },
  { id: "campaign_center", label: "Chiến dịch", glyph: "⇉" },
  { id: "template_center", label: "Thư viện Mẫu", glyph: "❏" },
  { id: "ai_approval", label: "Duyệt AI", glyph: "✓" },
  { id: "facebook_hub", label: "Facebook", glyph: "f" },
  { id: "outreach", label: "Zalo Outreach", glyph: "z" },
  { id: "whatsapp_outreach", label: "WhatsApp", glyph: "w" },
  { id: "email_outreach", label: "Email", glyph: "@" },
  { id: "pipeline", label: "Pipeline", glyph: "≡" },
  { id: "contacts", label: "Danh bạ", glyph: "☎" },
  { id: "settings", label: "Cài đặt", glyph: "⚙" },
];

/** The screen on show. */
export const route = signal<RouteId>("leads");
