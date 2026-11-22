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

function textOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/**
 * Sorts a thrown error by what the person can do:
 * invalid_request is theirs to fix (the core's message says what), not_found means the list is out
 * of date (refresh it), and everything else (internal, unavailable, conflict, unknown) is retried.
 */
export function failure(e: unknown): Failure {
  const code = codeOf(e);
  if (code === "invalid_request") {
    return { code, action: "fix", message: `Dữ liệu chưa hợp lệ: ${textOf(e)}` };
  }
  if (code === "not_found") {
    return { code, action: "refresh", message: "Mục này không còn nữa, danh sách đã được làm mới." };
  }
  if (code === "conflict") {
    return { code, action: "refresh", message: "Dữ liệu vừa được thay đổi ở nơi khác, đã làm mới." };
  }
  if (code === "unavailable") {
    return { code, action: "retry", message: "Dịch vụ tạm thời chưa sẵn sàng. Thử lại sau ít giây." };
  }
  return { code, action: "retry", message: "Có lỗi khi xử lý. Thử lại nhé." };
}
