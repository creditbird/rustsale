// Headless: the Chiến dịch screen against the real native module (no message is sent: the scheduler is not run).
import { expect, open, test } from "nsh/test";
import { Campaigns, Host, LeadsMore, Templates } from "rustsale/native";
import {
  buildPayload, describeRemoved, filterTemplates, formatStepAnalytics, moveStep, newStep, parseTags, previewSpintax, stepsToDrafts, stringifyTags, validateCampaign,
  type TemplateRecord,
} from "./stores/campaigns";

const dir = `/tmp/rustsale-v3-ui-campaign-${Date.now()}`;
const has = (app: Awaited<ReturnType<typeof openCampaigns>>, s: string) => app.texts().some((t) => t.includes(s));

async function openCampaigns() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-campaign_center");
  await app.waitFor("id:cc-new");
  return app;
}

const list = async () => (await Campaigns.unifiedCampaigns()) as { id: number; name: string; status: string }[];

test("the draft is checked before the core is asked", () => {
  const one = [newStep(1)];
  expect(validateCampaign("", one)).not.toBeNull();
  expect(validateCampaign("A", [])).not.toBeNull();
  expect(validateCampaign("A", one)).toContain("Bước 1");
  const filled = [{ ...one[0], template_content: "Chào" }];
  expect(validateCampaign("A", filled)).toBeNull();
  const two = moveStep([{ ...filled[0] }, { ...newStep(2), template_content: "Hai" }], 1, -1);
  expect(two.map((s) => s.template_content)).toEqual(["Hai", "Chào"]);
  expect(two.map((s) => s.step_order)).toEqual([1, 2]);
  expect(buildPayload(" A ", "", filled).campaign.description).toBeNull();
});

test("a two-step campaign is created, started, paused, enrolled and reported", async () => {
  const stamp = Date.now();
  const title = `CC ${stamp}`;
  await Host.start(dir);
  const leadId = ((await LeadsMore.list()) as { id: number }[])[0].id;

  const app = await openCampaigns();
  await app.press("id:cc-new");
  await app.waitFor("id:cc-name");
  await app.type("id:cc-name", title);
  await app.type("id:cc-step-1-content", "Chào [name] bước một");
  await app.press("id:cc-add-step");
  await app.waitFor("id:cc-step-2-content");
  await app.press("id:cc-step-2-ch-email");
  await app.type("id:cc-step-2-content", "Nhắc lại bước hai");
  await app.press("id:cc-save");
  await app.waitUntil(() => has(app, title));

  const created = (await list()).find((c) => c.name === title);
  expect(created?.status).toBe("DRAFT");
  const id = created!.id;

  await app.press(`id:cc-toggle-${id}`);
  await app.waitUntil(() => has(app, "Đang chạy"));
  expect((await list()).find((c) => c.id === id)?.status).toBe("ACTIVE");
  await app.press(`id:cc-toggle-${id}`);
  await app.waitUntil(() => has(app, "Tạm dừng"));
  expect((await list()).find((c) => c.id === id)?.status).toBe("PAUSED");

  await app.press(`id:cc-enrol-${id}`);
  await app.waitFor(`id:cc-enrol-lead-${leadId}`);
  await app.press(`id:cc-enrol-lead-${leadId}`);
  await app.waitUntil(() => has(app, "Đã chọn 1 khách"));
  await app.press("id:cc-enrol-confirm");
  await app.waitUntil(() => has(app, "Đã ghi danh 1 khách"));

  await app.press(`id:cc-analytics-${id}`);
  await app.waitFor("id:cc-an-enrolled");
  await app.waitUntil(() => has(app, "Bước 1 (zalo)") && has(app, "Bước 2 (email)"));
  const a = (await Campaigns.unifiedCampaignAnalytics(id)) as { total_enrolled: number; total_steps: number };
  expect(a.total_enrolled).toBe(1);
  expect(a.total_steps).toBe(2);
  await app.press("id:cc-an-close");

  await app.press(`id:cc-delete-${id}`);
  await app.press("id:cc-delete-confirm");
  await app.waitUntil(() => !has(app, title));
  expect((await list()).some((c) => c.id === id)).toBe(false);
});

test("an empty name is refused with a sentence, nothing is saved", async () => {
  const before = (await list()).length;
  const app = await openCampaigns();
  await app.press("id:cc-new");
  await app.waitFor("id:cc-save");
  await app.press("id:cc-save");
  await app.waitUntil(() => has(app, "Cần nhập tên chiến dịch."));
  expect((await list()).length).toBe(before);
});

