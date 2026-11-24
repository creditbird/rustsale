import { List, cleanup, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { GoogleMaps, Host } from "rustsale/native";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import type { BadgeTone } from "../ui/Badge";
import { startCore, type Page } from "../stores/core";
import { pager } from "../stores/paged";
import { runtimeVerdict } from "../stores/scraperRuntime";
import { notify, report } from "../stores/toast";

interface Job {
  id: string;
  keyword: string;
  location: string | null;
  status: string;
  total_units: number;
  processed_units: number;
  businesses_discovered: number;
}
interface Place { id: string; name: string; address: string | null; phone: string | null; import_status: string }

const STATUS_LABEL: Record<string, string> = {
  queued: "Đang chờ", running: "Đang chạy", paused: "Tạm dừng", completed: "Hoàn tất",
  cancelled: "Đã huỷ", failed: "Lỗi", partial: "Một phần",
};
function tone(status: string): BadgeTone {
  if (status === "completed") return "success";
  if (status === "partial") return "warning";
  if (status === "failed" || status === "cancelled") return "danger";
  if (status === "paused") return "warning";
  return "neutral";
}
const active = (s: string) => s === "running" || s === "queued";

/** Checks the form before the core is asked. */
export function validate(keyword: string): string | null {
  return keyword.trim() === "" ? "Cần nhập từ khoá tìm kiếm." : null;
}

/** Google Maps scraper: job form, job list with controls, live progress from core events, and a job's places. */
export function ScraperScreen() {
  const keyword = signal("");
  const location = signal("");
  const formError = signal<string | null>(null);
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

  const create = async () => {
    const problem = validate(keyword());
    if (problem) { formError.set(problem); return; }
    formError.set(null);
    try {
      await GoogleMaps.createJob({ mode: "keyword", keyword: keyword().trim(), location: location().trim() || undefined, headless: true });
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
          <View class="flex-row items-center gap-2">
            <View class="flex-1"><Input testID="scraper-keyword" value={keyword()} placeholder="Từ khoá (vd: quán cà phê)" onValue={(v) => keyword.set(v)} onSubmit={() => { void create(); }} /></View>
            <View class="flex-1"><Input testID="scraper-location" value={location()} placeholder="Khu vực (vd: Quận 1, TP.HCM)" onValue={(v) => location.set(v)} /></View>
            <Button testID="scraper-start" label="Tạo job" variant="primary" onPress={() => { void create(); }} />
          </View>
          <If when={formError()}>{(m) => <Text testID="scraper-error" class="text-sm text-destructive">{m()}</Text>}</If>
        </View>
        <If when={!ready()}><View class="gap-3"><Skeleton /><Skeleton /></View></If>
        <If when={ready() && jobs().length === 0}><EmptyState title="Chưa có job nào" hint="Nhập từ khoá rồi bấm “Tạo job”." /></If>
        <List testID="job-list" each={jobs()} keyExtractor={(j) => j.id} estimatedItemSize={72} class="flex-1">
            {(j) => (
              <View class="mb-2 flex-row items-center gap-3 rounded-(--radius) border border-border bg-card p-3">
                <Base testID={`job-${j().id}`} onPress={() => show(j())} class="flex-1 gap-1">
                  <Text class="text-sm font-medium text-foreground">{`${j().keyword}${j().location ? ` · ${j().location}` : ""}`}</Text>
                  <Text class="text-xs text-muted-foreground">{`${j().processed_units}/${j().total_units} ô · ${j().businesses_discovered} địa điểm`}</Text>
                </Base>
                <Badge label={STATUS_LABEL[j().status] ?? j().status} tone={tone(j().status)} />
                <If when={active(j().status)}>
                  <Button testID={`pause-${j().id}`} size="sm" label="Tạm dừng" onPress={() => { void control(j(), "pause"); }} />
                </If>
                <If when={j().status === "paused"}>
                  <Button testID={`resume-${j().id}`} size="sm" label="Tiếp tục" onPress={() => { void control(j(), "resume"); }} />
                </If>
                <If when={active(j().status) || j().status === "paused"}>
                  <Button testID={`cancel-${j().id}`} size="sm" variant="ghost" label="Huỷ" onPress={() => { void control(j(), "cancel"); }} />
                </If>
                <If when={!active(j().status)}>
                  <Button testID={`delete-${j().id}`} size="sm" variant="ghost" label="Xoá" onPress={() => { void control(j(), "delete"); }} />
                </If>
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
    </View>
  );
}
