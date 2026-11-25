// L5c of the scraper screen: the controls a job shows must match what the core accepts.
import { expect, open, test } from "nsh/test";
import { Host } from "rustsale/native";
import { canCancel, canDelete, canResume } from "./screens/ScraperScreen";

const dir = `/tmp/rustsale-v3-scraper-l5c-${Date.now()}`;

interface Job {
  id: string;
  keyword: string;
  location: string | null;
  status: string;
  stage: string;
  total_units: number;
  processed_units: number;
  failed_units: number;
  businesses_discovered: number;
  last_error_message: string | null;
  action_required: string | null;
}

function job(id: string, status: string): Job {
  return {
    id,
    keyword: `K ${id}`,
    location: null,
    status,
    stage: "scraping",
    total_units: 4,
    processed_units: 2,
    failed_units: 0,
    businesses_discovered: 8,
    last_error_message: null,
    action_required: null,
  };
}

const JOBS = [
  job("a", "paused_requires_action"),
  job("b", "paused_limit_reached"),
  job("c", "interrupted"),
  job("d", "completed"),
  job("e", "running"),
  job("f", "queued"),
];

test("the core's status rules decide which job controls exist", () => {
  for (const status of ["paused", "paused_requires_action", "paused_limit_reached", "interrupted"]) {
    expect(canResume(status)).toBe(true);
  }
  for (const status of ["queued", "running", "completed", "partial", "failed", "cancelled"]) {
    expect(canResume(status)).toBe(false);
  }
  for (const status of ["queued", "running", "paused", "paused_requires_action", "paused_limit_reached", "interrupted"]) {
    expect(canCancel(status)).toBe(true);
  }
  for (const status of ["completed", "partial", "failed", "cancelled"]) {
    expect(canCancel(status)).toBe(false);
  }
  for (const status of ["completed", "partial", "failed", "cancelled"]) {
    expect(canDelete(status)).toBe(true);
  }
  for (const status of ["queued", "running", "paused", "paused_requires_action", "paused_limit_reached", "interrupted"]) {
    expect(canDelete(status)).toBe(false);
  }
});

test("a paused or interrupted job offers resume and cancel, not delete", async () => {
  const resumed: string[] = [];
  const cancelled: string[] = [];
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "GoogleMaps.listJobs": () => JOBS,
        "GoogleMaps.resumeJob": (id: string) => {
          resumed.push(id);
          return { id };
        },
        "GoogleMaps.cancelJob": (id: string) => {
          cancelled.push(id);
          return { id };
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-scraper");
  await app.waitFor("id:job-list");

  for (const id of ["a", "b", "c"]) {
    await app.waitFor(`id:resume-${id}`);
    await app.waitFor(`id:cancel-${id}`);
    expect(app.shows(`id:delete-${id}`)).toBe(false);
  }
  expect(app.shows("id:resume-d")).toBe(false);
  expect(app.shows("id:cancel-d")).toBe(false);
  await app.waitFor("id:delete-d");
  expect(app.shows("id:resume-e")).toBe(false);
  await app.waitFor("id:cancel-e");
  expect(app.shows("id:resume-f")).toBe(false);
  await app.waitFor("id:cancel-f");

  await app.press("id:resume-a");
  await app.waitUntil(() => resumed.includes("a"));
  await app.press("id:cancel-c");
  await app.waitUntil(() => cancelled.includes("c"));
});