test("template helpers: filter, tags and a spintax preview", () => {
  const t = (id: number, channel: string, name: string, folder: number | null = null): TemplateRecord => ({
    id, folder_id: folder, name, channel, stage: null, subject: null, content: `Nội dung ${name}`, media_url: null,
    ai_prompt: null, tags_json: stringifyTags("vip, mới"),
  });
  const all = [t(1, "zalo", "Một"), t(2, "email", "Hai"), t(3, "omnichannel", "Ba", 9)];
  expect(filterTemplates(all, "zalo", null, "").map((x) => x.id)).toEqual([1, 3]);
  expect(filterTemplates(all, "all", 9, "").map((x) => x.id)).toEqual([3]);
  expect(filterTemplates(all, "all", null, "VIP").length).toBe(3);
  expect(filterTemplates(all, "all", null, "không có").length).toBe(0);
  expect(parseTags(stringifyTags("a, ,b"))).toEqual(["a", "b"]);
  expect(stringifyTags(" , ")).toBeNull();
  expect(previewSpintax("{Chào|Xin chào} {{name}}", () => 1)).toBe("Xin chào Nguyễn Văn A");
});

test("a step takes a saved template and a step is saved as a template", async () => {
  const stamp = Date.now();
  await Host.start(dir);
  const tplName = `Mẫu CC ${stamp}`;
  const tplId = Number(await Templates.createTemplate({
    name: tplName, channel: "zalo", folder_id: null, stage: "cold", subject: null,
    content: `Mẫu chào ${stamp}`, media_url: null, ai_prompt: null, tags_json: null,
  }));
  const app = await openCampaigns();
  await app.press("id:cc-new");
  await app.waitFor("id:cc-step-1-pick");
  await app.press("id:cc-step-1-pick");
  await app.waitFor(`id:cc-tpl-item-${tplId}`);
  await app.press(`id:cc-tpl-item-${tplId}`);
  await app.waitUntil(() => has(app, `Mẫu chào ${stamp}`));
  await app.press("id:cc-tpl-choose");
  await app.waitFor("id:cc-step-1-tplmark");

  await app.press("id:cc-step-1-savetpl");
  await app.waitFor("id:cc-st-name");
  await app.press("id:cc-st-save");
  await app.waitUntil(() => has(app, "Cần nhập tên mẫu."));
  const saved = `Lưu từ bước ${stamp}`;
  await app.type("id:cc-st-name", saved);
  await app.press("id:cc-st-save");
  await app.waitFor("id:cc-step-1-tplmark");
  const made = ((await Templates.list(null)) as { name: string; content: string }[]).find((x) => x.name === saved);
  expect(made?.content).toBe(`Mẫu chào ${stamp}`);
  await Templates.deleteTemplate(tplId);
});

test("stored steps become ordered drafts and an edit keeps the campaign's own fields", () => {
  const row = (id: number, order: number, text: string) => ({
    id, campaign_id: 7, step_order: order, channel: "email", delay_minutes: 60, condition_type: "ALWAYS",
    template_content: text, ai_prompt_override: null, created_at: "2026-01-01T00:00:00Z", template_id: null,
  });
  const drafts = stepsToDrafts([row(2, 5, "sau"), row(1, 2, "trước")]);
  expect(drafts.map((d) => d.template_content)).toEqual(["trước", "sau"]);
  expect(drafts.map((d) => d.step_order)).toEqual([1, 2]);
  expect(stepsToDrafts([])).toEqual([]);
  const existing = {
    id: 7, name: "Cũ", description: null, channel_type: "zalo", status: "ACTIVE", audience_filter_json: "{}",
    schedule_config_json: "{\"a\":1}", created_at: "2026-01-01T00:00:00Z", updated_at: "2026-01-01T00:00:00Z",
  };
  const body = buildPayload("Mới", "", drafts, existing);
  expect(body.campaign.id).toBe(7);
  expect(body.campaign.status).toBe("ACTIVE");
  expect(body.campaign.channel_type).toBe("zalo");
  expect(body.campaign.schedule_config_json).toBe("{\"a\":1}");
  expect(body.steps.every((x) => x.campaign_id === 7)).toBe(true);
  // The stored step ids ride along on an edit, so the core updates those steps in place.
  expect(drafts.map((d) => d.id)).toEqual([1, 2]);
  expect(body.steps.map((x) => x.id)).toEqual([1, 2]);
  expect(buildPayload("Mới", "", drafts).steps.every((x) => x.id === null)).toBe(true);
  expect(buildPayload("Mới", "", [newStep(1)], existing).steps[0].id).toBeNull();
  expect(describeRemoved([{ step_id: 9, step_order: 2, channel: "zalo", pending_leads: 3 }])).toEqual([
    "Sẽ bỏ 3 khách đang ở bước 2",
  ]);
  expect(formatStepAnalytics({
    step_id: 1, step_order: 1, channel: "zalo", retired: false,
    pending: 1, sent: 2, delivered: 0, replied: 0, failed: 0, skipped: 0,
  })).toBe("Bước 1 (zalo): chờ 1 · gửi 2 · tới 0 · phản hồi 0 · lỗi 0 · bỏ 0");
  expect(formatStepAnalytics({
    step_id: 2, step_order: 2, channel: "zalo", retired: true,
    pending: 0, sent: 1, delivered: 1, replied: 1, failed: 1, skipped: 0,
  })).toBe("Bước 2 (zalo, đã xoá): chờ 0 · gửi 1 · tới 1 · phản hồi 1 · lỗi 1 · bỏ 0");
});

