/** Pure parts of the Chiến dịch (campaign_center) tab, kept out of the screen so a test can check them. */

export interface UnifiedCampaign {
  id: number;
  name: string;
  description: string | null;
  channel_type: string;
  status: string;
  audience_filter_json: string | null;
  schedule_config_json: string | null;
  created_at: string;
  updated_at: string;
}

export interface StepDraft {
  step_order: number;
  channel: string;
  delay_minutes: number;
  condition_type: string;
  template_content: string;
  ai_prompt_override: string | null;
  template_id: number | null;
}

export interface StepAnalytics {
  step_id: number;
  step_order: number;
  channel: string;
  pending: number;
  sent: number;
  delivered: number;
  replied: number;
  failed: number;
  skipped: number;
}

export interface CampaignAnalytics {
  campaign_id: number;
  campaign_name: string;
  campaign_status: string;
  total_enrolled: number;
  total_steps: number;
  total_executions: number;
  pending_count: number;
  sent_count: number;
  delivered_count: number;
  replied_count: number;
  failed_count: number;
  skipped_count: number;
  converted_count: number;
  response_rate: number;
  conversion_rate: number;
  step_breakdown: StepAnalytics[];
}

export const CHANNELS = [
  { id: "zalo", label: "Zalo" },
  { id: "whatsapp", label: "WhatsApp" },
  { id: "email", label: "Email" },
  { id: "facebook", label: "Facebook" },
];

export const CONDITIONS = [
  { id: "ALWAYS", label: "Luôn gửi" },
  { id: "IF_NOT_REPLIED", label: "Nếu chưa phản hồi" },
  { id: "IF_DEAL_STAGE_IS", label: "Theo giai đoạn deal" },
  { id: "IF_INTENT_IS", label: "Theo ý định AI" },
];

export const DELAYS = [
  { label: "Ngay", minutes: 0 },
  { label: "15 phút", minutes: 15 },
  { label: "1 giờ", minutes: 60 },
  { label: "4 giờ", minutes: 240 },
  { label: "1 ngày", minutes: 1440 },
  { label: "2 ngày", minutes: 2880 },
  { label: "3 ngày", minutes: 4320 },
  { label: "5 ngày", minutes: 7200 },
];

export const PLACEHOLDERS = ["{{name}}", "{{phone}}", "{{company}}", "{{email}}", "{{value}}"];

export function statusLabel(status: string): string {
  if (status === "ACTIVE") return "Đang chạy";
  if (status === "PAUSED") return "Tạm dừng";
  return "Bản nháp";
}

/** The step a new campaign starts with, or the one added after `previous`. */
export function newStep(order: number, previous?: StepDraft): StepDraft {
  return {
    step_order: order,
    channel: previous?.channel ?? "zalo",
    delay_minutes: order === 1 ? 0 : 1440,
    condition_type: order === 1 ? "ALWAYS" : "IF_NOT_REPLIED",
    template_content: "",
    ai_prompt_override: null,
    template_id: null,
  };
}

export function renumber(steps: StepDraft[]): StepDraft[] {
  return steps.map((s, i) => ({ ...s, step_order: i + 1 }));
}

export function moveStep(steps: StepDraft[], index: number, by: -1 | 1): StepDraft[] {
  const to = index + by;
  if (index < 0 || to < 0 || to >= steps.length) return steps;
  const next = [...steps];
  [next[index], next[to]] = [next[to], next[index]];
  return renumber(next);
}

export function delayLabel(minutes: number): string {
  const preset = DELAYS.find((d) => d.minutes === minutes);
  if (preset) return preset.label;
  if (minutes % 1440 === 0) return `${minutes / 1440} ngày`;
  if (minutes % 60 === 0) return `${minutes / 60} giờ`;
  return `${minutes} phút`;
}

/** The sentence to show when the draft cannot be saved, or null when it can. */
export function validateCampaign(name: string, steps: StepDraft[]): string | null {
  if (name.trim() === "") return "Cần nhập tên chiến dịch.";
  if (steps.length === 0) return "Cần ít nhất 1 bước trong kịch bản.";
  const empty = steps.findIndex((s) => s.template_content.trim() === "");
  if (empty >= 0) return `Bước ${empty + 1} chưa có nội dung.`;
  return null;
}

/** A step as `Campaigns.unifiedCampaignSteps` returns it (the shape `saveUnifiedCampaign` takes back). */
export interface StepRecord {
  id: number | null;
  campaign_id: number;
  step_order: number;
  channel: string;
  delay_minutes: number;
  condition_type: string;
  template_content: string;
  ai_prompt_override: string | null;
  created_at: string;
  template_id: number | null;
}

/** The editor's drafts from the stored steps: ordered by step_order then id, renumbered 1..n. An empty list stays empty. */
export function stepsToDrafts(rows: StepRecord[]): StepDraft[] {
  const sorted = [...rows].sort((a, b) => a.step_order - b.step_order || (a.id ?? 0) - (b.id ?? 0));
  return renumber(
    sorted.map((r) => ({
      step_order: r.step_order,
      channel: r.channel,
      delay_minutes: r.delay_minutes,
      condition_type: r.condition_type,
      template_content: r.template_content,
      ai_prompt_override: r.ai_prompt_override ?? null,
      template_id: r.template_id ?? null,
    })),
  );
}

