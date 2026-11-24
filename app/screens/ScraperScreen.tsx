import { List, cleanup, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { GoogleMaps, GoogleMapsMore, Host } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { runtimeVerdict } from "../stores/scraperRuntime";
import {
  STEP_DEFAULT_KM, canRetryEmail, canRetryUnits, fmtCoord, parseBbox, parseStep, planSummary,
  type Bbox, type GridPlan,
} from "../stores/scraperGrid";
import { ScraperAreaPicker } from "./ScraperAreaPicker";
import { notify, report } from "../stores/toast";

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
interface Place { id: string; name: string; address: string | null; phone: string | null; import_status: string }

const STATUS_LABEL: Record<string, string> = {
  queued: "Đang chờ", running: "Đang chạy", paused: "Tạm dừng", completed: "Hoàn tất",
  cancelled: "Đã huỷ", failed: "Lỗi", partial: "Một phần",
  paused_requires_action: "Cần xử lý", paused_limit_reached: "Chạm giới hạn", interrupted: "Bị gián đoạn",
};
const STAGE_LABEL: Record<string, string> = {
  planning: "lập kế hoạch", installing_browser_runtime: "cài trình duyệt", scraping: "đang quét",
  email_enrichment: "tìm email", importing: "nhập kết quả", cleanup: "dọn dẹp",
};
function tone(status: string): BadgeTone {
  if (status === "completed") return "success";
  if (status === "partial") return "warning";
  if (status === "failed" || status === "cancelled") return "danger";
  if (status === "paused" || status === "paused_requires_action" || status === "paused_limit_reached" || status === "interrupted") return "warning";
  return "neutral";
}
const active = (s: string) => s === "running" || s === "queued";

/** "3/10 ô · 2 ô lỗi · 14 địa điểm · đang quét": what the job has done, and which step it is on while it runs. */
export function progressLine(j: Pick<Job, "status" | "stage" | "processed_units" | "total_units" | "failed_units" | "businesses_discovered">): string {
  const parts = [`${j.processed_units}/${j.total_units} ô`];
  if (j.failed_units > 0) parts.push(`${j.failed_units} ô lỗi`);
  parts.push(`${j.businesses_discovered} địa điểm`);
  if (active(j.status) && STAGE_LABEL[j.stage]) parts.push(STAGE_LABEL[j.stage]);
  return parts.join(" · ");
}

/** Checks the form before the core is asked. */
export function validate(keyword: string): string | null {
  return keyword.trim() === "" ? "Cần nhập từ khoá tìm kiếm." : null;
}

/** Google Maps scraper: job form, job list with controls, live progress from core events, and a job's places. */
export function ScraperScreen() {
  const keyword = signal("");
  const location = signal("");
  const formError = signal<string | null>(null);
  const mode = signal<"keyword" | "grid">("keyword");
  const latMin = signal("");
  const latMax = signal("");
  const lngMin = signal("");
  const lngMax = signal("");
  const step = signal(String(STEP_DEFAULT_KM));
  const plan = signal<GridPlan | null>(null);
  const planNote = signal<string | null>(null);
  const picking = signal(false);
  const retrying = signal<string | null>(null);
  let autoLabel = "";
  let planGeneration = 0;
  const jobs = signal<Job[]>([]);
  const ready = signal(false);
  const selected = signal<Job | null>(null);
  const runtime = signal<string>("đang kiểm tra…");

  const places = pager<Place>(
    async (cursor) => {
      const job = selected();
      if (!job) return { items: [], total: 0, nextCursor: null };
      return (await GoogleMaps.resultsPage(job.id, null, cursor, 50)) as Page<Place>;
    },
    (e, retry) => { report(e, { retry }); },
  );

  const refresh = async () => {
    try {
      await startCore();
      jobs.set((await GoogleMaps.listJobs()) as Job[]);
      ready.set(true);
    } catch (e) {
      report(e, { retry: () => { void refresh(); } });
    }
  };
  const checkRuntime = async () => {
    try {
      await GoogleMaps.runtimeStatus();
      runtime.set("sẵn sàng");
    } catch (e) {
      // A missing browser is not fatal: jobs can be listed and read, only running one needs it.
      // Only `unavailable` means that; any other code is a broken check and is reported.
      const verdict = runtimeVerdict(e);
      runtime.set(verdict.label);
      if (verdict.toast) report(e, { retry: () => { void checkRuntime(); } });
    }
  };

  /**
   * Asks the core how many cells the typed box and step make. Resolves the bbox and plan when the grid is good to
   * start, `null` otherwise (the note under the fields says why). A newer call drops the answer of an older one.
   */
  const replan = async (): Promise<{ bbox: Bbox; step: number; plan: GridPlan } | null> => {
    planGeneration += 1;
    const mine = planGeneration;
    const untouched = [latMin(), latMax(), lngMin(), lngMax()].every((v) => v.trim() === "");
    const box = parseBbox(latMin(), latMax(), lngMin(), lngMax());
    const size = parseStep(step());
    if (untouched) { plan.set(null); planNote.set(null); return null; }
    if (!box.ok) { plan.set(null); planNote.set(box.error); return null; }
    if (!size.ok) { plan.set(null); planNote.set(size.error); return null; }
    try {
      const p = (await GoogleMapsMore.planGrid(box.value, size.value)) as GridPlan;
      if (mine !== planGeneration) return null;
      plan.set(p);
      planNote.set(planSummary(p));
      return p.exceeds_limit ? null : { bbox: box.value, step: size.value, plan: p };
    } catch (e) {
      if (mine !== planGeneration) return null;
      plan.set(null);
      const f = report(e, { retry: () => { void replan(); } });
      planNote.set(f.message);
      return null;
    }
  };
  const setField = (field: ReturnType<typeof signal<string>>, v: string) => { field.set(v); void replan(); };

  const pickArea = (bbox: Bbox, label: string) => {
    latMin.set(fmtCoord(bbox[0]));
    latMax.set(fmtCoord(bbox[1]));
    lngMin.set(fmtCoord(bbox[2]));
    lngMax.set(fmtCoord(bbox[3]));
    if (location().trim() === "" || location() === autoLabel) { location.set(label); autoLabel = label; }
    picking.set(false);
    void replan();
  };
  const geocode = async () => {
    const q = location().trim();
    if (q === "") { notify("Nhập tên khu vực rồi bấm “Tìm toạ độ”.", "warning"); return; }
    try {
      const box = (await GoogleMapsMore.geocodeBbox(q, null)) as Bbox | null;
      if (!box) { notify("Danh mục chưa định vị được khu vực này. Chọn vùng hoặc nhập toạ độ tay.", "warning"); return; }
      pickArea(box, q);
    } catch (e) {
      report(e, { retry: () => { void geocode(); } });
    }
  };
  const useRecommendedStep = () => {
    const r = plan()?.recommended_step_km;
    if (r === null || r === undefined) return;
    step.set(String(r));
    void replan();
  };

  const retry = async (job: Job, what: "units" | "email") => {
    if (retrying() !== null) return;
    retrying.set(job.id);
    try {
      if (what === "units") await GoogleMapsMore.retryFailedUnits(job.id);
      else await GoogleMapsMore.retryEmailEnrichment(job.id);
      notify(what === "units" ? "Đang thử lại các ô lỗi" : "Đang tìm lại email", "success");
    } catch (e) {
      report(e, { refresh: () => { void refresh(); }, retry: () => { void retry(job, what); } });
    } finally {
      retrying.set(null);
    }
    await refresh();
  };

  const create = async () => {
    const problem = validate(keyword());
    if (problem) { formError.set(problem); return; }
    let box: { bbox: Bbox; step: number } | null = null;
    if (mode() === "grid") {
      const parsed = parseBbox(latMin(), latMax(), lngMin(), lngMax());
      if (!parsed.ok) { formError.set(parsed.error); return; }
      const size = parseStep(step());
      if (!size.ok) { formError.set(size.error); return; }
      box = await replan();
      if (!box) { formError.set(plan()?.exceeds_limit ? "Vùng quá lớn so với bước lưới. Tăng bước hoặc thu nhỏ vùng." : "Chưa ước tính được lưới, xem dòng bên dưới."); return; }
    }
    formError.set(null);
    try {
      await GoogleMaps.createJob(box
        ? { mode: "grid", keyword: keyword().trim(), location: location().trim() || undefined, bbox: box.bbox, step_km: box.step, headless: true }
        : { mode: "keyword", keyword: keyword().trim(), location: location().trim() || undefined, headless: true });
      keyword.set("");
      notify("Đã tạo job", "success");
      await refresh();
    } catch (e) {
      const f = report(e, { retry: () => { void create(); } });
      if (f.action === "fix") formError.set(f.message);
    }
  };
  const control = async (job: Job, action: "pause" | "resume" | "cancel" | "delete") => {
    try {
      if (action === "pause") await GoogleMaps.pauseJob(job.id);
      else if (action === "resume") await GoogleMaps.resumeJob(job.id);
      else if (action === "cancel") await GoogleMaps.cancelJob(job.id);
      else {
        await GoogleMaps.deleteJob(job.id);
        if (selected()?.id === job.id) selected.set(null);
      }
    } catch (e) {
      report(e, { refresh: () => { void refresh(); }, retry: () => { void control(job, action); } });
      return;
    }
    await refresh();
  };
  const show = (job: Job) => { selected.set(job); void places.reload(); };

  // Progress arrives as core events; re-read the list instead of guessing the payload.
  const stop = Host.onEvent((name) => { if (name === "google-maps-scrape-job-event") void refresh(); });
  cleanup(stop);

  void refresh();
  void checkRuntime();

  return (
    <View class="flex-1 flex-row">
      <View class="flex-1 gap-4 p-6">
        <View class="flex-row items-center justify-between">
          <Text class="text-xl font-semibold text-foreground">Thu thập Google Maps</Text>
          <Text testID="runtime-status" class="text-sm text-muted-foreground">{`Trình duyệt: ${runtime()}`}</Text>
        </View>
        <View class="gap-2 rounded-(--radius) border border-border bg-card p-4">
          <View class="flex-row gap-2">
            <Button testID="scraper-mode-keyword" size="sm" variant={mode() === "keyword" ? "primary" : "secondary"} label="Theo từ khoá" onPress={() => mode.set("keyword")} />
            <Button testID="scraper-mode-grid" size="sm" variant={mode() === "grid" ? "primary" : "secondary"} label="Quét theo lưới" onPress={() => { mode.set("grid"); void replan(); }} />
          </View>
          <View class="flex-row items-center gap-2">
            <View class="flex-1"><Input testID="scraper-keyword" value={keyword()} placeholder="Từ khoá (vd: quán cà phê)" onValue={(v) => keyword.set(v)} onSubmit={() => { void create(); }} /></View>
            <View class="flex-1"><Input testID="scraper-location" value={location()} placeholder="Khu vực (vd: Quận 1, TP.HCM)" onValue={(v) => location.set(v)} /></View>
            <Button testID="scraper-start" label="Tạo job" variant="primary" onPress={() => { void create(); }} />
          </View>
          <If when={mode() === "grid"}>
            <View testID="grid-form" class="gap-2">
              <View class="flex-row items-center gap-2">
                <Button testID="grid-pick-area" label="Chọn vùng…" onPress={() => picking.set(true)} />
                <Button testID="grid-geocode" label="Tìm toạ độ theo tên khu vực" variant="ghost" onPress={() => { void geocode(); }} />
              </View>
              <View class="flex-row items-center gap-2">
                <View class="flex-1"><Input testID="grid-lat-min" value={latMin()} placeholder="Vĩ độ nhỏ" onValue={(v) => setField(latMin, v)} /></View>
                <View class="flex-1"><Input testID="grid-lat-max" value={latMax()} placeholder="Vĩ độ lớn" onValue={(v) => setField(latMax, v)} /></View>
                <View class="flex-1"><Input testID="grid-lng-min" value={lngMin()} placeholder="Kinh độ nhỏ" onValue={(v) => setField(lngMin, v)} /></View>
                <View class="flex-1"><Input testID="grid-lng-max" value={lngMax()} placeholder="Kinh độ lớn" onValue={(v) => setField(lngMax, v)} /></View>
                <View class="w-28"><Input testID="grid-step" value={step()} placeholder="Bước (km)" onValue={(v) => setField(step, v)} /></View>
              </View>
              <If when={planNote()}>
                {(note) => (
                  <View class="flex-row items-center gap-2">
                    <Text testID="grid-plan" class={`flex-1 text-sm ${plan()?.exceeds_limit || plan() === null ? "text-destructive" : "text-muted-foreground"}`}>{note()}</Text>
                    <If when={plan()?.exceeds_limit && plan()?.recommended_step_km !== null}>
                      <Button testID="grid-use-step" size="sm" label="Dùng bước gợi ý" onPress={useRecommendedStep} />
                    </If>
                  </View>
                )}
              </If>
            </View>
          </If>
          <If when={formError()}>{(m) => <Text testID="scraper-error" class="text-sm text-destructive">{m()}</Text>}</If>
        </View>
        <If when={!ready()}><View class="gap-3"><Skeleton /><Skeleton /></View></If>
        <If when={ready() && jobs().length === 0}><EmptyState title="Chưa có job nào" hint="Nhập từ khoá rồi bấm “Tạo job”." /></If>
        <List testID="job-list" each={jobs()} keyExtractor={(j) => j.id} estimatedItemSize={104} class="flex-1">
            {(j) => (
              <View class="mb-2 gap-2 rounded-(--radius) border border-border bg-card p-3">
                <View class="flex-row items-center gap-3">
                  <Base testID={`job-${j().id}`} onPress={() => show(j())} class="flex-1 gap-1">
                    <Text class="text-sm font-medium text-foreground">{`${j().keyword}${j().location ? ` · ${j().location}` : ""}`}</Text>
                    <Text testID={`job-progress-${j().id}`} class="text-xs text-muted-foreground">{progressLine(j())}</Text>
                  </Base>
                  <Badge label={STATUS_LABEL[j().status] ?? j().status} tone={tone(j().status)} />
                </View>
                <If when={j().last_error_message}>
                  {(m) => <Text testID={`job-error-${j().id}`} class="text-xs text-destructive">{m()}</Text>}
                </If>
                <View class="flex-row flex-wrap items-center gap-2">
                  <If when={active(j().status)}>
                    <Button testID={`pause-${j().id}`} size="sm" label="Tạm dừng" onPress={() => { void control(j(), "pause"); }} />
                  </If>
                  <If when={j().status === "paused"}>
                    <Button testID={`resume-${j().id}`} size="sm" label="Tiếp tục" onPress={() => { void control(j(), "resume"); }} />
                  </If>
                  <If when={canRetryUnits(j().status, j().failed_units)}>
                    <Button testID={`retry-units-${j().id}`} size="sm" label={`Thử lại ${j().failed_units} ô lỗi`} disabled={retrying() !== null} onPress={() => { void retry(j(), "units"); }} />
                  </If>
                  <If when={canRetryEmail(j().status)}>
                    <Button testID={`retry-email-${j().id}`} size="sm" label="Thử lại tìm email" disabled={retrying() !== null} onPress={() => { void retry(j(), "email"); }} />
                  </If>
                  <If when={active(j().status) || j().status === "paused"}>
                    <Button testID={`cancel-${j().id}`} size="sm" variant="ghost" label="Huỷ" onPress={() => { void control(j(), "cancel"); }} />
                  </If>
                  <If when={!active(j().status)}>
                    <Button testID={`delete-${j().id}`} size="sm" variant="ghost" label="Xoá" onPress={() => { void control(j(), "delete"); }} />
                  </If>
                </View>
              </View>
            )}
        </List>
      </View>
      <If when={selected()}>
        {(job) => (
          <View testID="results-panel" class="w-[380px] border-l border-border bg-card">
            <View class="flex-row items-center justify-between border-b border-border px-4 py-3">
              <View>
                <Text class="text-base font-semibold text-foreground">Kết quả</Text>
                <Text testID="results-caption" class="text-xs text-muted-foreground">{`${job().keyword} · ${places.total()} địa điểm`}</Text>
              </View>
              <Button label="Đóng" variant="ghost" size="sm" onPress={() => selected.set(null)} />
            </View>
            <List testID="results-list" each={places.items()} keyExtractor={(p) => p.id} estimatedItemSize={52}
              onLoadMore={() => { void places.more(); }} class="flex-1">
              {(p) => (
                <View class="gap-0.5 border-b border-border px-4 py-2.5">
                  <Text class="truncate text-sm font-medium text-foreground">{p().name}</Text>
                  <Text class="truncate text-xs text-muted-foreground">{`${p().phone ?? "Chưa có SĐT"} · ${p().address ?? ""}`}</Text>
                </View>
              )}
            </List>
          </View>
        )}
      </If>
      <ScraperAreaPicker open={picking()} onClose={() => picking.set(false)} onPick={pickArea} />
    </View>
  );
}