const stepsOf = async (id: number) =>
  (await Campaigns.unifiedCampaignSteps(id)) as { id: number; step_order: number; channel: string; template_content: string }[];

test("a campaign is edited: its real steps load, a change is saved, status is kept", async () => {
  const stamp = Date.now();
  const title = `CC sửa ${stamp}`;
  await Host.start(dir);
  const app = await openCampaigns();
  await app.press("id:cc-new");
  await app.waitFor("id:cc-name");
  await app.type("id:cc-name", title);
  await app.type("id:cc-step-1-content", "Gốc một");
  await app.press("id:cc-add-step");
  await app.waitFor("id:cc-step-2-content");
  await app.press("id:cc-step-2-ch-email");
  await app.type("id:cc-step-2-content", "Gốc hai");
  await app.press("id:cc-save");
  await app.waitUntil(() => has(app, title));
  const id = (await list()).find((c) => c.name === title)!.id;
  await app.press(`id:cc-toggle-${id}`);
  await app.waitUntil(() => has(app, "Đang chạy"));

  await app.press(`id:cc-edit-${id}`);
  await app.waitFor("id:cc-step-2-content");
  expect(has(app, "Sửa kịch bản chiến dịch")).toBe(true);
  expect(has(app, "Bước 2 · sau 1 ngày")).toBe(true);
  await app.type("id:cc-step-2-content", " đã sửa");
  await app.press("id:cc-step-1-ch-whatsapp");
  await app.press("id:cc-save");
  await app.waitUntil(() => !has(app, "Sửa kịch bản chiến dịch"));

  const after = await stepsOf(id);
  expect(after.map((x) => x.template_content)).toEqual(["Gốc một", "Gốc hai đã sửa"]);
  expect(after.map((x) => x.channel)).toEqual(["whatsapp", "email"]);
  expect((await list()).find((c) => c.id === id)?.status).toBe("ACTIVE");
  expect((await list()).filter((c) => c.name === title).length).toBe(1);
  await Campaigns.deleteUnifiedCampaign(id);
});

test("editing a campaign with no steps shows the empty note and refuses to save it empty", async () => {
  const title = `CC rỗng ${Date.now()}`;
  await Host.start(dir);
  const stamp = new Date().toISOString();
  const id = Number(await Campaigns.saveUnifiedCampaign({
    id: null, name: title, description: null, channel_type: "omnichannel", status: "DRAFT",
    audience_filter_json: null, schedule_config_json: null, created_at: stamp, updated_at: stamp,
  }, []));
  const app = await openCampaigns();
  await app.waitUntil(() => has(app, title));
  await app.press(`id:cc-edit-${id}`);
  await app.waitFor("id:cc-steps-empty");
  await app.press("id:cc-save");
  await app.waitUntil(() => has(app, "Cần ít nhất 1 bước trong kịch bản."));
  expect((await stepsOf(id)).length).toBe(0);
  await Campaigns.deleteUnifiedCampaign(id);
});

