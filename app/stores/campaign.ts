import { signal } from "nsh";
import { Host } from "rustsale/native";

export interface LogLine {
  id: number;
  text: string;
}

interface ProgressEvent { progress?: number; status?: string; message?: string }
interface LogEvent { message?: string }
interface CompleteEvent { total_sent?: number; message?: string }

const MAX_LOGS = 50;

/**
 * What a running outreach campaign shows. It lives outside the screen, as v2's tab did: leaving the
 * screen must not lose the progress, and coming back must not allow a second run on top of the first.
 */
function createCampaign(prefix: string, unit: string) {
  const starting = signal(false);
  const progress = signal(0);
  const status = signal("");
  const logs = signal<LogLine[]>([]);
  let seq = 0;
  const doneListeners = new Set<() => void>();
  let listening = false;

  const appendLog = (text: string) => {
    seq += 1;
    const next = logs().concat([{ id: seq, text }]);
    logs.set(next.length > MAX_LOGS ? next.slice(next.length - MAX_LOGS) : next);
  };

  /** Subscribes to the core's events once for the life of the app (called at the bottom of this file). */
  const listen = () => {
    if (listening) return;
    listening = true;
    Host.onEvent((name, payload) => {
      if (name === `${prefix}-progress`) {
        const event = payload as ProgressEvent;
        if (typeof event.progress === "number") progress.set(event.progress);
        if (typeof event.message === "string" && event.message !== "") {
          status.set(event.message);
          appendLog(event.message);
        }
        if (event.status === "failed") starting.set(false);
      } else if (name === `${prefix}-log`) {
        const event = payload as LogEvent;
        if (typeof event.message === "string" && event.message !== "") appendLog(event.message);
      } else if (name === `${prefix}-complete`) {
        const event = payload as CompleteEvent;
        const sent = typeof event.total_sent === "number" ? event.total_sent : 0;
        progress.set(100);
        starting.set(false);
        status.set(typeof event.message === "string" && event.message !== "" ? event.message : `Đã gửi ${sent} ${unit}.`);
        appendLog(typeof event.message === "string" ? event.message : `Hoàn tất: ${sent} ${unit}.`);
        for (const fn of Array.from(doneListeners)) fn();
      }
    });
  };

  return {
    starting, progress, status, logs, appendLog, listen,
    /** A run is about to start: clear the last one. */
    begin: () => {
      starting.set(true);
      progress.set(0);
      status.set("Đang chuẩn bị…");
      logs.set([]);
    },
    /** The run did not start (or the core refused it). */
    abort: () => {
      starting.set(false);
      status.set("");
    },
    /** Calls `fn` when a run completes; returns what to call to stop. */
    onDone: (fn: () => void) => {
      doneListeners.add(fn);
      return () => { doneListeners.delete(fn); };
    },
  };
}

export const zaloCampaign = createCampaign("outreach", "tin");
export const emailCampaign = createCampaign("email-outreach", "email");
// At module level, outside any screen, so no screen's cleanup can take the subscription down.
zaloCampaign.listen();
emailCampaign.listen();
