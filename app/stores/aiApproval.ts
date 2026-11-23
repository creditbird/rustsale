import { computed, signal } from "nsh";
import { Ai, Leads } from "rustsale/native";
import { startCore } from "./core";
import { report } from "./toast";

/** An AI action proposal as the core's `AiActionProposal` serializes it. */
export interface AiProposal {
  id: number | null;
  lead_id: number | null;
  conversation_id: number | null;
  platform: string;
  proposed_content: string;
  reasoning: string;
  status: string;
  reviewed_by: string | null;
  reviewed_at: string | null;
  created_at: string;
}

export interface EvidenceMessage {
  id: number;
  direction: string;
  sender_name: string | null;
  text_content: string;
  timestamp: string;
}

export interface ProductEvidence {
  name: string;
  price: number;
  currency: string;
  category: string | null;
  description: string | null;
}

/** The CRM evidence the copilot reads for a lead (`Ai.leadEvidenceContext`). */
export interface LeadEvidence {
  lead_id: number;
  lead_name: string;
  lead_company: string;
  lead_phone: string;
  lead_email: string;
  lead_status: string;
  lead_value: number;
  lead_currency: string;
  lead_about: string | null;
  lead_address: string | null;
  deal_stage: string | null;
  deal_value: number | null;
  deal_title: string | null;
  journey_stage: string | null;
  active_campaign_name: string | null;
  recent_messages: EvidenceMessage[];
  known_products: ProductEvidence[];
  relationship_status: string | null;
  categories: string[];
  verified_channels: string[];
}

/** A lead's name and phone, read once so a card can show who the proposal is for. */
interface LeadInfo { name: string; phone: string | null }

export type AiStatusFilter = "PENDING" | "APPROVED" | "REJECTED" | "ALL";

export const AI_STATUSES: { id: AiStatusFilter; label: string }[] = [
  { id: "PENDING", label: "Chờ duyệt" },
  { id: "APPROVED", label: "Đã duyệt" },
  { id: "REJECTED", label: "Đã từ chối" },
  { id: "ALL", label: "Tất cả" },
];

/** "HH:MM" in the machine's time zone (the core stores UTC). */
export function hhmm(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** The screen's state: the pending queue, review actions, inline edit and the lead evidence panel. */
export function useAiApproval() {
  const proposals = signal<AiProposal[]>([]);
  const leadInfo = signal<Record<number, LeadInfo>>({});
  const ready = signal(false);
  const loading = signal(false);
  const failed = signal(false);
  const error = signal<string | null>(null);
  const success = signal<string | null>(null);
  const search = signal("");
  const statusFilter = signal<AiStatusFilter>("PENDING");
  const reviewing = signal(false);

  const editOpen = signal(false);
  const editing = signal<AiProposal | null>(null);
  const editedContent = signal("");

  const evidenceFor = signal<number | null>(null);
  const evidence = signal<LeadEvidence | null>(null);
  const evidenceLoading = signal(false);
  const evidenceError = signal<string | null>(null);
  let successTimer: ReturnType<typeof setTimeout> | null = null;

  const filtered = computed(() => {
    const q = search().trim().toLowerCase();
    return proposals().filter((p) => {
      if (statusFilter() !== "ALL" && p.status !== statusFilter()) return false;
      if (q === "") return true;
      const name = p.lead_id !== null ? leadInfo()[p.lead_id]?.name ?? "" : "";
      const phone = p.lead_id !== null ? leadInfo()[p.lead_id]?.phone ?? "" : "";
      return p.proposed_content.toLowerCase().includes(q)
        || p.reasoning.toLowerCase().includes(q)
        || name.toLowerCase().includes(q)
        || phone.includes(q);
    });
  });

  const showSuccess = (text: string) => {
    success.set(text);
    if (successTimer !== null) clearTimeout(successTimer);
    const mine = text;
    successTimer = setTimeout(() => { if (success() === mine) success.set(null); }, 4000);
  };

  const load = async () => {
    if (loading()) return;
    loading.set(true);
    failed.set(false);
    error.set(null);
    try {
      await startCore();
      const rows = ((await Ai.pendingProposals()) as AiProposal[])
        .filter((p) => typeof p.id === "number");
      proposals.set(rows);
      const known = leadInfo();
      const next = { ...known };
      for (const p of rows) {
        if (p.lead_id === null || next[p.lead_id] !== undefined) continue;
        try {
          const lead = (await Leads.details(p.lead_id)) as { name?: string | null; phone?: string | null };
          next[p.lead_id] = { name: lead.name ?? "", phone: lead.phone ?? null };
        } catch {
          // The lead may be gone; the card still shows its id.
        }
      }
      leadInfo.set(next);
      ready.set(true);
    } catch (e) {
      failed.set(true);
      const f = report(e, { retry: () => { void load(); } });
      error.set(f.message);
    } finally {
      loading.set(false);
    }
  };

  const review = async (proposal: AiProposal, status: "APPROVED" | "REJECTED", reviewer: string) => {
    if (proposal.id === null || reviewing()) return;
    reviewing.set(true);
    error.set(null);
    try {
      await Ai.reviewProposal(proposal.id, status, reviewer);
      showSuccess(status === "APPROVED"
        ? "Đã duyệt và xếp lịch gửi tin nhắn thành công!"
        : "Đã từ chối đề xuất AI.");
      await load();
    } catch (e) {
      const f = report(e, { retry: () => { void review(proposal, status, reviewer); } });
      error.set(f.message);
    } finally {
      reviewing.set(false);
    }
  };
  const approve = (p: AiProposal) => review(p, "APPROVED", "Human Reviewer");
  const reject = (p: AiProposal) => review(p, "REJECTED", "Human Reviewer");

  const openEdit = (p: AiProposal) => {
    editing.set(p);
    editedContent.set(p.proposed_content);
    editOpen.set(true);
  };
  const closeEdit = () => { if (!reviewing()) editOpen.set(false); };
  const saveAndApprove = async () => {
    const p = editing();
    if (p === null) return;
    // The core has no export that rewrites a proposal's text: the review below approves the stored text.
    await review(p, "APPROVED", "Human Reviewer (Edited)");
    editOpen.set(false);
  };

  const closeEvidence = () => { evidenceFor.set(null); evidence.set(null); evidenceError.set(null); };
  const toggleEvidence = async (p: AiProposal) => {
    if (p.lead_id === null) return;
    if (evidenceFor() === p.id) { closeEvidence(); return; }
    evidenceFor.set(p.id);
    evidence.set(null);
    evidenceError.set(null);
    evidenceLoading.set(true);
    try {
      evidence.set((await Ai.leadEvidenceContext(p.lead_id)) as LeadEvidence);
    } catch (e) {
      const f = report(e, { retry: () => { void toggleEvidence(p); } });
      evidenceError.set(f.message);
    } finally {
      evidenceLoading.set(false);
    }
  };

  void load();

  return {
    proposals, leadInfo, ready, loading, failed, error, success, search, statusFilter, filtered, reviewing,
    editOpen, editing, editedContent, openEdit, closeEdit, saveAndApprove,
    evidenceFor, evidence, evidenceLoading, evidenceError, toggleEvidence, closeEvidence,
    load, approve, reject,
  };
}

export type AiApprovalState = ReturnType<typeof useAiApproval>;
