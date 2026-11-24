import { cleanup, signal, Text, View } from "nsh";
import { Select } from "nsh/base";
import { Host, ImportLeads } from "rustsale/native";
import { Button, Checkbox, Input, Modal } from "../ui";
import {
  MAPPING_FIELDS,
  autoMap,
  emptyMapping,
  type ColumnMapping,
  type ImportInspection,
  type ImportProgress,
  type ImportSummary,
} from "../stores/leadCsv";
import { notify, report } from "../stores/toast";

const CONFLICTS = [
  { key: "skip", label: "Bỏ qua dòng trùng" },
  { key: "overwrite", label: "Ghi đè dòng trùng" },
  { key: "allow", label: "Cho phép dòng trùng" },
];

export interface LeadImportDialogProps {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
}

/** Choose a CSV/Excel file, map its columns, run the core's importer and show the summary. */
export function LeadImportDialog(props: LeadImportDialogProps) {
  const path = signal<string | null>(null);
  const inspection = signal<ImportInspection | null>(null);
  const mapping = signal<ColumnMapping>(emptyMapping());
  const conflict = signal("skip");
  const defaultStatus = signal("");
  const normalizePhone = signal(true);
  const picking = signal(false);
  const running = signal(false);
  const progress = signal<ImportProgress | null>(null);
  const summary = signal<ImportSummary | null>(null);
  const error = signal<string | null>(null);

  const stop = Host.onEvent((name, payload) => {
    if (name !== "lead-import-progress") return;
    const p = payload as Partial<ImportProgress> | null;
    if (p === null || typeof p.percentage !== "number") return;
    progress.set({
      processed_rows: typeof p.processed_rows === "number" ? p.processed_rows : 0,
      total_rows: typeof p.total_rows === "number" ? p.total_rows : 0,
      percentage: p.percentage,
    });
  });
  cleanup(() => stop());

  const reset = () => {
    path.set(null);
    inspection.set(null);
    mapping.set(emptyMapping());
    progress.set(null);
    summary.set(null);
    error.set(null);
    running.set(false);
    defaultStatus.set("");
  };
  const close = () => {
    reset();
    props.onClose();
  };

  const inspect = async (file: string, sheet: string | null) => {
    error.set(null);
    summary.set(null);
    try {
      const found = (await ImportLeads.inspectFile(file, sheet ?? undefined)) as ImportInspection;
      inspection.set(found);
      mapping.set(autoMap(found.headers));
    } catch (e) {
      const f = report(e, { retry: () => { void inspect(file, sheet); } });
      if (f.action === "fix") error.set(f.message);
    }
  };
  const pick = async () => {
    if (picking()) return;
    picking.set(true);
    try {
      const chosen = (await ImportLeads.pickFile()) as string | null;
      if (chosen !== null) {
        path.set(chosen);
        await inspect(chosen, null);
      }
    } catch (e) {
      const f = report(e, { retry: () => { void pick(); } });
      if (f.action === "fix") error.set(f.message);
    } finally {
      picking.set(false);
    }
  };
  const pickSheet = (sheet: string) => {
    const file = path();
    if (file === null) return;
    const found = inspection();
    if (found !== null) inspection.set({ ...found, selected_sheet: sheet });
    void inspect(file, sheet);
  };
  const pickColumn = (key: keyof ColumnMapping, value: string) => {
    mapping.set({ ...mapping(), [key]: value === "" ? null : Number(value) });
  };
  const run = async () => {
    const file = path();
    const found = inspection();
    if (file === null || found === null || running()) return;
    if (mapping().name_col === null) {
      error.set("Cần chọn cột cho Tên khách.");
      return;
    }
    running.set(true);
    error.set(null);
    summary.set(null);
    progress.set({ processed_rows: 0, total_rows: found.total_rows_estimate, percentage: 0 });
    try {
      const result = (await ImportLeads.execute({
        file_path: file,
        sheet_name: found.selected_sheet,
        has_header: true,
        header_row_index: 0,
        mapping: mapping(),
        conflict_strategy: conflict(),
        default_category_id: null,
        default_status: defaultStatus().trim() || null,
        default_owner: null,
        default_currency: null,
        normalize_phone: normalizePhone(),
      })) as ImportSummary;
      summary.set(result);
      notify(`Đã nhập ${result.inserted} khách`, "success");
      props.onImported();
    } catch (e) {
      const f = report(e, { retry: () => { void run(); } });
      if (f.action === "fix") error.set(f.message);
    } finally {
      running.set(false);
    }
  };

  const headers = () => inspection()?.headers ?? [];
  const headerName = (index: number | null) => {
    if (index === null) return "— Không dùng —";
    return headers()[index] ?? "— Không dùng —";
  };
  const columnItems = (current: number | null) => [
    { value: "", title: "— Không dùng —" },
    ...headers().map((header, index) => ({ value: String(index), title: header || `Cột ${index + 1}` })),
  ];
  const sheetName = () => inspection()?.selected_sheet ?? inspection()?.sheets[0] ?? "";

  return (
    <Modal open={props.open} title="Nhập khách từ tệp" onClose={close}
      class="m-auto w-[760px] max-w-full rounded-(--radius) border border-border bg-popover p-5">
      <View class="gap-3">
        <View class="flex-row items-center gap-3">
          <Button testID="import-pick" label="Chọn tệp CSV / Excel" variant="secondary"
            onPress={() => { void pick(); }} />
          <Text testID="import-file" class="truncate text-sm text-muted-foreground">
            {path() ?? "Chưa chọn tệp"}
          </Text>
        </View>

        <If when={inspection() !== null}>
          <View class="gap-1">
            <Text testID="import-estimate" class="text-sm text-foreground">
              {`${inspection()?.file_type === "csv" ? "CSV" : "Excel"} · khoảng ${inspection()?.total_rows_estimate ?? 0} dòng`}
            </Text>
            <If when={(inspection()?.sheets.length ?? 0) > 1}>
              <View class="w-64">
                <Select testID="import-sheet"
                  items={(inspection()?.sheets ?? []).map((sheet) => ({ value: sheet, title: sheet }))}
                  value={sheetName()}
                  placeholder="Chọn sheet"
                  onChange={({ value }) => { if (value !== null) pickSheet(value); }}
                  trigger={
                    <View testID="import-sheet-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                      <Text class="truncate text-sm text-foreground">{sheetName()}</Text>
                      <Text class="text-xs text-muted-foreground">▾</Text>
                    </View>
                  }
                  itemClass="px-3 py-2 selected:bg-accent"
                  renderItem={(entry) => (
                    <View testID={`import-sheet-item-${entry.value}`}>
                      <Text class="text-sm text-foreground">{entry.title}</Text>
                    </View>
                  )} />
              </View>
            </If>
          </View>
        </If>

        <If when={inspection() !== null}>
          <View class="flex-row flex-wrap gap-2">
            <Each of={MAPPING_FIELDS} key={(f) => f.key}>
              {(field) => (
                <View class="w-[214px] gap-1">
                  <Text class="text-xs text-muted-foreground">{field().label}</Text>
                  <Select testID={`import-map-${field().key}`}
                    items={columnItems(mapping()[field().key])}
                    value={mapping()[field().key] === null ? "" : String(mapping()[field().key])}
                    placeholder="— Không dùng —"
                    onChange={({ value }) => pickColumn(field().key, value ?? "")}
                    trigger={
                      <View testID={`import-map-${field().key}-trigger`} class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                        <Text class="truncate text-sm text-foreground">{headerName(mapping()[field().key])}</Text>
                        <Text class="text-xs text-muted-foreground">▾</Text>
                      </View>
                    }
                    itemClass="px-3 py-2 selected:bg-accent"
                    renderItem={(entry) => (
                      <View testID={`import-map-${field().key}-item-${entry.value === "" ? "none" : entry.value}`}>
                        <Text class="text-sm text-foreground">{entry.title}</Text>
                      </View>
                    )} />
                </View>
              )}
            </Each>
          </View>

          <View class="flex-row flex-wrap items-center gap-3">
            <View class="w-56">
              <Select testID="import-conflict"
                items={CONFLICTS.map((c) => ({ value: c.key, title: c.label }))}
                value={conflict()}
                placeholder="Bỏ qua dòng trùng"
                onChange={({ value }) => { conflict.set(value ?? "skip"); }}
                trigger={
                  <View testID="import-conflict-trigger" class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                    <Text class="truncate text-sm text-foreground">
                      {CONFLICTS.find((c) => c.key === conflict())?.label ?? "Bỏ qua dòng trùng"}
                    </Text>
                    <Text class="text-xs text-muted-foreground">▾</Text>
                  </View>
                }
                itemClass="px-3 py-2 selected:bg-accent"
                renderItem={(entry) => (
                  <View testID={`import-conflict-item-${entry.value}`}>
                    <Text class="text-sm text-foreground">{entry.title}</Text>
                  </View>
                )} />
            </View>
            <View class="w-40">
              <Input testID="import-status" value={defaultStatus()} placeholder="Trạng thái mặc định"
                onValue={(v) => defaultStatus.set(v)} />
            </View>
            <Checkbox testID="import-normalize" checked={normalizePhone()}
              onChange={(checked) => normalizePhone.set(checked)} label="Chuẩn hoá SĐT" />
          </View>
        </If>

        <If when={progress() !== null}>
          <Text testID="import-progress" class="text-sm text-muted-foreground">
            {`Đang nhập ${progress()?.processed_rows ?? 0}/${progress()?.total_rows ?? 0} (${progress()?.percentage ?? 0}%)`}
          </Text>
        </If>
        <If when={summary() !== null}>
          <Text testID="import-summary" class="text-sm text-foreground">
            {`Đã nhập ${summary()?.inserted ?? 0}, cập nhật ${summary()?.updated ?? 0}, bỏ qua ${summary()?.skipped_duplicates ?? 0}, lỗi ${summary()?.failed ?? 0} / ${summary()?.total_rows ?? 0} dòng`}
          </Text>
        </If>
        <If when={error() !== null}>
          <Text testID="import-error" class="text-sm text-destructive">{error() ?? ""}</Text>
        </If>

        <View class="flex-row justify-end gap-2">
          <Button label="Đóng" onPress={close} />
          <Button testID="import-run" label="Nhập" variant="primary"
            onPress={() => { void run(); }} />
        </View>
      </View>
    </Modal>
  );
}
