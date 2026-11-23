// Headless: the AI approval queue with the native module stubbed, so no provider is called and
// nothing is sent. Only the queue and review calls are stand-ins; leads still come from the core.
import { open, test } from "nsh/test";
import { eventually } from "./testing";
import { Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l3-ai-${Date.now()}`;

interface Proposal {
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

const has = (app: Awaited<ReturnType<typeof openQueue>>, s: string) => app.texts().some((t) => t.includes(s));

async function openQueue(options?: Parameters<typeof open>[1]) {
  await Host.start(dir);
  const app = await open("./App.tsx", options);
  await app.setViewport(1280, 800);
  await app.press("id:nav-ai_approval");
  await app.waitFor("id:ai-search");
  return app;
}

function proposal(id: number, leadId: number | null, platform: string, content: string): Proposal {
  return {
    id,
    lead_id: leadId,
    conversation_id: null,
    platform,
    proposed_content: content,
    reasoning: `Vì khách đã hỏi ${content}`,
    status: "PENDING",
    reviewed_by: null,
    reviewed_at: null,
    created_at: "2026-11-23T02:00:00Z",
  };
}

function modulesFor(pending: Proposal[], reviewed: { id: number; status: string; reviewer: string }[], evidence?: unknown) {
  return {
    modules: {
      "rustsale/native": {
        "Ai.pendingProposals": () => pending.filter((p) => p.status === "PENDING"),
        "Ai.reviewProposal": (id: number, status: string, reviewer: string) => {
          reviewed.push({ id, status, reviewer });
          const found = pending.find((p) => p.id === id);
          if (found) found.status = status;
          return null;
        },
        "Ai.leadEvidenceContext": () => evidence ?? null,
      },
    },
  };
}

test("the queue shows proposals, filters them and opens the CRM evidence", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const leadId = (await Leads.add({ name: `Khách AI ${stamp}`, phone: "0901111111", status: "New" })) as number;
  const pending = [
    proposal(1, leadId, "zalo", `Nội dung A ${stamp}`),
    proposal(2, null, "whatsapp", `Nội dung B ${stamp}`),
  ];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const evidence = {
    lead_id: leadId,
    lead_name: `Khách AI ${stamp}`,
    lead_company: "TechCorp",
    lead_phone: "0901111111",
    lead_email: "a@example.test",
    lead_status: "New",
    lead_value: 1000000,
    lead_currency: "VND",
    lead_about: null,
    lead_address: null,
    deal_stage: "PROPOSAL",
    deal_value: 25000000,
    deal_title: "Gói Enterprise",
    journey_stage: "PROPOSAL",
    active_campaign_name: null,
    recent_messages: [
      { id: 1, direction: "inbound", sender_name: "Khách", text_content: "Còn giá không?", timestamp: "2026-11-23T01:00:00Z" },
    ],
    known_products: [],
    relationship_status: null,
    categories: ["VIP"],
    verified_channels: ["zalo"],
  };

  const app = await openQueue(modulesFor(pending, reviewed, evidence));
  await app.waitUntil(() => has(app, `Nội dung A ${stamp}`));
  await app.waitUntil(() => has(app, `Nội dung B ${stamp}`));
  await app.waitUntil(() => has(app, "2 tin chờ duyệt"));
  await app.waitUntil(() => has(app, `Khách AI ${stamp}`));

  await app.type("id:ai-search", `Nội dung B ${stamp}`);
  await app.waitUntil(() => has(app, `Nội dung B ${stamp}`) && !has(app, `Nội dung A ${stamp}`));
  await app.clear("id:ai-search");
  await app.waitUntil(() => has(app, `Nội dung A ${stamp}`));

  await app.press("id:ai-evidence-1");
  await app.waitFor("id:ai-evidence-panel-1");
  await app.waitUntil(() => has(app, "Bằng chứng từ CRM"));
  await app.waitUntil(() => has(app, "Gói Enterprise"));
  await app.waitUntil(() => has(app, "Còn giá không?"));
  await app.press("text:Đóng");
});

test("approve, reject and edit call the review export and clear the queue", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const pending = [
    proposal(11, null, "zalo", `Duyệt ${stamp}`),
    proposal(12, null, "email", `Từ chối ${stamp}`),
  ];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const app = await openQueue(modulesFor(pending, reviewed));
  await app.waitUntil(() => has(app, `Duyệt ${stamp}`));

  await app.press("id:ai-approve-11");
  await app.waitUntil(() => reviewed.some((r) => r.id === 11 && r.status === "APPROVED" && r.reviewer === "Human Reviewer"));
  await app.waitUntil(() => has(app, "Bản v3 chưa tự gửi tin"));
  await app.waitUntil(() => !has(app, `Duyệt ${stamp}`));

  await app.press("id:ai-reject-12");
  await app.waitUntil(() => reviewed.some((r) => r.id === 12 && r.status === "REJECTED"));
  await app.waitUntil(() => has(app, "Đã từ chối đề xuất AI."));
  await app.waitUntil(() => has(app, "Không có tin nhắn nào đang chờ duyệt"));
});

test("an edited text cannot be approved, the stored text can", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const pending = [proposal(21, null, "zalo", `Sửa ${stamp}`)];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const app = await openQueue(modulesFor(pending, reviewed));
  await app.waitUntil(() => has(app, `Sửa ${stamp}`));

  await app.press("id:ai-edit-21");
  await app.waitFor("id:ai-edit-text");
  await app.waitUntil(() => has(app, "Bản v3 chưa có API lưu nội dung đã sửa"));
  await app.type("id:ai-edit-text", ` đã sửa`);
  await app.waitUntil(() => has(app, "không thể duyệt bản này"));
  // The button is disabled for an edited text, so pressing it must be refused.
  let refused = false;
  try { await app.press("id:ai-edit-save"); } catch { refused = true; }
  if (!refused) throw new Error("the save button must be disabled for an edited text");
  await new Promise((r) => setTimeout(r, 400));
  if (reviewed.length !== 0) throw new Error("an edited text must not be approved");

  await app.clear("id:ai-edit-text");
  await app.type("id:ai-edit-text", `Sửa ${stamp}`);
  await app.waitUntil(() => has(app, "chỉ duyệt được nội dung gốc"));
});

test("the approved and rejected tabs say the history is not readable yet", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const app = await openQueue(modulesFor([proposal(41, null, "zalo", `Tab ${stamp}`)], []));
  await app.waitUntil(() => has(app, `Tab ${stamp}`));
  await app.press("id:ai-filter-APPROVED");
  await app.waitFor("id:ai-history-note");
  await app.waitUntil(() => !has(app, "Không có tin nhắn nào đang chờ duyệt"));
});

test("a provider failure shows the error and retry loads the queue again", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const pending = [proposal(31, null, "zalo", `Lỗi ${stamp}`)];
  let fail = true;
  const modules = {
    modules: {
      "rustsale/native": {
        "Ai.pendingProposals": () => {
          if (fail) throw Object.assign(new Error("provider down"), { code: "internal" });
          return pending;
        },
        "Ai.reviewProposal": () => null,
      },
    },
  };
  const app = await openQueue(modules);
  await app.waitFor("id:ai-error");
  await app.waitUntil(() => has(app, "Có lỗi khi xử lý"));
  fail = false;
  await app.press("id:ai-refresh");
  await app.waitUntil(() => has(app, `Lỗi ${stamp}`));
  await eventually(async () => !app.shows("id:ai-error"));
});
