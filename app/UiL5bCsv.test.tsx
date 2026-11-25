// Headless: the leads screen's CSV import and export. The OS file dialogs cannot open in a test, so
// the native dialog calls are stubbed; the CSV itself, the mapping and the core query are real.
import { expect, open, test } from "nsh/test";
import { Host, Leads } from "rustsale/native";
import { importSay } from "./components/LeadImportDialog";
import { failure, MESSAGE_FOR_CODE } from "./stores/core";

const dir = `/tmp/rustsale-v3-ui-l5b-csv-${Date.now()}`;

type Options = {
  file_path: string;
  mapping: Record<string, number | null>;
  has_header: boolean;
  conflict_strategy: string;
  normalize_phone: boolean;
};
type App = Awaited<ReturnType<typeof open>>;

const inspection = {
  file_type: "csv",
  sheets: [],
  selected_sheet: null,
  headers: ["Tên", "Công ty", "SĐT", "Ghi chú"],
  sample_rows: [["An", "Acme", "0901", "gọi sau"]],
  total_rows_estimate: 2,
};

async function openLeads(
  state: { executed: Options[]; exported: { filename: string; content: string } | null },
  stubs: Record<string, (...args: unknown[]) => unknown> = {},
) {
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "ImportLeads.pickFile": () => "/tmp/rustsale-ui-l5b-import.csv",
        "ImportLeads.inspectFile": () => inspection,
        "ImportLeads.execute": (options: unknown) => {
          state.executed.push(options as Options);
          return { total_rows: 2, inserted: 2, updated: 0, skipped_duplicates: 0, failed: 0, error_rows: [] };
        },
        "ImportLeads.exportCsv": (filename: string, content: string) => {
          state.exported = { filename, content };
          return "/tmp/rustsale-ui-l5b-export.csv";
        },
        ...stubs,
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-leads");
  await app.waitFor("id:lead-list");
  return app;
}

const has = (app: App, text: string) => app.texts().some((t) => t.includes(text));

test("a picked file is inspected, auto-mapped and imported through the core", async () => {
  const state = { executed: [] as Options[], exported: null as { filename: string; content: string } | null };
  const app = await openLeads(state);

  await app.press("id:lead-import-open");
  await app.waitFor("id:import-pick");
  await app.press("id:import-pick");
  await app.waitFor("id:import-estimate");
  expect(has(app, "khoảng 2 dòng")).toBe(true);

  await app.press("id:import-run");
  await app.waitFor("id:import-summary");
  expect(has(app, "Đã nhập 2")).toBe(true);

  expect(state.executed.length).toBe(1);
  const sent = state.executed[0];
  expect(sent.file_path).toBe("/tmp/rustsale-ui-l5b-import.csv");
  expect(sent.has_header).toBe(true);
  expect(sent.mapping.name_col).toBe(0);
  expect(sent.mapping.company_col).toBe(1);
  expect(sent.mapping.phone_col).toBe(2);
  expect(sent.mapping.about_col).toBe(3);
  expect(sent.mapping.email_col).toBeNull();
});

test("a mapped column can be replaced before the import runs", async () => {
  const state = { executed: [] as Options[], exported: null as { filename: string; content: string } | null };
  const app = await openLeads(state);
  await app.press("id:lead-import-open");
  await app.press("id:import-pick");
  await app.waitFor("id:import-estimate");

  await app.press("id:import-map-email_col-trigger");
  await app.press("id:import-map-email_col-item-2");
  await app.press("id:import-run");
  await app.waitFor("id:import-summary");
  expect(state.executed[0].mapping.email_col).toBe(2);
});

const coreError = (code: string, message: string) => Object.assign(new Error(message), { code });
const tooLarge = "Trang tính 'Sheet2' quá lớn: 1048576 dòng x 16384 cột (giới hạn tối đa 100000 dòng, 200 cột). Vui lòng xóa các ô thừa ở xa hoặc tách nhỏ tệp trước khi nhập.";

// A stubbed native call cannot carry a code (the host stamps every thrown error ERR_NSH_HOST_ERROR),
// so the code-to-sentence step is checked as a pure function and the screen tests check the state a
// failure leaves behind.
test("the importer's own sentence shows for invalid_request only, never for other codes", () => {
  const rejected = coreError("invalid_request", tooLarge);
  expect(importSay(rejected)).toEqual({ invalid_request: tooLarge });
  const f = failure(rejected, importSay(rejected));
  expect(f.action).toBe("fix");
  expect(f.message).toBe(tooLarge);
  const other = coreError("internal", "disk exploded");
  expect(importSay(other)).toEqual({});
  expect(failure(other, importSay(other)).message).toBe(MESSAGE_FOR_CODE.internal);
  expect(importSay(coreError("invalid_request", ""))).toEqual({});
  expect(importSay(null)).toEqual({});
});

test("a failed re-inspect leaves nothing importable from the previous sheet", async () => {
  const state = { executed: [] as Options[], exported: null as { filename: string; content: string } | null };
  let calls = 0;
  const app = await openLeads(state, {
    "ImportLeads.inspectFile": () => {
      calls += 1;
      if (calls === 1) return { ...inspection, file_type: "xlsx", sheets: ["Sheet1", "Sheet2"], selected_sheet: "Sheet1" };
      throw coreError("invalid_request", tooLarge);
    },
  });
  await app.press("id:lead-import-open");
  await app.press("id:import-pick");
  await app.waitFor("id:import-estimate");
  expect(app.shows("id:import-run")).toBe(true);

  await app.press("id:import-sheet-trigger");
  await app.press("id:import-sheet-item-Sheet2");
  await app.waitUntil(() => has(app, MESSAGE_FOR_CODE.internal));
  // The first sheet's headers and mapping are gone, so nothing stale can be imported.
  expect(app.shows("id:import-estimate")).toBe(false);
  expect(app.shows("id:import-map-name_col-trigger")).toBe(false);
  expect(state.executed.length).toBe(0);
});

test("a failed import clears its progress", async () => {
  const state = { executed: [] as Options[], exported: null as { filename: string; content: string } | null };
  const app = await openLeads(state, {
    "ImportLeads.execute": () => {
      throw coreError("invalid_request", tooLarge);
    },
  });
  await app.press("id:lead-import-open");
  await app.press("id:import-pick");
  await app.waitFor("id:import-estimate");
  await app.press("id:import-run");
  await app.waitUntil(() => has(app, MESSAGE_FOR_CODE.internal));
  expect(app.shows("id:import-progress")).toBe(false);
});

test("export writes every matching lead as CSV with the lead's own fields", async () => {
  const state = { executed: [] as Options[], exported: null as { filename: string; content: string } | null };
  await Host.start(dir);
  const name = `Csv ${Date.now()}`;
  await Leads.add({ name, company: "Acme", phone: "0901234567", owner: "An", value: 15_000_000, status: "New" });

  const app = await openLeads(state);
  await app.waitFor("id:lead-list");
  await app.press("id:lead-export");
  await app.waitUntil(() => state.exported !== null, { timeout: 8000 });

  const saved = state.exported!;
  expect(saved.filename.startsWith("RustSale_Leads_")).toBe(true);
  expect(saved.filename.endsWith(".csv")).toBe(true);
  const lines = saved.content.split("\r\n");
  expect(lines[0]).toContain('"Name"');
  expect(lines[0]).toContain('"Zalo_Status"');
  const row = lines.find((line) => line.includes(name));
  expect(row).toBeDefined();
  expect(row).toContain('"Acme"');
  expect(row).toContain("'0901234567");
  expect(row).toContain('"15000000"');
  expect(row).toContain('"UNTESTED"');
});
