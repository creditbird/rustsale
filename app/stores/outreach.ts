import { computed, signal } from "nsh";
import { Leads, LeadsMore } from "rustsale/native";
import { startCore, type Page } from "./core";
import { pager } from "./paged";
import { report } from "./toast";

/** A lead as the outreach pages return it (the core's `Lead`, snake_case). */
export interface OutreachLead {
  id: number;
  name: string;
  company: string | null;
  status: string;
  email: string | null;
  phone: string | null;
  has_zalo: number | null;
  zalo_status: string | null;
  zalo_reached: number | null;
}

export type Segmentation = "all" | "never_sent" | "last_failed" | "template_success";

export const SEGMENTATIONS: { key: Segmentation; label: string }[] = [
  { key: "all", label: "Tất cả" },
  { key: "never_sent", label: "Chưa gửi" },
  { key: "last_failed", label: "Gửi lỗi gần nhất" },
  { key: "template_success", label: "Khớp mẫu đã gửi" },
];

const PAGE_SIZE = 50;

/**
 * The lead picker the Zalo and Email screens share: filters and cursor paging in the core, plus
 * "select every match, then exclude some" or an explicit pick. `resolve` turns the choice into the
 * ids a campaign sends to.
 */
export function useOutreachLeads(channel: "zalo" | "email") {
  const search = signal("");
  const segmentation = signal<Segmentation>("all");
  const categoryId = signal<number | null>(null);
  const filterTemplateId = signal<number | null>(null);
  const categories = signal<{ id: number; name: string }[]>([]);
  const verifiedZalo = signal(false);
  const unreached = signal(false);
  const mobileOnly = signal(false);
  const allMatch = signal(true);
  const excluded = signal<number[]>([]);
  const picked = signal<number[]>([]);
  const failed = signal(false);

  const filters = (): Record<string, unknown> => ({
    categoryId: categoryId() ?? undefined,
    segmentation: segmentation(),
    filterTemplateId: filterTemplateId() ?? undefined,
    channel,
    search: search().trim() || undefined,
    filterVerifiedZalo: verifiedZalo(),
    filterUnreached: unreached(),
    filterMobileOnly: mobileOnly(),
  });

  const list = pager<OutreachLead, number>(
    async (cursor) => {
      await startCore();
      const page = (await LeadsMore.outreachPage(filters(), cursor, PAGE_SIZE)) as {
        items: OutreachLead[];
        total: number;
        next_cursor: number | null;
      };
      return { items: page.items, total: page.total, nextCursor: page.next_cursor };
    },
    (e, retry) => { failed.set(true); report(e, { retry }); },
  );

  const resetChoice = () => {
    allMatch.set(true);
    excluded.set([]);
    picked.set([]);
  };
  const reload = () => {
    failed.set(false);
    void list.reload();
  };
  /** A filter change starts over: a new match set has a new selection, and no answer can land mid-pick. */
  const refilter = () => {
    resetChoice();
    reload();
  };

  let timer: ReturnType<typeof setTimeout> | null = null;
  const setSearch = (value: string) => {
    search.set(value);
    resetChoice();
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(reload, 200);
  };

  const setSegmentation = (value: Segmentation) => {
    if (segmentation() === value) return;
    segmentation.set(value);
    refilter();
  };
  const setCategory = (value: number | null) => {
    if (categoryId() === value) return;
    categoryId.set(value);
    refilter();
  };
  const setFilterTemplate = (value: number | null) => {
    if (filterTemplateId() === value) return;
    filterTemplateId.set(value);
    refilter();
  };
  /** Chooses `template_success` and its template together, so the core never sees one without the other. */
  const setSegmentationAndTemplate = (value: Segmentation, templateId: number | null) => {
    segmentation.set(value);
    filterTemplateId.set(templateId);
    refilter();
  };

  const loadCategories = async () => {
    try {
      await startCore();
      categories.set((await Leads.categories()) as { id: number; name: string }[]);
    } catch (e) {
      report(e, { retry: () => { void loadCategories(); } });
    }
  };
  const toggleVerified = () => { verifiedZalo.set(!verifiedZalo()); refilter(); };
  const toggleUnreached = () => { unreached.set(!unreached()); refilter(); };
  const toggleMobile = () => { mobileOnly.set(!mobileOnly()); refilter(); };

  /** How many leads the run would send to. */
  const chosenCount = computed(() => {
    if (allMatch()) return Math.max(0, list.total() - excluded().length);
    return picked().length;
  });
  const isChosen = (id: number): boolean =>
    allMatch() ? !excluded().includes(id) : picked().includes(id);

  const toggle = (id: number) => {
    if (allMatch()) {
      const current = excluded();
      excluded.set(current.includes(id) ? current.filter((x) => x !== id) : current.concat([id]));
    } else {
      const current = picked();
      picked.set(current.includes(id) ? current.filter((x) => x !== id) : current.concat([id]));
    }
  };
  const selectAll = () => {
    allMatch.set(true);
    excluded.set([]);
  };
  const clearChoice = () => {
    allMatch.set(false);
    excluded.set([]);
    picked.set([]);
  };

  /** The ids to send to: every match without the exclusions, or the picks. */
  const resolve = async (): Promise<number[]> => {
    if (!allMatch()) return picked();
    await startCore();
    return (await LeadsMore.outreachIds(filters(), excluded())) as number[];
  };

  const hasMore = computed(() => list.ready() && list.items().length < list.total());

  void list.reload();
  void loadCategories();

  return {
    search, segmentation, categoryId, filterTemplateId, categories,
    verifiedZalo, unreached, mobileOnly,
    list, failed, allMatch, chosenCount, isChosen,
    setSearch, setSegmentation, setCategory, setFilterTemplate, setSegmentationAndTemplate,
    toggleVerified, toggleUnreached, toggleMobile,
    toggle, selectAll, clearChoice, resolve, reload,
    more: () => { void list.more(); }, hasMore,
  };
}

export type OutreachPicker = ReturnType<typeof useOutreachLeads>;
