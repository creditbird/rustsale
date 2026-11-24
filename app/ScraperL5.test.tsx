// L5 of the scraper screen: retry failed cells / email (UI-11) and the grid/bbox region picker (UI-14).
import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Demo, GoogleMaps, GoogleMapsMore, Host } from "rustsale/native";
import {
  canRetryEmail, canRetryUnits, filterNames, fmtCoord, parseBbox, parseStep, planSummary, slug, thousands,
  type GridPlan,
} from "./stores/scraperGrid";

let seeded: Promise<unknown> | undefined;
const dir = `/tmp/rustsale-v3-scraper-l5-${Date.now()}`;

async function openScraper() {
  await Demo.prepare(dir);
  await Host.start(dir);
  seeded ??= Demo.seed();
  await seeded;
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-scraper");
  await app.waitFor("id:scraper-start");
  return app;
}
type App = Awaited<ReturnType<typeof openScraper>>;
const has = (app: App, s: string) => app.texts().some((t) => t.includes(s));
type Job = { status: string; mode: string; bbox: number[] | null; step_km: number | null; location: string | null; version: number; failed_units: number };

const HCM = [10.37, 11.16, 106.36, 107.03];

test("bbox and step mistakes each get their own sentence", () => {
  expect(parseBbox("10.37", "11.16", "106.36", "107.03")).toEqual({ ok: true, value: HCM });
  expect(parseBbox("10,37", "11,16", "106,36", "107,03")).toEqual({ ok: true, value: HCM });
  const bad = (a: string, b: string, c: string, d: string) => {
    const r = parseBbox(a, b, c, d);
    return r.ok ? "" : r.error;
  };
  expect(bad("", "11", "106", "107")).toContain("đủ 4 toạ độ");
  expect(bad("abc", "11", "106", "107")).toContain("đủ 4 toạ độ");
  expect(bad("10", "95", "106", "107")).toContain("Vĩ độ phải");
  expect(bad("10", "11", "106", "181")).toContain("Kinh độ phải");
  expect(bad("11", "10", "106", "107")).toContain("Vĩ độ nhỏ");
  expect(bad("10", "11", "107", "106")).toContain("Kinh độ nhỏ");
  expect(parseStep("2")).toEqual({ ok: true, value: 2 });
  expect(parseStep("0,2")).toEqual({ ok: true, value: 0.2 });
  for (const text of ["", "x", "0.1", "51", "NaN", "Infinity"]) expect(parseStep(text).ok).toBe(false);
});

test("names are found without accents; ids and numbers format", () => {
  const names = ["Thành phố Hồ Chí Minh", "Thành phố Đà Nẵng", "Tỉnh An Giang"];
  expect(filterNames(names, "ho chi minh")).toEqual([names[0]]);
  expect(filterNames(names, "da nang")).toEqual([names[1]]);
  expect(filterNames(names, "  ")).toEqual(names);
  expect(slug(names[0])).toBe("thanh-pho-ho-chi-minh");
  expect(fmtCoord(10.370000001)).toBe("10.37");
  expect(fmtCoord(106.123456)).toBe("106.1235");
  expect(thousands(1234567)).toBe("1.234.567");
  expect(thousands(12)).toBe("12");
});

test("a retry button shows only where the core takes it", () => {
  expect(canRetryUnits("partial", 2)).toBe(true);
  expect(canRetryUnits("failed", 1)).toBe(true);
  expect(canRetryUnits("partial", 0)).toBe(false);
  for (const s of ["running", "queued", "paused", "completed", "cancelled", "interrupted"]) expect(canRetryUnits(s, 3)).toBe(false);
  expect(canRetryEmail("partial")).toBe(true);
  for (const s of ["failed", "completed", "running", "paused"]) expect(canRetryEmail(s)).toBe(false);
});

test("the plan line names cells and, over the limit, the step to use", () => {
  const plan: GridPlan = {
    area_km2: 5000, width_km: 70, height_km: 90, requested_step_km: 2, effective_step_km: 2, estimated_cells: 1500,
    recommended_step_km: null, exceeds_limit: false, estimated_chunks: 1,
  };
  expect(planSummary(plan)).toBe("~1.500 ô · 5.000 km² · bước 2 km");
  const over = planSummary({ ...plan, estimated_cells: 40000, exceeds_limit: true, recommended_step_km: 4.1 });
  expect(over).toContain("Vượt giới hạn");
  expect(over).toContain("4.1 km");
});

test("a job that finished with gaps offers the retries; a completed one does not", async () => {
  const app = await openScraper();
  await app.waitFor("id:retry-units-demo-job-3");
  await app.waitFor("id:retry-email-demo-job-3");
  expect(has(app, "Thử lại 1 ô lỗi")).toBe(true);
  expect(has(app, "1 ô lỗi")).toBe(true);
  expect(app.shows("id:retry-units-demo-job-1")).toBe(false);
  expect(app.shows("id:retry-email-demo-job-1")).toBe(false);
  expect(app.shows("id:retry-units-demo-job-4")).toBe(false);
  await app.waitFor("id:retry-email-demo-job-4");
});

