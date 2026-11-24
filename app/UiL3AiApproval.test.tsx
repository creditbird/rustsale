// Headless: the AI approval queue with the native module stubbed, so no provider is called and
// nothing is sent. The queue's own exports are stand-ins; leads still come from the core.
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

function proposal(id: number, leadId: number | null, platform: string, content: string, status = "PENDING"): Proposal {
  return {
    id,
    lead_id: leadId,
    conversation_id: null,
    platform,
    proposed_content: content,
    reasoning: `Vì khách đã hỏi ${content}`,
    status,
    reviewed_by: status === "PENDING" ? null : "Reviewer",
    reviewed_at: status === "PENDING" ? null : "2026-11-23T03:00:00Z",
    created_at: "2026-11-23T02:00:00Z",
  };
}

function modulesFor(
  rows: Proposal[],
  reviewed: { id: number; status: string; reviewer: string }[],
  edited: { id: number; content: string }[],
  evidence?: unknown,
) {
  const byStatus = (status: unknown): Proposal[] => {
    const wanted = typeof status === "string" ? status.trim().toUpperCase() : "";
    if (wanted === "" || wanted === "ALL") return rows;
    return rows.filter((p) => p.status === wanted);
  };
  return {
    modules: {
      "rustsale/native": {
        "Ai.proposals": (status: unknown) => byStatus(status),
        "Ai.editProposal": (id: number, content: string) => {
          edited.push({ id, content });
          const found = rows.find((p) => p.id === id);
          if (found) found.proposed_content = content.trim();
          return null;
        },
        "Ai.reviewProposal": (id: number, status: string, reviewer: string) => {
          reviewed.push({ id, status, reviewer });
          const found = rows.find((p) => p.id === id);
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
  const rows = [
    proposal(1, leadId, "zalo", `Nội dung A ${stamp}`),
    proposal(2, null, "whatsapp", `Nội dung B ${stamp}`),
  ];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const edited: { id: number; content: string }[] = [];
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

  const app = await openQueue(modulesFor(rows, reviewed, edited, evidence));
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

test("approve and reject call the review export and clear the pending queue", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const rows = [
    proposal(11, null, "zalo", `Duyệt ${stamp}`),
    proposal(12, null, "email", `Từ chối ${stamp}`),
  ];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const edited: { id: number; content: string }[] = [];
  const app = await openQueue(modulesFor(rows, reviewed, edited));
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

test("an edited text is saved by the core and then approved", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const rows = [proposal(21, null, "zalo", `Sửa ${stamp}`)];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const edited: { id: number; content: string }[] = [];
  const app = await openQueue(modulesFor(rows, reviewed, edited));
  await app.waitUntil(() => has(app, `Sửa ${stamp}`));

  await app.press("id:ai-edit-21");
  await app.waitFor("id:ai-edit-text");
  await app.waitUntil(() => has(app, "Giữ nguyên nội dung gốc hoặc sửa trước khi duyệt."));
  const wanted = `Sửa ${stamp} đã sửa`;
  await app.type("id:ai-edit-text", " đã sửa");
  await app.waitUntil(() => has(app, "Nội dung đã sửa sẽ được lưu lại trước khi duyệt."));
  await app.press("id:ai-edit-save");

  await app.waitUntil(() => edited.some((e) => e.id === 21 && e.content === wanted));
  await app.waitUntil(() => reviewed.some((r) => r.id === 21 && r.status === "APPROVED" && r.reviewer === "Human Reviewer (Edited)"));
  await app.waitUntil(() => !app.shows("id:ai-edit-text"));
  await app.waitUntil(() => has(app, "Đã lưu nội dung đã sửa và duyệt"));

  await app.press("id:ai-filter-ALL");
  await app.waitUntil(() => has(app, wanted));
  await app.waitUntil(() => has(app, "ĐÃ DUYỆT"));
});

test("the history tabs show approved, rejected and applied proposals", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const rows = [
    proposal(31, null, "zalo", `Chờ ${stamp}`),
    proposal(32, null, "email", `Đã duyệt ${stamp}`, "APPROVED"),
    proposal(33, null, "zalo", `Đã từ chối ${stamp}`, "REJECTED"),
    proposal(34, null, "whatsapp", `Đã áp dụng ${stamp}`, "APPLIED"),
  ];
  const reviewed: { id: number; status: string; reviewer: string }[] = [];
  const edited: { id: number; content: string }[] = [];
  const app = await openQueue(modulesFor(rows, reviewed, edited));
  await app.waitUntil(() => has(app, `Chờ ${stamp}`));
  await app.waitUntil(() => has(app, "1 tin chờ duyệt"));

  await app.press("id:ai-filter-APPROVED");
  await app.waitUntil(() => has(app, `Đã duyệt ${stamp}`));
  await app.waitUntil(() => !has(app, `Chờ ${stamp}`));
  await app.waitUntil(() => !app.shows("id:ai-history-note"));

  await app.press("id:ai-filter-REJECTED");
  await app.waitUntil(() => has(app, `Đã từ chối ${stamp}`));
  await app.waitUntil(() => has(app, "ĐÃ TỪ CHỐI"));

  await app.press("id:ai-filter-APPLIED");
  await app.waitUntil(() => has(app, `Đã áp dụng ${stamp}`));
  await app.waitUntil(() => has(app, "ĐÃ ÁP DỤNG"));

  await app.press("id:ai-filter-ALL");
  await app.waitUntil(() => has(app, `Chờ ${stamp}`));
  await app.waitUntil(() => has(app, `Đã duyệt ${stamp}`));
  await app.waitUntil(() => has(app, `Đã từ chối ${stamp}`));
  await app.waitUntil(() => has(app, `Đã áp dụng ${stamp}`));
});

test("a provider failure shows the error and retry loads the queue again", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const pending = [proposal(41, null, "zalo", `Lỗi ${stamp}`)];
  let fail = true;
  const modules = {
    modules: {
      "rustsale/native": {
        "Ai.proposals": () => {
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
