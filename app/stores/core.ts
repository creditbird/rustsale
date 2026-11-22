import { Host } from "rustsale/native";

/** A cursor page as the native module returns it. */
export interface Page<T> {
  items: T[];
  total: number;
  nextCursor: string | null;
}

/** What the person is asked to do about a failure. */
export type Action = "fix" | "refresh" | "retry";

export interface Failure {
  /** The native error code: invalid_request, not_found, conflict, unavailable, internal. */
  code: string;
  action: Action;
  /** A sentence for the person, never a stack or a SQL fragment. */
  message: string;
}

let started: Promise<void> | null = null;

/**
 * Starts the native core once (the v3-dev folder unless a test started it on another folder first:
 * `Host.start` answers the same for every later call). A failed start can be tried again.
 */
export function startCore(): Promise<void> {
  if (started === null) {
    started = Host.start()
      .then(() => undefined)
      .catch((e: unknown) => {
        started = null;
        throw e;
      });
  }
  return started;
}

/** The code of anything thrown: `e.code` for native errors, `internal` for the rest. */
export function codeOf(e: unknown): string {
  const code = (e as { code?: unknown } | null)?.code;
  return typeof code === "string" && code !== "" ? code : "internal";
}

/** The sentence for each native code. The core's own English message is never shown. */
export const MESSAGE_FOR_CODE: Record<string, string> = {
  invalid_request: "Dữ liệu chưa hợp lệ. Kiểm tra lại các ô đã nhập rồi thử lại.",
  not_found: "Mục này không còn nữa, danh sách đã được làm mới.",
  conflict: "Dữ liệu vừa được thay đổi ở nơi khác, đã làm mới.",
  unavailable: "Dịch vụ tạm thời chưa sẵn sàng. Thử lại sau ít giây.",
  internal: "Có lỗi khi xử lý. Thử lại nhé.",
};

/**
 * Sorts a thrown error by what the person can do:
 * invalid_request is theirs to fix, not_found and conflict mean the list is out of date (refresh it),
 * and everything else (internal, unavailable, unknown) is retried. `say` replaces the sentence of a
 * code for one action (for example, a send that has no connected channel).
 */
export function failure(e: unknown, say: Partial<Record<string, string>> = {}): Failure {
  const code = codeOf(e);
  const message = say[code] ?? MESSAGE_FOR_CODE[code] ?? MESSAGE_FOR_CODE.internal;
  if (code === "invalid_request") return { code, action: "fix", message };
  if (code === "not_found" || code === "conflict") return { code, action: "refresh", message };
  return { code, action: "retry", message };
}
