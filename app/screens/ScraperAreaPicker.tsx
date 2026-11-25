import { List, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { GoogleMapsMore } from "rustsale/native";
import { Button, EmptyState, Input, Modal } from "../ui";
import { catalogBbox, filterNames, fold, presetBbox, slug, type Bbox, type CatalogBounds, type PresetBounds } from "../stores/scraperGrid";
import { notify, report } from "../stores/toast";

interface Country { code: string; name: string }
interface CountryStatus { country_code: string; state: string; record_count: number; last_error: string | null }
interface Area { geoname_id: number; name: string; canonical_path: string; has_children: boolean }
interface Crumb { id: number; name: string }

type Tab = "province" | "area";

/** Accent-free alphabetical order, so "Đà Nẵng" sits with the Ds. */
const byName = (a: string, b: string): number => (fold(a) < fold(b) ? -1 : fold(a) > fold(b) ? 1 : 0);

const AREA_LIMIT = 100;
const COUNTRY_ROWS = 8;

export interface AreaPickerProps {
  open: boolean;
  onClose: () => void;
  /** The chosen region: its bbox and the name the job is listed under. */
  onPick: (bbox: Bbox, label: string) => void;
}

/**
 * The region picker of the grid form: the 63 preset provinces, or any country's administrative tree from the
 * bundled GeoNames catalog (load the country first when it is not on this machine, then browse or search).
 * A pick resolves to a bbox; nothing here starts a job.
 */
export function ScraperAreaPicker(props: AreaPickerProps) {
  const tab = signal<Tab>("province");

  // Provinces: the core keeps them as one object keyed by name.
  const presets = signal<Record<string, PresetBounds>>({});
  const provinceQuery = signal("");
  const provinceNames = () => filterNames(Object.keys(presets()).sort(byName), provinceQuery());

  // Administrative catalog.
  const countries = signal<Country[]>([]);
  const countryQuery = signal("");
  const country = signal<Country | null>(null);
  const status = signal<CountryStatus | null>(null);
  const loading = signal(false);
  const areas = signal<Area[]>([]);
  const trail = signal<Crumb[]>([]);
  const areaQuery = signal("");
  const areaBusy = signal(false);
  let generation = 0;

  const visibleCountries = () => {
    const q = countryQuery().trim();
    const names = filterNames(countries().map((c) => `${c.name} (${c.code})`), q);
    const keep = new Set(names);
    return countries().filter((c) => keep.has(`${c.name} (${c.code})`)).slice(0, COUNTRY_ROWS);
  };

  const loadStatic = async () => {
    try {
      const [p, c] = await Promise.all([GoogleMapsMore.presetProvinces(), GoogleMapsMore.countries()]);
      presets.set(p as Record<string, PresetBounds>);
      countries.set((c as Country[]).slice().sort((a, b) => byName(a.name, b.name)));
    } catch (e) {
      report(e, { retry: () => { void loadStatic(); } });
    }
  };
  // Both lists are bundled with the app (no network), so read them once when the screen mounts.
  void loadStatic();

  const readAreas = async () => {
    const c = country();
    if (!c) return;
    generation += 1;
    const mine = generation;
    areaBusy.set(true);
    try {
      const q = areaQuery().trim();
      const parent = trail().length > 0 ? trail()[trail().length - 1].id : null;
      const rows = q !== ""
        ? await GoogleMapsMore.searchAreas(c.code, q, AREA_LIMIT)
        : await GoogleMapsMore.areas(c.code, parent, AREA_LIMIT);
      if (mine === generation) areas.set(rows as Area[]);
    } catch (e) {
      if (mine === generation) report(e, { retry: () => { void readAreas(); } });
    } finally {
      if (mine === generation) areaBusy.set(false);
    }
  };

  const chooseCountry = async (c: Country) => {
    country.set(c);
    trail.set([]);
    areas.set([]);
    areaQuery.set("");
    status.set(null);
    try {
      const s = (await GoogleMapsMore.countryStatus(c.code)) as CountryStatus;
      if (country()?.code !== c.code) return;
      status.set(s);
      if (s.state === "ready") await readAreas();
    } catch (e) {
      report(e, { retry: () => { void chooseCountry(c); } });
    }
  };

  const loadCountry = async () => {
    const c = country();
    if (!c || loading()) return;
    loading.set(true);
    try {
      const s = (await GoogleMapsMore.loadCountry(c.code, false)) as CountryStatus;
      if (country()?.code !== c.code) return;
      status.set(s);
      if (s.state === "ready") await readAreas();
    } catch (e) {
      report(e, { retry: () => { void loadCountry(); } });
    } finally {
      loading.set(false);
    }
  };

  const open = (a: Area) => {
    trail.set(trail().concat({ id: a.geoname_id, name: a.name }));
    areaQuery.set("");
    void readAreas();
  };
  const up = () => {
    trail.set(trail().slice(0, -1));
    areaQuery.set("");
    void readAreas();
  };

  const pickArea = async (a: Area) => {
    try {
      const b = (await GoogleMapsMore.resolveBounds(a.geoname_id)) as CatalogBounds;
      props.onPick(catalogBbox(b), a.canonical_path || a.name);
    } catch (e) {
      report(e, { retry: () => { void pickArea(a); } });
    }
  };
  const pickProvince = (name: string) => {
    const p = presets()[name];
    if (!p) { notify("Không có toạ độ của vùng này.", "danger"); return; }
    props.onPick(presetBbox(p), name);
  };

  const tabButton = (id: Tab, label: string) => (
    <Base
      testID={`area-tab-${id}`}
      accessibilityLabel={label}
      onPress={() => tab.set(id)}
      class={`rounded-full border px-3 py-1 ${tab() === id ? "border-primary bg-primary" : "border-border bg-card"}`}
    >
      <Text class={`text-sm ${tab() === id ? "text-primary-foreground" : "text-foreground"}`}>{label}</Text>
    </Base>
  );

  return (
    <Modal open={props.open} title="Chọn vùng quét" onClose={props.onClose} class="m-auto w-[560px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
      <View class="gap-3">
        <View class="flex-row gap-2">
          {tabButton("province", "Tỉnh/thành Việt Nam")}
          {tabButton("area", "Khu vực hành chính")}
        </View>

        <If when={tab() === "province"}>
          <View class="gap-2">
            <Input testID="province-search" value={provinceQuery()} onValue={(v) => provinceQuery.set(v)} placeholder="Tìm tỉnh/thành (vd: hồ chí minh)" />
            <View class="h-[300px]">
              <List testID="province-list" each={provinceNames()} keyExtractor={(n) => n} estimatedItemSize={40} class="flex-1">
                {(n) => (
                  <Base testID={`province-${slug(n())}`} accessibilityLabel={n()} onPress={() => pickProvince(n())} class="border-b border-border px-3 py-2.5 hover:bg-accent">
                    <Text class="text-sm text-foreground">{n()}</Text>
                  </Base>
                )}
              </List>
            </View>
          </View>
        </If>

        <If when={tab() === "area"}>
          <View class="gap-2">
            <If when={country() === null}>
              <View class="gap-2">
                <Input testID="country-search" value={countryQuery()} onValue={(v) => countryQuery.set(v)} placeholder="Tìm quốc gia (vd: Vietnam, VN)" />
                <Each of={visibleCountries()} key={(c) => c.code}>
                  {(c) => (
                    <Base testID={`country-${c().code}`} accessibilityLabel={c().name} onPress={() => { void chooseCountry(c()); }} class="rounded-(--radius) border border-border px-3 py-2 hover:bg-accent">
                      <Text class="text-sm text-foreground">{`${c().name} (${c().code})`}</Text>
                    </Base>
                  )}
                </Each>
              </View>
            </If>

            <If when={country()}>
              {(c) => (
                <View class="gap-2">
                  <View class="flex-row items-center justify-between">
                    <Text testID="area-country" class="text-sm font-medium text-foreground">{c().name}</Text>
                    <Button testID="area-change-country" size="sm" variant="ghost" label="Đổi quốc gia" onPress={() => { country.set(null); status.set(null); areas.set([]); trail.set([]); }} />
                  </View>

                  <If when={status() !== null && status()?.state !== "ready"}>
                    <View class="gap-2 rounded-(--radius) border border-border p-3">
                      <Text testID="area-not-loaded" class="text-sm text-muted-foreground">
                        {status()?.state === "failed"
                          ? `Lần nạp trước bị lỗi: ${status()?.last_error ?? "không rõ"}`
                          : "Chưa có dữ liệu hành chính của nước này trên máy. Cần tải từ GeoNames (cần mạng, có thể mất vài phút)."}
                      </Text>
                      <Button testID="area-load" variant="primary" label={loading() ? "Đang tải…" : "Tải dữ liệu"} disabled={loading()} onPress={() => { void loadCountry(); }} />
                    </View>
                  </If>

                  <If when={status()?.state === "ready"}>
                    <View class="gap-2">
                      <Input testID="area-search" value={areaQuery()} onValue={(v) => { areaQuery.set(v); void readAreas(); }} placeholder="Tìm khu vực theo tên" />
                      <If when={trail().length > 0 && areaQuery().trim() === ""}>
                        <View class="flex-row items-center gap-2">
                          <Button testID="area-up" size="sm" variant="ghost" label="← Lên" onPress={up} />
                          <Text testID="area-trail" class="flex-1 truncate text-xs text-muted-foreground">{trail().map((t) => t.name).join(" › ")}</Text>
                        </View>
                      </If>
                      <If when={!areaBusy() && areas().length === 0}><EmptyState title="Không có khu vực nào" hint="Thử từ khoá khác." /></If>
                      <View class="h-[260px]">
                        <List testID="area-list" each={areas()} keyExtractor={(a) => String(a.geoname_id)} estimatedItemSize={52} class="flex-1">
                          {(a) => (
                            <View class="flex-row items-center gap-2 border-b border-border px-3 py-2">
                              <View class="flex-1">
                                <Text class="truncate text-sm font-medium text-foreground">{a().name}</Text>
                                <Text class="truncate text-xs text-muted-foreground">{a().canonical_path}</Text>
                              </View>
                              <If when={a().has_children}>
                                <Button testID={`area-open-${a().geoname_id}`} size="sm" variant="ghost" label="Mở" onPress={() => open(a())} />
                              </If>
                              <Button testID={`area-pick-${a().geoname_id}`} size="sm" label="Chọn" onPress={() => { void pickArea(a()); }} />
                            </View>
                          )}
                        </List>
                      </View>
                    </View>
                  </If>
                </View>
              )}
            </If>
          </View>
        </If>

        <View class="flex-row justify-end"><Button testID="area-close" label="Đóng" onPress={props.onClose} /></View>
      </View>
    </Modal>
  );
}
