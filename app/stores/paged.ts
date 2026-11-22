import { signal } from "nsh";
import type { Page } from "./core";

/**
 * A cursor-paged list that never blocks the UI thread (every fetch is a native promise) and drops
 * the answer of a query that a newer one replaced. `reload` starts over, `more` appends the next page.
 */
export function pager<T>(
  fetchPage: (cursor: string | null) => Promise<Page<T>>,
  onError: (e: unknown, retry: () => void) => void,
) {
  const items = signal<T[]>([]);
  const total = signal(0);
  const ready = signal(false);
  const loading = signal(false);
  let generation = 0;
  let next: string | null = null;
  let busy = false;

  const reload = async (): Promise<void> => {
    generation += 1;
    const mine = generation;
    busy = true;
    loading.set(true);
    try {
      const page = await fetchPage(null);
      if (mine !== generation) return;
      items.set(page.items);
      total.set(page.total);
      next = page.nextCursor;
      ready.set(true);
    } catch (e) {
      if (mine === generation) onError(e, () => { void reload(); });
    } finally {
      if (mine === generation) {
        busy = false;
        loading.set(false);
      }
    }
  };

  const more = async (): Promise<void> => {
    if (busy || next === null) return;
    const mine = generation;
    const cursor = next;
    busy = true;
    try {
      const page = await fetchPage(cursor);
      if (mine !== generation) return;
      items.set(items().concat(page.items));
      total.set(page.total);
      next = page.nextCursor;
    } catch (e) {
      if (mine === generation) onError(e, () => { void more(); });
    } finally {
      if (mine === generation) busy = false;
    }
  };

  return { items, total, ready, loading, reload, more };
}
