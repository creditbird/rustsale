import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Demo, GoogleMaps, Host } from "rustsale/native";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui2-scraper-${Date.now()}`;

async function openScraper(entry = "./App.tsx") {
  await Demo.prepare(dir);
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open(entry);
  await app.setViewport(1280, 800);
  await app.press("id:nav-scraper");
  await app.waitFor("id:scraper-start");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openScraper>>, s: string) => app.texts().some((t) => t.includes(s));

test("a job needs a keyword", async () => {
  const app = await openScraper();
  await app.press("id:scraper-start");
  await app.waitFor("id:scraper-error");
  expect(has(app, "Cần nhập từ khoá")).toBe(true);
});

test("seeded jobs are listed and a job's places page in from the core", async () => {
  const app = await openScraper();
  await app.waitUntil(() => has(app, "quán cà phê"));
  await app.press("id:job-demo-job-1");
  await app.waitUntil(() => has(app, "25 địa điểm"));
  expect(has(app, "quán cà phê số 0")).toBe(true);
});

test("a paused job resumes through the core", async () => {
  const app = await openScraper();
  await app.waitFor("id:resume-demo-job-2");
  await app.press("id:resume-demo-job-2");
  await eventually(async () => ((await GoogleMaps.getJob("demo-job-2")) as { status: string }).status !== "paused");
});

test("a scrape job event from the core refreshes the list", async () => {
  const app = await openScraper("./harness/EventApp.tsx");
  await app.waitUntil(() => has(app, "nhà hàng"));
  const before = ((await GoogleMaps.listJobs()) as unknown[]).length;
  await GoogleMaps.createJob({ mode: "keyword", keyword: "tiệm bánh sự kiện", headless: true });
  expect(((await GoogleMaps.listJobs()) as unknown[]).length).toBe(before + 1);
  // The core's own event name triggers the reload; the old name does not.
  await app.press("id:emit-scrape");
  await app.waitUntil(() => has(app, "tiệm bánh sự kiện"), { timeout: 4000 });
});
