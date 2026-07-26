import { createInterface } from 'node:readline';
import { CloakRuntime } from './runtime';
import {
  parseRuntimeRequest,
  RuntimeFault,
  runtimeError,
  type EvaluatePayload,
  type LaunchPayload,
  type NavigatePayload,
  type RuntimeRequest,
  type RuntimeResponse,
  type ScreenshotPayload,
} from './protocol';

export { RuntimeFault } from './protocol';

export interface RuntimeAdapter {
  runtimeStatus(): Promise<unknown>;
  ensureRuntime(): Promise<unknown>;
  launch(payload: LaunchPayload): Promise<unknown>;
  navigate(payload: NavigatePayload): Promise<unknown>;
  evaluate(payload: EvaluatePayload): Promise<unknown>;
  screenshot(payload: ScreenshotPayload): Promise<unknown>;
  healthCheck(): Promise<unknown>;
  shutdown(): Promise<unknown>;
}

export async function dispatch(
  request: RuntimeRequest,
  runtime: RuntimeAdapter,
): Promise<RuntimeResponse> {
  try {
    let result: unknown;
    switch (request.command) {
      case 'runtime_status':
        result = await runtime.runtimeStatus();
        break;
      case 'ensure_runtime':
        result = await runtime.ensureRuntime();
        break;
      case 'launch':
        result = await runtime.launch(request.payload as unknown as LaunchPayload);
        break;
      case 'navigate':
        result = await runtime.navigate(request.payload as unknown as NavigatePayload);
        break;
      case 'evaluate':
        result = await runtime.evaluate(request.payload as unknown as EvaluatePayload);
        break;
      case 'screenshot':
        result = await runtime.screenshot(request.payload as unknown as ScreenshotPayload);
        break;
      case 'health_check':
        result = await runtime.healthCheck();
        break;
      case 'shutdown':
        result = await runtime.shutdown();
        break;
      default:
        throw new RuntimeFault(
          'invalid_input',
          `unsupported runtime command: ${String(request.command)}`,
          false,
        );
    }
    return { id: request.id, ok: true, result };
  } catch (error) {
    return { id: request.id, ok: false, error: runtimeError(error) };
  }
}

export async function runSidecar(runtime: RuntimeAdapter = new CloakRuntime()): Promise<void> {
  const lines = createInterface({ input: process.stdin, crlfDelay: Number.POSITIVE_INFINITY });
  const shutdown = async () => {
    await runtime.shutdown().catch((error) => console.error('[cloak-runtime] shutdown failed', error));
  };
  process.once('SIGINT', () => void shutdown().finally(() => process.exit(130)));
  process.once('SIGTERM', () => void shutdown().finally(() => process.exit(143)));

  for await (const line of lines) {
    if (!line.trim()) continue;
    let id = 'unknown';
    let response: RuntimeResponse;
    try {
      const raw = JSON.parse(line) as unknown;
      if (raw && typeof raw === 'object' && typeof (raw as { id?: unknown }).id === 'string') {
        id = (raw as { id: string }).id;
      }
      response = await dispatch(parseRuntimeRequest(raw), runtime);
    } catch (error) {
      response = { id, ok: false, error: runtimeError(error) };
    }
    process.stdout.write(`${JSON.stringify(response)}\n`);
  }
  await shutdown();
}

if (import.meta.main) {
  void runSidecar().catch((error) => {
    console.error('[cloak-runtime] fatal error', error);
    process.exitCode = 1;
  });
}