test("a campaign that vanished shows the load error and cannot be saved", async () => {
  const title = `CC mất ${Date.now()}`;
  await Host.start(dir);
  const stamp = new Date().toISOString();
  const id = Number(await Campaigns.saveUnifiedCampaign({
    id: null, name: title, description: null, channel_type: "omnichannel", status: "DRAFT",
    audience_filter_json: null, schedule_config_json: null, created_at: stamp, updated_at: stamp,
  }, []));
  const app = await openCampaigns();
  await app.waitUntil(() => has(app, title));
  await Campaigns.deleteUnifiedCampaign(id);
  await app.press(`id:cc-edit-${id}`);
  await app.waitUntil(() => has(app, "Chưa tải được các bước của chiến dịch."));
  await app.waitFor("id:cc-steps-retry");
  expect((await list()).some((c) => c.id === id)).toBe(false);
  await app.press("id:cc-build-cancel");
});

test("removing a step with customers waiting asks first, and saving keeps the history", async () => {
  const stamp = Date.now();
  const title = `CC bỏ bước ${stamp}`;
  await Host.start(dir);
  const leadId = ((await LeadsMore.list()) as { id: number }[])[0].id;
  const at = new Date().toISOString();
  const step = (order: number, text: string) => ({
    id: null, campaign_id: 0, step_order: order, channel: "zalo", delay_minutes: order * 60, condition_type: "ALWAYS",
    template_content: text, ai_prompt_override: null, created_at: at, template_id: null,
  });
  const id = Number(await Campaigns.saveUnifiedCampaign({
    id: null, name: title, description: null, channel_type: "zalo", status: "ACTIVE",
    audience_filter_json: null, schedule_config_json: null, created_at: at, updated_at: at,
  }, [step(1, "Một"), step(2, "Hai")]));
  try {
    const before = await stepsOf(id);
    expect(await Campaigns.enrollLeadsIntoCampaign(id, [leadId])).toBe(1);

    const app = await openCampaigns();
    await app.waitUntil(() => has(app, title));
    await app.press(`id:cc-edit-${id}`);
    await app.waitFor("id:cc-step-2-content");
    await app.press("id:cc-step-remove-1");
    await app.press("id:cc-save");
    await app.waitUntil(() => has(app, "Sẽ bỏ 1 khách đang ở bước 1"));
    // Nothing is saved while the question is open, and a direct unconfirmed core call is also refused.
    expect((await stepsOf(id)).length).toBe(2);
    await expect(
      Campaigns.saveUnifiedCampaign({
        id, name: title, description: null, channel_type: "zalo", status: "ACTIVE",
        audience_filter_json: null, schedule_config_json: null, created_at: at, updated_at: at,
      }, [{ ...step(1, "Hai"), id: before[1].id, campaign_id: id }], false),
    ).rejects.toMatchObject({ code: "invalid_request" });
    expect((await stepsOf(id)).length).toBe(2);

    await app.press("id:cc-removal-cancel");
    await app.waitUntil(() => !has(app, "Sẽ bỏ 1 khách đang ở bước 1"));
    expect(has(app, "Sửa kịch bản chiến dịch")).toBe(true);
    expect((await stepsOf(id)).length).toBe(2);

    await app.press("id:cc-save");
    await app.waitFor("id:cc-removal-confirm");
    await app.press("id:cc-removal-confirm");
    await app.waitUntil(() => !has(app, "Sửa kịch bản chiến dịch"));

    const after = await stepsOf(id);
    expect(after.map((x) => x.template_content)).toEqual(["Hai"]);
    expect(after[0].id).toBe(before[1].id);
    // The customer's row is still there (cancelled, not deleted), and the retired step stays in the breakdown.
    const a = (await Campaigns.unifiedCampaignAnalytics(id)) as {
      total_executions: number;
      pending_count: number;
      total_steps: number;
      step_breakdown: { step_id: number; step_order: number; retired: boolean }[];
    };
    expect(a.total_executions).toBe(1);
    expect(a.pending_count).toBe(0);
    expect(a.total_steps).toBe(1);
    expect(a.step_breakdown.length).toBe(2);
    expect(a.step_breakdown.some((s) => s.step_id === before[0].id && s.retired)).toBe(true);
    expect(a.step_breakdown.some((s) => s.step_id === before[1].id && !s.retired)).toBe(true);

    // The analytics dialog renders the retired step with the "đã xoá" tag alongside the live step.
    await app.press(`id:cc-analytics-${id}`);
    await app.waitFor("id:cc-an-enrolled");
    await app.waitUntil(() => has(app, "Bước 1 (zalo, đã xoá)") && has(app, "Bước 1 (zalo):"));
    await app.press("id:cc-an-close");
  } finally {
    await Campaigns.deleteUnifiedCampaign(id);
  }
});