/**
 * What `Campaigns.saveUnifiedCampaign` takes. Without `existing` it is a new DRAFT campaign (the core assigns the ids);
 * with it the campaign keeps its id, status, channel type, audience and schedule, and only name, description and steps change.
 */
export function buildPayload(name: string, description: string, steps: StepDraft[], existing?: UnifiedCampaign) {
  const stamp = new Date().toISOString();
  return {
    campaign: {
      id: existing?.id ?? null,
      name: name.trim(),
      description: description.trim() === "" ? null : description.trim(),
      channel_type: existing?.channel_type ?? "omnichannel",
      status: existing?.status ?? "DRAFT",
      audience_filter_json: existing?.audience_filter_json ?? null,
      schedule_config_json: existing?.schedule_config_json ?? null,
      created_at: existing?.created_at ?? stamp,
      updated_at: stamp,
    },
    steps: steps.map((s) => ({
      id: null,
      campaign_id: existing?.id ?? 0,
      step_order: s.step_order,
      channel: s.channel,
      delay_minutes: s.delay_minutes,
      condition_type: s.condition_type,
      template_content: s.template_content.trim(),
      ai_prompt_override: s.ai_prompt_override,
      created_at: stamp,
      template_id: s.template_id,
    })),
  };
}

export function percent(rate: number): string {
  return `${(Number.isFinite(rate) ? rate : 0).toFixed(1)}%`;
}

// ---- templates (UI-35): the picker and the "save as template" form

export interface TemplateRecord {
  id: number;
  folder_id: number | null;
  name: string;
  channel: string;
  stage: string | null;
  subject: string | null;
  content: string;
  media_url: string | null;
  ai_prompt: string | null;
  tags_json: string | null;
}

export interface TemplateFolder { id: number; name: string; color?: string | null }

export const TEMPLATE_CHANNELS = [
  { id: "all", label: "Tất cả" },
  ...CHANNELS,
  { id: "omnichannel", label: "Đa kênh" },
];

export const TEMPLATE_STAGES = [
  { id: "cold", label: "Tiếp cận đầu" },
  { id: "followup", label: "Chăm sóc lại" },
  { id: "nurture", label: "Nuôi dưỡng" },
  { id: "closing", label: "Chốt đơn" },
];

export function parseTags(json: string | null | undefined): string[] {
  if (!json) return [];
  try {
    const v: unknown = JSON.parse(json);
    if (Array.isArray(v)) return v.map((t) => String(t).trim()).filter((t) => t !== "");
  } catch {
    return json.split(",").map((t) => t.trim()).filter((t) => t !== "");
  }
  return [];
}

/** `"a, b"` → `'["a","b"]'`, or null when there is no tag. */
export function stringifyTags(input: string): string | null {
  const list = input.split(",").map((t) => t.trim()).filter((t) => t !== "");
  return list.length === 0 ? null : JSON.stringify(list);
}

/** Picker list: a channel keeps its own templates and the omnichannel ones; the search looks at name, content, subject and tags. */
export function filterTemplates(
  list: TemplateRecord[],
  channel: string,
  folderId: number | null,
  query: string,
): TemplateRecord[] {
  const q = query.trim().toLowerCase();
  return list.filter((t) => {
    if (channel !== "all" && t.channel !== channel && t.channel !== "omnichannel") return false;
    if (folderId !== null && t.folder_id !== folderId) return false;
    if (q === "") return true;
    return [t.name, t.content, t.subject ?? "", t.tags_json ?? ""].some((x) => x.toLowerCase().includes(q));
  });
}

const SAMPLE: Record<string, string> = {
  name: "Nguyễn Văn A",
  company: "Công ty Á Châu",
  phone: "0987654321",
  email: "nguyenvana@gmail.com",
  value: "15,000,000 VND",
};

/** Fills the `{{name}}`-style variables with sample data and picks one `{A|B}` option; `pick` is injectable for tests. */
export function previewSpintax(text: string, pick: (n: number) => number = (n) => Math.floor(Math.random() * n)): string {
  let out = text;
  for (const [key, val] of Object.entries(SAMPLE)) out = out.split(`{{${key}}}`).join(val);
  for (let i = 0; i < 10 && /\{[^{}]+\}/.test(out); i += 1) {
    out = out.replace(/\{([^{}]+)\}/g, (_m, choices: string) => {
      const options = choices.split("|");
      return options[pick(options.length)] ?? options[0];
    });
  }
  return out;
}

export function validateTemplateDraft(name: string, content: string): string | null {
  if (name.trim() === "") return "Cần nhập tên mẫu.";
  if (content.trim() === "") return "Cần nhập nội dung mẫu.";
  return null;
}
