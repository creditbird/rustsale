import { Scroll, signal, Text, View } from "nsh";
import { Deals, LeadsMore, Settings } from "rustsale/native";
import { Badge, Button, EmptyState, Skeleton } from "../ui";
import { tone } from "../components/LeadDetail";
import { hhmm } from "../stores/format";
import { startCore } from "../stores/core";
import { report } from "../stores/toast";
import {
  convertAmount,
  dealInPeriod,
  displayMoney,
  stageBreakdown,
  summarize,
  type CurrencySettings,
  type DashboardActivity,
  type DashboardDeal,
  type DashboardLead,
  type DashboardSummary,
  type Period,
  type StageSlice,
} from "../stores/dashboard";

/** Tổng quan (UI-04): the v2 KPIs and the recent deals and activities, read from the core. */
export function DashboardScreen() {
  const summary = signal<DashboardSummary | null>(null);
  const activities = signal<DashboardActivity[]>([]);
  const currency = signal<CurrencySettings>({ base_currency: "VND", exchange_rate: 25400 });
  const leadsRaw = signal<DashboardLead[]>([]);
  const dealsRaw = signal<DashboardDeal[]>([]);
  const period = signal<Period>("");
  const failed = signal(false);
  const loading = signal(false);
  let generation = 0;

  const applyPeriod = (next: Period) => {
    period.set(next);
    const deals = dealsRaw().filter((deal) => dealInPeriod(deal, next));
    summary.set(summarize(leadsRaw(), deals, currency()));
  };
  const stages = (): StageSlice[] =>
    stageBreakdown(dealsRaw().filter((deal) => dealInPeriod(deal, period())), currency());

  const load = async () => {
    generation += 1;
    const mine = generation;
    loading.set(true);
    try {
      await startCore();
      const [leads, deals, recent, settings] = await Promise.all([
        LeadsMore.list(),
        Deals.list(),
        LeadsMore.recentActivities(5),
        Settings.currency(),
      ]);
      if (mine !== generation) return;
      const base = settings as CurrencySettings;
      currency.set(base);
      leadsRaw.set(leads as DashboardLead[]);
      dealsRaw.set(deals as DashboardDeal[]);
      summary.set(summarize(
        leads as DashboardLead[],
        (deals as DashboardDeal[]).filter((deal) => dealInPeriod(deal, period())),
        base,
      ));
      activities.set(recent as DashboardActivity[]);
      failed.set(false);
    } catch (e) {
      if (mine !== generation) return;
      failed.set(true);
      report(e, { retry: () => { void load(); } });
    } finally {
      if (mine === generation) loading.set(false);
    }
  };

  load();

  return (
    <View class="flex-1">
      <Scroll class="flex-1">
        <View class="gap-4 p-6">
          <View class="flex-row items-center justify-between">
            <View class="gap-1">
              <Text class="text-xl font-semibold text-foreground">Tổng quan</Text>
              <Text testID="dash-caption" class="text-sm text-muted-foreground">
                {summary() === null
                  ? (failed() ? "Chưa tải được dữ liệu" : "Đang mở dữ liệu…")
                  : `${summary()!.openCount} deal đang mở · ${activities().length} hoạt động gần đây`}
              </Text>
            </View>
            <Button
              testID="dash-refresh"
              label={loading() ? "Đang tải…" : "Làm mới"}
              size="sm"
              disabled={loading()}
              onPress={() => { void load(); }}
            />
          </View>

          <View testID="dash-periods" class="flex-row flex-wrap items-center gap-2">
            <Text class="text-xs text-muted-foreground">Kỳ</Text>
            <Button testID="dash-period-all" label="Tất cả" size="sm"
              variant={period() === "" ? "primary" : "secondary"} onPress={() => applyPeriod("")} />
            <Button testID="dash-period-30" label="30 ngày" size="sm"
              variant={period() === "30" ? "primary" : "secondary"} onPress={() => applyPeriod("30")} />
            <Button testID="dash-period-90" label="90 ngày" size="sm"
              variant={period() === "90" ? "primary" : "secondary"} onPress={() => applyPeriod("90")} />
            <Button testID="dash-period-year" label="Năm nay" size="sm"
              variant={period() === "year" ? "primary" : "secondary"} onPress={() => applyPeriod("year")} />
          </View>

          <If when={summary() === null && !failed()}>
            <View class="flex-row gap-4">
              <Skeleton class="h-24 flex-1" />
              <Skeleton class="h-24 flex-1" />
              <Skeleton class="h-24 flex-1" />
              <Skeleton class="h-24 flex-1" />
            </View>
            <View class="gap-2"><Skeleton /><Skeleton /><Skeleton /></View>
          </If>

          <If when={failed() && summary() === null}>
            <View class="rounded-(--radius) border border-border bg-card">
              <EmptyState title="Chưa tải được tổng quan" hint="Bấm Làm mới để thử lại." />
            </View>
          </If>

          <If when={summary()}>
            {(s) => (
              <View class="gap-4">
                <View class="flex-row flex-wrap gap-4">
                  <View testID="dash-kpi-revenue" class="min-w-40 flex-1 gap-1 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-xs font-medium text-muted-foreground">Doanh thu đã chốt</Text>
                    <Text testID="dash-kpi-revenue-value" class="text-xl font-semibold text-foreground">
                      {displayMoney(s().wonValue, currency().base_currency)}
                    </Text>
                  </View>
                  <View testID="dash-kpi-won" class="min-w-40 flex-1 gap-1 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-xs font-medium text-muted-foreground">Deal đã thắng</Text>
                    <Text testID="dash-kpi-won-value" class="text-xl font-semibold text-foreground">{`${s().wonCount} deal`}</Text>
                  </View>
                  <View testID="dash-kpi-leads" class="min-w-40 flex-1 gap-1 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-xs font-medium text-muted-foreground">Lead đang mở</Text>
                    <Text testID="dash-kpi-leads-value" class="text-xl font-semibold text-foreground">{`${s().activeLeads} lead`}</Text>
                  </View>
                  <View testID="dash-kpi-pipeline" class="min-w-40 flex-1 gap-1 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-xs font-medium text-muted-foreground">Giá trị pipeline</Text>
                    <Text testID="dash-kpi-pipeline-value" class="text-xl font-semibold text-foreground">
                      {displayMoney(s().pipelineValue, currency().base_currency)}
                    </Text>
                  </View>
                </View>

                <View class="gap-2 rounded-(--radius) border border-border bg-card p-4">
                  <Text class="text-sm font-semibold text-foreground">Phân bố theo giai đoạn</Text>
                  <Each of={stages()} key={(row) => row.stage}>
                    {(row) => (
                      <View testID={`dash-stage-${row().stage}`} class="flex-row items-center gap-3 border-b border-border py-2">
                        <Text class="min-w-0 flex-1 truncate text-sm text-foreground">{row().label}</Text>
                        <Text class="text-sm text-muted-foreground">{`${row().count} deal`}</Text>
                        <Text class="w-40 text-right text-sm text-foreground">
                          {displayMoney(row().value, currency().base_currency)}
                        </Text>
                        <Text class="w-12 text-right text-xs text-muted-foreground">{`${row().pct}%`}</Text>
                      </View>
                    )}
                  </Each>
                </View>

                <View class="flex-row gap-4">
                  <View class="flex-[3] gap-2 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-sm font-semibold text-foreground">Deal đang mở</Text>
                    <If when={s().deals.length === 0}>
                      <Text class="text-sm text-muted-foreground">Chưa có deal nào đang mở.</Text>
                    </If>
                    <Each of={s().deals} key={(d) => String(d.id)}>
                      {(d) => (
                        <View testID={`dash-deal-${d().id}`} class="flex-row items-center gap-3 border-b border-border py-2">
                          <Text class="min-w-0 flex-1 truncate text-sm text-foreground">{d().title}</Text>
                          <Badge label={d().stage} tone={tone(d().stage)} />
                          <Text class="text-sm text-foreground">
                            {displayMoney(
                              convertAmount(d().value, d().currency, currency().base_currency, currency().exchange_rate),
                              currency().base_currency,
                            )}
                          </Text>
                        </View>
                      )}
                    </Each>
                  </View>

                  <View class="flex-[2] gap-2 rounded-(--radius) border border-border bg-card p-4">
                    <Text class="text-sm font-semibold text-foreground">Hoạt động gần đây</Text>
                    <If when={activities().length === 0}>
                      <Text class="text-sm text-muted-foreground">Chưa có hoạt động.</Text>
                    </If>
                    <Each of={activities()} key={(a) => String(a.id)}>
                      {(a) => (
                        <View testID={`dash-activity-${a().id}`} class="flex-row items-start gap-2 border-b border-border py-2">
                          <Text class="min-w-0 flex-1 truncate text-sm text-foreground">{`${a().activity_type}: ${a().content}`}</Text>
                          <Text class="text-xs text-muted-foreground">{hhmm(a().created_at)}</Text>
                        </View>
                      )}
                    </Each>
                  </View>
                </View>
              </View>
            )}
          </If>
        </View>
      </Scroll>
    </View>
  );
}