test("'Thử lại ô lỗi' asks the core and the job leaves the partial state", async () => {
  const app = await openScraper();
  await app.waitFor("id:retry-units-demo-job-3");
  const before = (await GoogleMaps.getJob("demo-job-3")) as Job;
  expect(before.status).toBe("partial");
  await app.press("id:retry-units-demo-job-3");
  await eventually(async () => ((await GoogleMaps.getJob("demo-job-3")) as Job).version > before.version);
});

test("'Thử lại tìm email' reopens the partial job through the core", async () => {
  const app = await openScraper();
  await app.waitFor("id:retry-email-demo-job-4");
  const before = (await GoogleMaps.getJob("demo-job-4")) as Job;
  expect(before.status).toBe("partial");
  await app.press("id:retry-email-demo-job-4");
  await eventually(async () => ((await GoogleMaps.getJob("demo-job-4")) as Job).version > before.version);
});

test("grid mode estimates the cells of the typed box and refuses a step the backend refuses", async () => {
  const app = await openScraper();
  await app.press("id:scraper-mode-grid");
  await app.waitFor("id:grid-lat-min");
  await app.type("id:grid-lat-min", "10.37");
  await app.type("id:grid-lat-max", "11.16");
  await app.type("id:grid-lng-min", "106.36");
  await app.type("id:grid-lng-max", "107.03");
  await app.waitUntil(() => has(app, "ô · ") && has(app, "bước 2 km"));
  const plan = (await GoogleMapsMore.planGrid(HCM, 2)) as GridPlan;
  expect(has(app, `~${thousands(plan.estimated_cells)} ô`)).toBe(true);

  // A step below the floor never reaches the core: the sentence says the range.
  await app.type("id:grid-step", "0");
  await app.waitUntil(() => has(app, "Bước lưới phải từ"));
});

test("a box over the cell limit does not start a job", async () => {
  const app = await openScraper();
  const before = ((await GoogleMaps.listJobs()) as unknown[]).length;
  await app.press("id:scraper-mode-grid");
  await app.type("id:scraper-keyword", "cửa hàng lớn");
  await app.type("id:grid-lat-min", "8");
  await app.type("id:grid-lat-max", "23");
  await app.type("id:grid-lng-min", "102");
  await app.type("id:grid-lng-max", "110");
  await app.type("id:grid-step", ".2");
  await app.waitUntil(() => has(app, "Vượt giới hạn"));
  await app.press("id:scraper-start");
  await app.waitFor("id:scraper-error");
  expect(has(app, "Vùng quá lớn")).toBe(true);
  expect(((await GoogleMaps.listJobs()) as unknown[]).length).toBe(before);
  // The core's own advice is one tap away and clears the problem.
  await app.press("id:grid-use-step");
  await app.waitUntil(() => !has(app, "Vượt giới hạn"));
});

test("a province from the dialog fills the box and the job is created in grid mode with it", async () => {
  const app = await openScraper();
  await app.press("id:scraper-mode-grid");
  await app.press("id:grid-pick-area");
  await app.waitFor("id:province-search");
  await app.type("id:province-search", "ho chi minh");
  await app.waitFor("id:province-thanh-pho-ho-chi-minh");
  await app.press("id:province-thanh-pho-ho-chi-minh");
  await app.waitUntil(() => has(app, "ô · "));
  await app.type("id:scraper-keyword", "quán bún L5");
  await app.press("id:scraper-start");
  await eventually(async () => ((await GoogleMaps.listJobs()) as Array<Job & { keyword: string }>).some((j) => j.keyword === "quán bún L5"));
  const job = ((await GoogleMaps.listJobs()) as Array<Job & { keyword: string }>).find((j) => j.keyword === "quán bún L5") as Job;
  expect(job.mode).toBe("grid");
  expect(job.bbox).toEqual(HCM);
  expect(job.step_km).toBe(2);
  expect(job.location).toBe("Thành phố Hồ Chí Minh");
});

test("the administrative tab lists countries and either offers the download or browses the loaded tree", async () => {
  const app = await openScraper();
  await app.press("id:scraper-mode-grid");
  await app.press("id:grid-pick-area");
  await app.press("id:area-tab-area");
  await app.waitFor("id:country-search");
  await app.type("id:country-search", "viet nam");
  await app.waitFor("id:country-VN");
  const status = (await GoogleMapsMore.countryStatus("VN")) as { state: string };
  await app.press("id:country-VN");
  if (status.state === "ready") {
    await app.waitFor("id:area-list");
    await app.waitFor("id:area-search");
  } else {
    // Nothing is downloaded here: the dialog says so and offers the load, it does not call the network on its own.
    await app.waitFor("id:area-load");
    expect(has(app, "Chưa có dữ liệu hành chính")).toBe(true);
  }
  await app.press("id:area-change-country");
  await app.waitFor("id:country-search");
});
