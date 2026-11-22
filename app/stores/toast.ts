import { signal } from "nsh";
import { failure, type Failure } from "./core";

export type ToastTone = "info" | "success" | "danger" | "warning";

export interface ToastMessage {
  id: number;
  text: string;
  tone: ToastTone;
  /** A retry offered next to the text. */
  retry?: () => void;
}

/** The toast on show, if any. One at a time: a new one replaces the old. */
export const toast = signal<ToastMessage | null>(null);

let seq = 0;
let timer: ReturnType<typeof setTimeout> | null = null;

export function dismiss(): void {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  toast.set(null);
}

/** Shows `text` for a few seconds (longer when a retry is offered). */
export function notify(text: string, tone: ToastTone = "info", retry?: () => void): void {
  dismiss();
  seq += 1;
  toast.set({ id: seq, text, tone, retry });
  timer = setTimeout(dismiss, retry ? 8000 : 4000);
}

/**
 * Tells the person about a native failure the way its code asks for:
 * `fix` shows the core's message, `refresh` runs `refresh` and says the data moved,
 * `retry` offers the retry. Resolves the sorted failure.
 */
export function report(e: unknown, handlers: { refresh?: () => void; retry?: () => void; say?: Partial<Record<string, string>> } = {}): Failure {
  const f = failure(e, handlers.say);
  if (f.action === "refresh") {
    notify(f.message, "warning");
    handlers.refresh?.();
  } else if (f.action === "retry") {
    notify(f.message, "danger", handlers.retry);
  } else {
    notify(f.message, "danger");
  }
  return f;
}
