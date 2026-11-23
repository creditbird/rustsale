import { computed, signal } from "nsh";
import { Campaigns, ConversationNetwork, LeadsMore } from "rustsale/native";
import { startCore } from "./core";
import { useOutreachLeads, type OutreachPicker } from "./outreach";
import { notify, report } from "./toast";

/** A connected WhatsApp account as `ConversationNetwork.whatsappAccounts` returns it. */
export interface WhatsAppAccount {
  id: number;
  provider_account_key: string;
  display_name: string | null;
  phone_number: string | null;
}

/** A campaign as the core stores it (snake_case, counters included). */
export interface WhatsAppCampaign {
  id: number;
  name: string;
  template_id: number | null;
  status: string;
  total_recipients: number;
  sent_count: number;
  delivered_count: number;
  read_count: number;
  failed_count: number;
  skipped_count: number;
  assigned_account_ids: number[];
  rate_limit_min_delay_secs: number;
  rate_limit_max_delay_secs: number;
  burst_limit_per_batch: number;
  burst_pause_secs: number;
  quiet_hours_enabled: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
  timezone: string;
  last_heartbeat_at: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}

/** One recipient of a campaign, with the rendered text and the last error. */
export interface CampaignRecipient {
  id: number;
  campaign_id: number;
  lead_id: number | null;
  recipient_phone_raw: string;
  recipient_phone_e164: string;
  status: string;
  skip_reason: string | null;
  rendered_text: string | null;
  attempt_count: number;
  last_error: string | null;
}

/** A lead row as `LeadsMore.list` returns it, for the template variables. */
interface LeadRow {
  id: number;
  name?: string | null;
  company?: string | null;
  email?: string | null;
  phone?: string | null;
  about?: string | null;
}

/** The recipient shape `Campaigns.populateWhatsappCampaignRecipients` takes. */
interface RecipientInput {
  lead_id: number | null;
  recipient_phone_raw: string;
  template: string;
  lead_vars: Record<string, string>;
}

export interface ParsedPhone {
  raw: string;
  e164: string;
}

export const CAMPAIGN_STATUSES: { id: string; label: string }[] = [
  { id: "all", label: "Tất cả" },
  { id: "running", label: "Đang chạy" },
  { id: "queued", label: "Đang chờ" },
  { id: "paused", label: "Tạm dừng" },
  { id: "completed", label: "Hoàn tất" },
  { id: "draft", label: "Nháp" },
  { id: "failed", label: "Lỗi" },
  { id: "cancelled", label: "Đã huỷ" },
];

const PAGE = 50;
const SAMPLE_LEAD: Record<string, string> = {
  name: "Nguyễn Văn A",
  company: "TechCorp Việt Nam",
  phone: "+84901234567",
  email: "a.nguyen@example.com",
  notes: "Khách hàng VIP",
};

/** Parses single-level spintax: `{Option A|Option B|Option C}`. */
export function parseSpintax(input: string): string {
  let text = input;
  const regex = /\{([^{}]+)\}/;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(text)) !== null) {
    const options = match[1].split("|");
    const chosen = options.length > 0 ? options[Math.floor(Math.random() * options.length)] : "";
    text = text.slice(0, match.index) + chosen + text.slice(match.index + match[0].length);
  }
  return text;
}

/** Replaces `[name]`, `[phone]`… tokens and evaluates spintax, as the core does when it sends. */
export function renderSpintaxTemplate(template: string, vars: Record<string, string> = {}): string {
  let text = template;
  for (const [key, val] of Object.entries(vars)) {
    text = text.replace(new RegExp(`\\[${key}\\]`, "gi"), val);
  }
  return parseSpintax(text);
}

/** E.164 for a raw phone, with Vietnam as the default country; null when it is not a phone. */
export function normalizeWhatsAppPhone(raw: string, defaultCountry = "84"): string | null {
  const digits = raw.replace(/[^\d+]/g, "");
  if (!digits) return null;
  let cleaned = digits;
  if (cleaned.startsWith("+")) cleaned = cleaned.slice(1);
  else if (cleaned.startsWith("00")) cleaned = cleaned.slice(2);
  else if (cleaned.startsWith("0")) cleaned = defaultCountry + cleaned.slice(1);
  else if (!cleaned.startsWith(defaultCountry) && cleaned.length <= 10) cleaned = defaultCountry + cleaned;
  return cleaned.length >= 8 && cleaned.length <= 15 ? `+${cleaned}` : null;
}

