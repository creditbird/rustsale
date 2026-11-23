// Headless: the Pipeline screen against the real native module (temp data folder, no real data).
import { open, test } from "nsh/test";
import { eventually } from "./testing";
import { Deals, Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-pipeline-${Date.now()}`;
interface DealRow { id: number; title: string; stage: string; value: number | null; currency: string | null }

async function openPipeline(clock = false) {
  await Host.start(dir);
  const app = await open("./App.tsx", clock ? { clock: {} } : undefined);
  await app.setViewport(1280, 800);
  await app.press("id:nav-pipeline");
  await app.waitFor("id:pipe-create");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openPipeline>>, s: string) => app.texts().some((t) => t.includes(s));
const dealById = async (id: number) => ((await Deals.list()) as DealRow[]).find((d) => d.id === id);
async function newDeal(title: string, stage = "Prospecting"): Promise<number> {
  const leadId = (await Leads.add({ name: `Khách ${title}`, company: "Công ty thử", status: "New" })) as number;
  const d = (await Deals.create({ leadId, title, value: 12_000_000, stage })) as { id: number } | number;
  return typeof d === "number" ? d : d.id;
}

test("a deal moves to the next stage through the core", async () => {
  const title = `Chuyển ${Date.now()}`;
  await Host.start(dir);
  const id = await newDeal(title);
  const app = await openPipeline();
  await app.waitFor(`id:deal-${id}`);
  await app.press(`id:deal-next-${id}`);
  await eventually(async () => (await dealById(id))?.stage === "Demo");
  await app.press(`id:deal-prev-${id}`);
  await eventually(async () => (await dealById(id))?.stage === "Prospecting");
});

// Not tested here: `app.keyboard.press` reaches `onKeyCommand` shortcuts and the focused input, not the
// `onKeyDown` of a focused view, so the ← → [ ] Delete keys on a card are checked by hand on a host.

test("a deal is created and deleted", async () => {
  const stamp = Date.now();
  const leadName = `Khách tạo ${stamp}`;
  await Host.start(dir);
  await Leads.add({ name: leadName, company: "Công ty tạo", status: "New" });
  const app = await openPipeline();
  await app.press("id:pipe-create");
  await app.press("id:deal-save");
  await app.waitFor("id:deal-error");
  await app.type("id:deal-lead", leadName);
  const leads = (await Leads.page({ search: leadName }, null, 5)) as { items: { id: number }[] };
  await app.press(`id:deal-pick-${leads.items[0].id}`);
  await app.press("id:deal-stage-Proposal");
  await app.press("id:deal-save");
  let created: DealRow | undefined;
  await eventually(async () => {
    created = ((await Deals.list()) as DealRow[]).find((d) => d.title.includes("Công ty tạo"));
    return created !== undefined;
  });
  if (!created) throw new Error("the created deal is not in the core");
  if (created.stage !== "Proposal") throw new Error(`stage is ${created.stage}`);
  await app.waitFor(`id:deal-${created.id}`);
  await app.press(`id:deal-del-${created.id}`);
  await app.press("id:deal-confirm-del");
  await eventually(async () => (await dealById(created!.id)) === undefined);
  await app.waitUntil(() => !has(app, "Công ty tạo"));
});

test("a deal gone from the core refreshes the board instead of failing silently", async () => {
  const title = `Mất ${Date.now()}`;
  await Host.start(dir);
  const id = await newDeal(title);
  const app = await openPipeline();
  await app.waitFor(`id:deal-${id}`);
  await Deals.delete(id);
  await app.press(`id:deal-next-${id}`);
  await app.waitUntil(() => !app.texts().some((t) => t.includes(title)));
  await app.waitUntil(() => has(app, "không còn nữa"));
});

test("a long column scrolls: cost per scroll step", async () => {
  await Host.start(dir);
  const leadId = (await Leads.add({ name: "Khách cuộn", status: "New" })) as number;
  const N = 1500;
  for (let i = 0; i < N; i++) await Deals.create({ leadId, title: `Cuộn ${i}`, value: 1_000_000 + i, stage: "Negotiation" });
  const app = await openPipeline(true);
  await app.waitFor("id:pipe-list-Negotiation");
  const times: number[] = [];
  for (let i = 0; i < 150; i++) {
    const t = performance.now();
    await app.scroll("id:pipe-list-Negotiation", 240);
    times.push(performance.now() - t);
  }
  times.sort((a, b) => a - b);
  const q = (p: number) => times[Math.min(times.length - 1, Math.floor(p * times.length))].toFixed(2);
  console.log(`pipeline-scroll N=${N} steps=${times.length} (24 frames each, manual clock) median=${q(0.5)}ms p95=${q(0.95)}ms max=${times[times.length - 1].toFixed(2)}ms`);
}, 300000);
