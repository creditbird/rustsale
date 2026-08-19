import { copyFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, basename } from 'node:path';
import { createInterface } from 'node:readline';
import { Readable } from 'node:stream';

const binaryPath = process.argv[2] || process.env.SIDECAR_PATH;
if (!binaryPath) {
  console.error('[clean-room-smoke] Usage: bun run scripts/smoke-test-compiled-sidecar.ts <path-to-binary>');
  process.exit(1);
}

const resolvedBinaryPath = resolve(binaryPath);
if (!existsSync(resolvedBinaryPath)) {
  console.error(`[clean-room-smoke] FATAL: Binary not found at: ${resolvedBinaryPath}`);
  process.exit(1);
}

console.log(`[clean-room-smoke] Starting clean-room smoke test on compiled binary: ${resolvedBinaryPath}`);

const isolatedRoot = mkdtempSync(join(tmpdir(), 'rustsale-clean-room-sidecar-'));
const binaryFileName = basename(resolvedBinaryPath);
const isolatedBinaryPath = join(isolatedRoot, binaryFileName);
const isolatedCacheDir = join(isolatedRoot, 'cache');
const isolatedProfileDir = join(isolatedRoot, 'profile');

copyFileSync(resolvedBinaryPath, isolatedBinaryPath);

// On Unix, ensure executable bit is set
if (process.platform !== 'win32') {
  Bun.spawnSync(['chmod', '+x', isolatedBinaryPath]);
}

// Verification: Assert no node_modules or package.json exist anywhere in isolatedRoot
if (existsSync(join(isolatedRoot, 'node_modules')) || existsSync(join(isolatedRoot, 'package.json'))) {
  console.error('[clean-room-smoke] FATAL: Clean-room isolation contaminated with node_modules or package.json');
  process.exit(1);
}

const server = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  fetch: () =>
    new Response('<!doctype html><title>RustSale Clean Room</title><div role="feed">Clean Room OK</div>', {
      headers: { 'content-type': 'text/html; charset=utf-8' },
    }),
});

const serverUrl = `http://127.0.0.1:${server.port}/test-feed`;

console.log(`[clean-room-smoke] Running isolated binary in ${isolatedRoot} without node_modules...`);

const proc = Bun.spawn([isolatedBinaryPath], {
  cwd: isolatedRoot,
  stdin: 'pipe',
  stdout: 'pipe',
  stderr: 'inherit',
  env: {
    ...process.env,
    CLOAKBROWSER_CACHE_DIR: isolatedCacheDir,
  },
});

const pending = new Map<string, { resolve: (res: any) => void; reject: (err: any) => void }>();

const nodeReadable = Readable.fromWeb(proc.stdout as any);
const rl = createInterface({
  input: nodeReadable,
  crlfDelay: Infinity,
});

rl.on('line', (line) => {
  if (!line.trim()) return;
  try {
    const parsed = JSON.parse(line);
    const deferred = pending.get(parsed.id);
    if (deferred) {
      pending.delete(parsed.id);
      if (parsed.ok) {
        deferred.resolve(parsed.result);
      } else {
        deferred.reject(new Error(`[${parsed.error?.kind || 'unknown'}] ${parsed.error?.message || 'Unknown error'}`));
      }
    }
  } catch (err) {
    console.error(`[clean-room-smoke] Failed to parse stdout line: ${line}`, err);
  }
});

function sendCommand(id: string, command: string, payload: Record<string, unknown> = {}, timeoutMs = 30_000): Promise<any> {
  return new Promise((res, rej) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      rej(new Error(`[clean-room-smoke] Command "${command}" (id=${id}) timed out after ${timeoutMs}ms`));
    }, timeoutMs);

    pending.set(id, {
      resolve: (val) => {
        clearTimeout(timer);
        res(val);
      },
      reject: (err) => {
        clearTimeout(timer);
        rej(err);
      },
    });
    const message = JSON.stringify({ id, command, payload }) + '\n';
    (proc.stdin as any).write(message);
    (proc.stdin as any).flush();
  });
}

async function run() {
  try {
    console.log('[clean-room-smoke] 1. Requesting runtime_status...');
    const status = await sendCommand('smoke-1', 'runtime_status');
    console.log('[clean-room-smoke] runtime_status OK:', status.version || status.bundledVersion);

    console.log('[clean-room-smoke] 2. Requesting ensure_runtime...');
    const runtimeInfo = await sendCommand('smoke-2', 'ensure_runtime');
    console.log('[clean-room-smoke] ensure_runtime OK: binaryPath =', runtimeInfo.binaryPath);

    console.log('[clean-room-smoke] 3. Launching browser in isolated clean-room profile...');
    const launchRes = await sendCommand('smoke-3', 'launch', {
      profileDir: isolatedProfileDir,
      headless: true,
      startUrl: serverUrl,
    });
    console.log('[clean-room-smoke] launch OK:', launchRes);

    console.log('[clean-room-smoke] 4. Evaluating script on page in clean-room...');
    const evalRes = await sendCommand('smoke-4', 'evaluate', {
      expression: 'document.querySelector(\'[role="feed"]\')?.textContent',
    });
    if (evalRes.value !== 'Clean Room OK') {
      throw new Error(`Unexpected evaluate result: expected "Clean Room OK", got "${evalRes.value}"`);
    }
    console.log('[clean-room-smoke] evaluate OK: value =', evalRes.value);

    console.log('[clean-room-smoke] 5. Checking health_check in clean-room...');
    const health = await sendCommand('smoke-5', 'health_check');
    if (health.state !== 'ready') {
      throw new Error(`Unexpected health check state: expected "ready", got "${health.state}"`);
    }
    console.log('[clean-room-smoke] health_check OK: state =', health.state);

    console.log('[clean-room-smoke] 6. Shutting down browser in clean-room...');
    const shutdownRes = await sendCommand('smoke-6', 'shutdown');
    console.log('[clean-room-smoke] shutdown OK:', shutdownRes);

    console.log('[clean-room-smoke] SUCCESS: Clean-room smoke test passed completely without node_modules!');
  } catch (error) {
    console.error('[clean-room-smoke] FATAL ERROR:', error);
    process.exitCode = 1;
  } finally {
    try {
      proc.kill();
    } catch {}
    server.stop(true);
    try {
      rmSync(isolatedRoot, { recursive: true, force: true });
    } catch {}
    process.exit(process.exitCode || 0);
  }
}

run();
