export type RuntimeCommand =
  | 'runtime_status'
  | 'ensure_runtime'
  | 'launch'
  | 'navigate'
  | 'evaluate'
  | 'screenshot'
  | 'health_check'
  | 'shutdown';

export type RuntimeErrorKind =
  | 'disconnected'
  | 'navigation_timeout'
  | 'evaluation_timeout'
  | 'transient_network'
  | 'captcha'
  | 'consent_required'
  | 'rate_limited'
  | 'selector_changed'
  | 'invalid_response'
  | 'browser_startup'
  | 'runtime_unavailable'
  | 'invalid_input'
  | 'cancelled';

export interface RuntimeRequest {
  id: string;
  command: RuntimeCommand;
  payload: Record<string, unknown>;
}

export interface RuntimeError {
  kind: RuntimeErrorKind;
  message: string;
  retryable: boolean;
}

export type RuntimeResponse =
  | { id: string; ok: true; result: unknown }
  | { id: string; ok: false; error: RuntimeError };

export interface LaunchPayload {
  profileDir: string;
  headless: boolean;
  startUrl?: string;
}

export interface NavigatePayload {
  url: string;
  timeoutMs?: number;
}

export interface EvaluatePayload {
  expression: string;
  timeoutMs?: number;
}

export interface ScreenshotPayload {
  path: string;
  fullPage?: boolean;
}

export class RuntimeFault extends Error {
  constructor(
    public readonly kind: RuntimeErrorKind,
    message: string,
    public readonly retryable: boolean,
  ) {
    super(message);
    this.name = 'RuntimeFault';
  }
}

export function parseRuntimeRequest(value: unknown): RuntimeRequest {
  if (!value || typeof value !== 'object') {
    throw new RuntimeFault('invalid_input', 'request must be a JSON object', false);
  }
  const request = value as Record<string, unknown>;
  if (typeof request.id !== 'string' || request.id.length === 0) {
    throw new RuntimeFault('invalid_input', 'request id must be a non-empty string', false);
  }
  if (typeof request.command !== 'string' || request.command.length === 0) {
    throw new RuntimeFault('invalid_input', 'request command must be a non-empty string', false);
  }
  if (request.payload !== undefined && (!request.payload || typeof request.payload !== 'object')) {
    throw new RuntimeFault('invalid_input', 'request payload must be an object', false);
  }
  return {
    id: request.id,
    command: request.command as RuntimeCommand,
    payload: (request.payload ?? {}) as Record<string, unknown>,
  };
}

export function runtimeError(error: unknown): RuntimeError {
  if (error instanceof RuntimeFault) {
    return {
      kind: error.kind,
      message: stripTerminalControlSequences(error.message),
      retryable: error.retryable,
    };
  }
  const message = stripTerminalControlSequences(
    error instanceof Error ? error.message : String(error),
  );
  const normalized = message.toLowerCase();
  if (normalized.includes('timeout')) {
    return { kind: 'transient_network', message, retryable: true };
  }
  if (normalized.includes('closed') || normalized.includes('disconnected')) {
    return { kind: 'disconnected', message, retryable: true };
  }
  return { kind: 'invalid_response', message, retryable: false };
}

function stripTerminalControlSequences(value: string): string {
  let clean = '';
  let index = 0;

  while (index < value.length) {
    const code = value.charCodeAt(index);
    if (code === 0x9b) {
      index = skipControlSequence(value, index + 1);
      continue;
    }
    if (code !== 0x1b) {
      clean += value[index];
      index += 1;
      continue;
    }

    const introducer = value.charCodeAt(index + 1);
    if (introducer === 0x5b) {
      index = skipControlSequence(value, index + 2);
      continue;
    }
    if (introducer === 0x5d) {
      index = skipOperatingSystemCommand(value, index + 2);
      continue;
    }
    index += Math.min(2, value.length - index);
  }

  return clean;
}

function skipControlSequence(value: string, start: number): number {
  let index = start;
  while (index < value.length) {
    const code = value.charCodeAt(index);
    index += 1;
    if (code >= 0x40 && code <= 0x7e) break;
  }
  return index;
}

function skipOperatingSystemCommand(value: string, start: number): number {
  let index = start;
  while (index < value.length) {
    const code = value.charCodeAt(index);
    if (code === 0x07) return index + 1;
    if (code === 0x1b && value.charCodeAt(index + 1) === 0x5c) return index + 2;
    index += 1;
  }
  return index;
}
