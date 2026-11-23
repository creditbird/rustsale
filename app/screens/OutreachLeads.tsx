import { List, Text, View } from "nsh";
import { Button as Base, Select } from "nsh/base";
import { Badge, Button, EmptyState, Input, Skeleton } from "../ui";
import { tone } from "../components/LeadDetail";
import { SEGMENTATIONS, type OutreachPicker, type Segmentation } from "../stores/outreach";

export interface OutreachLeadsProps {
  pick: OutreachPicker;
  prefix: string;
  /** The Zalo-only toggles (verified Zalo, mobile numbers) hidden on the email screen. */
  zalo: boolean;
  /** Hide the outreach-history segmentation on screens that do not use it (WhatsApp). */
  showSegmentation?: boolean;
  /** Outreach templates of the channel, for the "successful template match" segmentation. */
  templates?: { id: number; name: string }[];
}

const COLUMNS: { label: string; w: string }[] = [
  { label: "Tên", w: "flex-[2]" },
  { label: "Công ty", w: "flex-[2]" },
  { label: "Liên hệ", w: "flex-[2]" },
  { label: "Trạng thái", w: "w-28" },
];

/** The lead table both outreach screens share: filters, select-all with exclusions, paging and a run count. */
export function OutreachLeads(props: OutreachLeadsProps) {
  const p = props.pick;
  const t = (name: string) => `${props.prefix}-${name}`;
  const chooseSegmentation = (key: Segmentation) => {
    if (key === "template_success") {
      const first = props.templates?.[0]?.id ?? null;
      p.setSegmentationAndTemplate(key, p.filterTemplateId() ?? first);
      return;
    }
    p.setSegmentation(key);
  };
  const categoryName = () => p.categories().find((c) => c.id === p.categoryId())?.name ?? "Tất cả danh mục";
  const templateName = () =>
    (props.templates ?? []).find((tpl) => tpl.id === p.filterTemplateId())?.name ?? "Chọn mẫu đã gửi";

  return (
    <View class="flex-1 gap-2">
      <View class="flex-row flex-wrap items-center gap-2">
        <View class="w-64">
          <Input testID={t("search")} value={p.search()} placeholder="Tìm tên, công ty, SĐT…" onValue={p.setSearch} />
        </View>
        <View class="w-48">
          <Select testID={t("category")}
            items={[{ value: "", title: "Tất cả danh mục" },
              ...p.categories().map((c) => ({ value: String(c.id), title: c.name }))]}
            value={p.categoryId() === null ? "" : String(p.categoryId())}
            placeholder="Tất cả danh mục"
            onChange={({ value }) => p.setCategory(value === null || value === "" ? null : Number(value))}
            trigger={
              <View testID={t("category-trigger")} class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                <Text class="truncate text-sm text-foreground">{categoryName()}</Text>
                <Text class="text-xs text-muted-foreground">▾</Text>
              </View>
            }
            itemClass="px-3 py-2 selected:bg-accent"
            renderItem={(entry) => (
              <View testID={t(`category-item-${entry.value === "" ? "all" : entry.value}`)}>
                <Text class="text-sm text-foreground">{entry.title}</Text>
              </View>
            )} />
        </View>
        <If when={props.showSegmentation ?? true}>
          <Each of={SEGMENTATIONS} key={(s) => s.key}>
            {(s) => (
              <Button testID={t(`seg-${s().key}`)} label={s().label} size="sm"
                variant={p.segmentation() === s().key ? "primary" : "secondary"}
                onPress={() => chooseSegmentation(s().key)} />
            )}
          </Each>
        </If>
        <If when={(props.showSegmentation ?? true) && p.segmentation() === "template_success"}>
          <View class="w-52">
            <Select testID={t("seg-template")}
              items={(props.templates ?? []).map((tpl) => ({ value: String(tpl.id), title: tpl.name }))}
              value={p.filterTemplateId() === null ? null : String(p.filterTemplateId())}
              placeholder="Chọn mẫu đã gửi"
              onChange={({ value }) => p.setFilterTemplate(value === null ? null : Number(value))}
              trigger={
                <View testID={t("seg-template-trigger")} class="h-9 flex-row items-center justify-between rounded-(--radius) border border-input bg-background px-3">
                  <Text class="truncate text-sm text-foreground">{templateName()}</Text>
                  <Text class="text-xs text-muted-foreground">▾</Text>
                </View>
              }
              itemClass="px-3 py-2 selected:bg-accent"
              renderItem={(entry) => (
                <View testID={t(`seg-template-item-${entry.value}`)}>
                  <Text class="text-sm text-foreground">{entry.title}</Text>
                </View>
              )} />
          </View>
        </If>
        <If when={props.zalo}>
          <Button testID={t("filter-verified")} label="Có Zalo" size="sm"
            variant={p.verifiedZalo() ? "primary" : "secondary"} onPress={p.toggleVerified} />
          <Button testID={t("filter-unreached")} label="Chưa nhắn" size="sm"
            variant={p.unreached() ? "primary" : "secondary"} onPress={p.toggleUnreached} />
        </If>
        <Button testID={t("filter-mobile")} label="Số di động" size="sm"
          variant={p.mobileOnly() ? "primary" : "secondary"} onPress={p.toggleMobile} />
      </View>

      <View class="flex-row items-center justify-between">
        <View class="flex-row items-center gap-2">
          <Button testID={t("select-all")} label="Chọn tất cả" size="sm" onPress={p.selectAll} />
          <Button testID={t("clear")} label="Bỏ chọn" size="sm" variant="ghost" onPress={p.clearChoice} />
        </View>
        <Text testID={t("count")} class="text-sm text-muted-foreground">
          {p.list.ready() ? `Đã chọn ${p.chosenCount()} / ${p.list.total()} lead` : "Đang mở dữ liệu…"}
        </Text>
      </View>

      <View class="flex-1 overflow-hidden rounded-(--radius) border border-border bg-card">
        <View class="flex-row border-b border-border bg-muted px-3 py-2">
          <View class="w-6" />
          <Each of={COLUMNS} key={(c) => c.label}>
            {(c) => (
              <View class={c().w}>
                <Text class="text-xs font-medium text-muted-foreground">{c().label}</Text>
              </View>
            )}
          </Each>
        </View>
        <View class="border-b border-border px-3 py-1.5">
          <Text testID={t("page-caption")} class="text-xs text-muted-foreground">
            {!p.list.ready() && !p.failed()
              ? "Đang mở dữ liệu…"
              : (p.failed() && !p.list.ready()
                ? "Chưa tải được danh sách"
                : `${p.list.items().length} / ${p.list.total()} lead`)}
          </Text>
        </View>
        <If when={!p.list.ready() && !p.failed()}>
          <View class="gap-2 p-3"><Skeleton /><Skeleton /></View>
        </If>
        <If when={p.list.ready() && p.list.total() === 0}>
          <EmptyState title="Không có lead phù hợp" hint="Thử bỏ bớt bộ lọc hoặc đổi từ khoá." />
        </If>
        <List testID={t("list")} each={p.list.items()} keyExtractor={(lead) => String(lead.id)}
          estimatedItemSize={40} onLoadMore={p.more} class="flex-1">
          {(row) => (
            <Base testID={t(`row-${row().id}`)} onPress={() => p.toggle(row().id)}
              class={`flex-row items-center border-b border-border px-3 py-2 hover:bg-accent ${p.isChosen(row().id) ? "bg-primary/5" : ""}`}>
              <View class="w-6">
                <Text class={`text-sm ${p.isChosen(row().id) ? "text-primary" : "text-muted-foreground"}`}>
                  {p.isChosen(row().id) ? "✓" : "○"}
                </Text>
              </View>
              <Text class="truncate flex-[2] pr-2 text-sm text-foreground">{row().name}</Text>
              <Text class="truncate flex-[2] pr-2 text-sm text-muted-foreground">{row().company ?? ""}</Text>
              <Text class="truncate flex-[2] pr-2 text-sm text-muted-foreground">
                {row().phone ?? row().email ?? "—"}
              </Text>
              <View class="w-28"><Badge label={row().status} tone={tone(row().status)} /></View>
            </Base>
          )}
        </List>
        <If when={p.hasMore()}>
          <View class="border-t border-border p-2">
            <Button testID={t("more")} label="Xem thêm" size="sm" variant="ghost" onPress={p.more} />
          </View>
        </If>
      </View>
    </View>
  );
}