/** The phone list of the manual mode: split on newlines, commas and semicolons, normalized and deduplicated. */
export function parseManualPhones(raw: string): ParsedPhone[] {
  if (raw.trim() === "") return [];
  const seen = new Set<string>();
  const out: ParsedPhone[] = [];
  for (const token of raw.split(/[\n,;]+/).map((t) => t.trim()).filter((t) => t !== "")) {
    const e164 = normalizeWhatsAppPhone(token);
    if (e164 !== null && !seen.has(e164)) {
      seen.add(e164);
      out.push({ raw: token, e164 });
    }
  }
  return out;
}

/** "dd/mm/yyyy" in the machine's time zone. */
export function dayLabel(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}/${d.getFullYear()}`;
}

/** The progress of a campaign, 0 to 100. */
export function campaignProgress(c: WhatsAppCampaign): number {
  if (c.total_recipients <= 0) return 0;
  return Math.min(100, Math.round((c.sent_count / c.total_recipients) * 100));
}

function validateNumber(value: string, min: number, max: number): number | null {
  const n = Number(value);
  return Number.isInteger(n) && n >= min && n <= max ? n : null;
}

/**
 * The WhatsApp outreach screen's state: campaigns and accounts from the core, the create form
 * (accounts, template, anti-ban pacing, quiet hours, CRM pick or manual phones) and the recipient ledger.
 */
export function useWhatsAppOutreach() {
  const accounts = signal<WhatsAppAccount[]>([]);
  const campaigns = signal<WhatsAppCampaign[]>([]);
  const ready = signal(false);
  const loading = signal(false);
  const loadFailed = signal(false);

  const statusFilter = signal("all");
  const searchQuery = signal("");

  const createOpen = signal(false);
  const creating = signal(false);
  const createError = signal<string | null>(null);
  const name = signal("");
  const chosenAccounts = signal<number[]>([]);
  const template = signal("{Xin chào|Chào bạn|Hello} [name], chúng tôi gửi ưu đãi dành riêng cho [company]!");
  const previewSeed = signal(0);
  const minDelay = signal("8");
  const maxDelay = signal("20");
  const burstLimit = signal("25");
  const burstPause = signal("60");
  const quietEnabled = signal(true);
  const quietStart = signal("22:00");
  const quietEnd = signal("07:30");
  const timezone = signal("Asia/Ho_Chi_Minh");
  const recipientMode = signal<"crm" | "manual">("crm");
  const manualPhones = signal("");

  const detailsOpen = signal(false);
  const selected = signal<WhatsAppCampaign | null>(null);
  const recipients = signal<CampaignRecipient[]>([]);
  const recipientsLoading = signal(false);
  const recipientsDone = signal(false);
  let recipientOffset = 0;

  const pick: OutreachPicker = useOutreachLeads("whatsapp");

  const filtered = computed(() => {
    const q = searchQuery().trim().toLowerCase();
    const status = statusFilter().toLowerCase();
    return campaigns().filter((c) => {
      if (status !== "all" && c.status.toLowerCase() !== status) return false;
      return q === "" || c.name.toLowerCase().includes(q);
    });
  });
  const totalCampaigns = computed(() => campaigns().length);
  const runningCampaigns = computed(
    () => campaigns().filter((c) => c.status === "running" || c.status === "queued").length,
  );
  const totalSent = computed(() => campaigns().reduce((sum, c) => sum + (c.sent_count || 0), 0));
  const totalDelivered = computed(() => campaigns().reduce((sum, c) => sum + (c.delivered_count || 0), 0));
  const totalRead = computed(() => campaigns().reduce((sum, c) => sum + (c.read_count || 0), 0));
  const totalSkipped = computed(() => campaigns().reduce((sum, c) => sum + (c.skipped_count || 0), 0));
  const deliveryReadRate = computed(() => {
    const sent = totalSent();
    return sent > 0 ? Math.min(100, Math.round(((totalDelivered() + totalRead()) / sent) * 100)) : 0;
  });
  const statusCount = (id: string): number => {
    if (id === "all") return campaigns().length;
    return campaigns().filter((c) => c.status.toLowerCase() === id).length;
  };
  const renderedPreview = computed(() => {
    void previewSeed();
    const lead = SAMPLE_LEAD;
    return renderSpintaxTemplate(template(), {
      name: lead.name,
      company: lead.company,
      phone: lead.phone,
      email: lead.email,
      notes: lead.notes,
    });
  });
  const parsedManual = computed(() => parseManualPhones(manualPhones()));
  const recipientCount = computed(() =>
    recipientMode() === "manual" ? parsedManual().length : pick.chosenCount(),
  );
  const loadCampaigns = async () => {
    try {
      await startCore();
      campaigns.set((await Campaigns.whatsappCampaigns()) as WhatsAppCampaign[]);
      ready.set(true);
      loadFailed.set(false);
    } catch (e) {
      loadFailed.set(true);
      report(e, { retry: () => { void loadCampaigns(); } });
    }
  };
  const load = async () => {
    if (loading()) return;
    loading.set(true);
    loadFailed.set(false);
    try {
      await startCore();
      const accs = (await ConversationNetwork.whatsappAccounts().catch(() => [])) as WhatsAppAccount[];
      accounts.set(accs);
      if (accs.length > 0 && chosenAccounts().length === 0) chosenAccounts.set([accs[0].id]);
      await loadCampaigns();
    } finally {
      loading.set(false);
    }
  };

  const toggleAccount = (id: number) => {
    const current = chosenAccounts();
    chosenAccounts.set(current.includes(id) ? current.filter((x) => x !== id) : current.concat([id]));
  };
  const openCreate = () => {
    name.set("");
    createError.set(null);
    if (accounts().length > 0 && chosenAccounts().length === 0) chosenAccounts.set([accounts()[0].id]);
    if (pick.categoryId() !== null) pick.setCategory(null);
    pick.selectAll();
    manualPhones.set("");
    recipientMode.set("crm");
    previewSeed.set(0);
    createOpen.set(true);
  };
  const closeCreate = () => { if (!creating()) createOpen.set(false); };

  const insertTag = (tag: string) => { template.set(`${template()}[${tag}]`); };
  const shufflePreview = () => { previewSeed.set(previewSeed() + 1); };

  const buildRecipients = async (ids: number[], body: string): Promise<{ rows: RecipientInput[]; skipped: number }> => {
    const rows: RecipientInput[] = [];
    let skipped = 0;
    // One read of the lead list, not one `Leads.details` round trip per chosen lead.
    const byId = new Map<number, LeadRow>();
    if (ids.length > 0) {
      for (const lead of (await LeadsMore.list()) as LeadRow[]) byId.set(lead.id, lead);
    }
    for (const id of ids) {
      const lead = byId.get(id);
      if (lead === undefined) { skipped += 1; continue; }
      const phone = lead.phone?.trim() ?? "";
      if (phone === "") { skipped += 1; continue; }
      rows.push({
        lead_id: id,
        recipient_phone_raw: phone,
        template: body,
        lead_vars: {
          name: lead.name ?? "",
          phone,
          company: lead.company ?? "",
          email: lead.email ?? "",
          notes: lead.about ?? "",
        },
      });
    }
    return { rows, skipped };
  };

  const create = async () => {
    if (creating()) return;
    createError.set(null);
    if (name().trim() === "") { createError.set("Cần nhập tên chiến dịch."); return; }
    if (chosenAccounts().length === 0) { createError.set("Cần chọn ít nhất một tài khoản WhatsApp."); return; }
    if (template().trim() === "") { createError.set("Cần nhập nội dung tin nhắn."); return; }
    if (recipientCount() === 0) { createError.set("Cần chọn ít nhất một người nhận."); return; }
    const min = validateNumber(minDelay(), 0, 3600);
    const max = validateNumber(maxDelay(), 0, 3600);
    if (min === null || max === null || max < min) { createError.set("Độ trễ chưa hợp lệ: cần số nguyên ≥ 0 và tối đa ≥ tối thiểu."); return; }
    const burst = validateNumber(burstLimit(), 1, 1000);
    const pause = validateNumber(burstPause(), 0, 3600);
    if (burst === null || pause === null) { createError.set("Giới hạn theo lô chưa hợp lệ."); return; }
    if (quietEnabled() && !/^\d{2}:\d{2}$/.test(quietStart())) { createError.set("Giờ yên tĩnh bắt đầu chưa hợp lệ (HH:MM)."); return; }
    if (quietEnabled() && !/^\d{2}:\d{2}$/.test(quietEnd())) { createError.set("Giờ yên tĩnh kết thúc chưa hợp lệ (HH:MM)."); return; }

    creating.set(true);
    try {
      await startCore();
      let rows: RecipientInput[];
      let skipped = 0;
      if (recipientMode() === "manual") {
        const phones = parsedManual();
        rows = phones.map((p) => ({
          lead_id: null,
          recipient_phone_raw: p.raw,
          template: template(),
          lead_vars: { phone: p.e164 },
        }));
      } else {
        const ids = await pick.resolve();
        const built = await buildRecipients(ids, template());
        rows = built.rows;
        skipped = built.skipped;
      }
      if (rows.length === 0) { createError.set("Không có người nhận nào có số điện thoại hợp lệ."); return; }
      const id = (await Campaigns.createWhatsappCampaign({
        name: name().trim(),
        template_id: null,
        assigned_account_ids: chosenAccounts(),
        rate_limit_min_delay_secs: min,
        rate_limit_max_delay_secs: max,
        burst_limit_per_batch: burst,
        burst_pause_secs: pause,
        quiet_hours_enabled: quietEnabled(),
        quiet_hours_start: quietStart(),
        quiet_hours_end: quietEnd(),
        timezone: timezone().trim(),
      })) as number;
      await Campaigns.populateWhatsappCampaignRecipients(id, rows, template());
      createOpen.set(false);
      notify(skipped > 0 ? `Đã tạo chiến dịch, bỏ qua ${skipped} lead không có SĐT` : "Đã tạo chiến dịch", skipped > 0 ? "warning" : "success");
      await loadCampaigns();
    } catch (e) {
      const f = report(e, { retry: () => { void create(); } });
      if (f.action === "fix") createError.set(f.message);
    } finally {
      creating.set(false);
    }
  };

  const control = async (c: WhatsAppCampaign, action: "start" | "pause" | "resume" | "cancel") => {
    try {
      await startCore();
      if (action === "start") await Campaigns.startWhatsappCampaign(c.id);
      else if (action === "pause") await Campaigns.pauseWhatsappCampaign(c.id);
      else if (action === "resume") await Campaigns.resumeWhatsappCampaign(c.id);
      else await Campaigns.cancelWhatsappCampaign(c.id);
    } catch (e) {
      report(e, {
        refresh: () => { void loadCampaigns(); },
        retry: () => { void control(c, action); },
      });
      return;
    }
    await loadCampaigns();
  };

  const loadRecipients = async (campaignId: number, reset: boolean) => {
    if (recipientsLoading()) return;
    recipientsLoading.set(true);
    if (reset) {
      recipientOffset = 0;
      recipients.set([]);
      recipientsDone.set(false);
    }
    try {
      const page = (await Campaigns.whatsappCampaignRecipients(campaignId, PAGE, recipientOffset)) as CampaignRecipient[];
      recipients.set(reset ? page : recipients().concat(page));
      recipientOffset += page.length;
      recipientsDone.set(page.length < PAGE);
      const fresh = (await Campaigns.whatsappCampaign(campaignId).catch(() => null)) as WhatsAppCampaign | null;
      if (fresh !== null) {
        selected.set(fresh);
        campaigns.set(campaigns().map((c) => (c.id === fresh.id ? fresh : c)));
      }
    } catch (e) {
      report(e, { retry: () => { void loadRecipients(campaignId, reset); } });
    } finally {
      recipientsLoading.set(false);
    }
  };
  const openDetails = (c: WhatsAppCampaign) => {
    selected.set(c);
    detailsOpen.set(true);
    void loadRecipients(c.id, true);
  };
  const closeDetails = () => { detailsOpen.set(false); selected.set(null); recipients.set([]); };
  const moreRecipients = () => {
    const c = selected();
    if (c === null || recipientsLoading() || recipientsDone()) return;
    void loadRecipients(c.id, false);
  };

  void load();

  return {
    accounts, campaigns, ready, loading, loadFailed,
    statusFilter, searchQuery, filtered, statusCount,
    totalCampaigns, runningCampaigns, totalSent, totalDelivered, totalRead, totalSkipped, deliveryReadRate,
    pick,
    createOpen, creating, createError, name, chosenAccounts, template, minDelay, maxDelay,
    burstLimit, burstPause, quietEnabled, quietStart, quietEnd, timezone,
    recipientMode, manualPhones, parsedManual, recipientCount, renderedPreview,
    openCreate, closeCreate, toggleAccount, setTemplate: (v: string) => template.set(v),
    insertTag, shufflePreview, create, loadCampaigns, control,
    detailsOpen, selected, recipients, recipientsLoading, recipientsDone,
    openDetails, closeDetails, moreRecipients,
  };
}

export type WhatsAppState = ReturnType<typeof useWhatsAppOutreach>;
