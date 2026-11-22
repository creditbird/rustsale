import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Demo, GoogleMaps, Host } from "rustsale/native";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-ui2-scraper-${Date.now()}`;

async function openScraper() {
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open("./App.tsx");
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
