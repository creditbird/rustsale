import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';

const playwrightCoreRoot = resolve('node_modules/playwright-core');
const packageJsonPath = join(playwrightCoreRoot, 'package.json');

if (!existsSync(packageJsonPath)) {
  console.error(`[patch-playwright] FATAL: Required file not found: ${packageJsonPath}`);
  process.exit(1);
}

let version = '';
try {
  const pkg = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
  if (!pkg.version || typeof pkg.version !== 'string') {
    throw new Error('package.json does not specify a valid version string');
  }
  version = pkg.version;
} catch (error) {
  console.error(`[patch-playwright] FATAL: Could not parse ${packageJsonPath}: ${(error as Error).message}`);
  process.exit(1);
}

console.log(`[patch-playwright] Patching playwright-core v${version} for Bun standalone compilation (fail-closed)...`);

const patches: Array<{ file: string; target: string | RegExp; replacement: string; description: string }> = [
  {
    file: join(playwrightCoreRoot, 'lib/server/utils/nodePlatform.js'),
    target: 'const coreDir = import_path.default.dirname(require.resolve("../../../package.json"));',
    replacement: 'const coreDir = import_path.default.resolve(__dirname, "..", "..", "..");',
    description: 'nodePlatform coreDir relative resolution',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/utils/userAgent.js'),
    target: 'const version = process.env.PW_VERSION_OVERRIDE || require("./../../../package.json").version;',
    replacement: `const version = process.env.PW_VERSION_OVERRIDE || "${version}";`,
    description: 'userAgent hardcoded version fallback',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/registry/dependencies.js'),
    target: 'const languageBindingVersion = process.env.PW_CLI_DISPLAY_VERSION || require("../../../package.json").version;',
    replacement: `const languageBindingVersion = process.env.PW_CLI_DISPLAY_VERSION || "${version}";`,
    description: 'dependencies languageBindingVersion hardcoded version',
  },
  {
    file: join(playwrightCoreRoot, 'lib/cli/program.js'),
    target: 'const packageJSON = require("../../package.json");',
    replacement: `const packageJSON = { version: "${version}", name: "playwright-core" };`,
    description: 'cli program packageJSON mock',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/browserType.js'),
    target: 'supportsPipeTransport() {\n    return true;\n  }',
    replacement: 'supportsPipeTransport() {\n    return process.platform !== "win32";\n  }',
    description: 'browserType Windows pipe transport bypass',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/chromium/chromium.js'),
    target: 'if (options.cdpPort !== void 0)\n      chromeArguments.push(`--remote-debugging-port=${options.cdpPort}`);\n    else\n      chromeArguments.push("--remote-debugging-pipe");',
    replacement: 'if (options.cdpPort !== void 0 || process.platform === "win32")\n      chromeArguments.push(`--remote-debugging-port=${options.cdpPort ?? 0}`);\n    else\n      chromeArguments.push("--remote-debugging-pipe");',
    description: 'chromium defaultArgs Windows remote-debugging-port fallback',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/chromium/chromium.js'),
    target: 'async function waitForReadyState(options, browserLogsCollector) {\n  if (options.cdpPort === void 0 && !options.args?.some((a) => a.startsWith("--remote-debugging-port")))',
    replacement: 'async function waitForReadyState(options, browserLogsCollector) {\n  if (options.cdpPort === void 0 && process.platform !== "win32" && !options.args?.some((a) => a.startsWith("--remote-debugging-port")))',
    description: 'chromium waitForReadyState Windows port listener',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/transport.js'),
    target: /(?:class BunNativeWebSocket\s*\{[\s\S]*?\n\}\s*\n)?class WebSocketTransport\s*\{/,
    replacement: `class BunNativeWebSocket {
  constructor(url, protocols, options) {
    this._events = { open: [], message: [], close: [], error: [], "unexpected-response": [], upgrade: [] };
    const wsOptions = options?.headers ? { headers: options.headers } : undefined;
    this._ws = new globalThis.WebSocket(url, wsOptions);
    this._ws.onopen = () => this._emit("open", {});
    this._ws.onmessage = (event) => this._emit("message", { data: event.data });
    this._ws.onclose = (event) => this._emit("close", { code: event.code || 1000, reason: event.reason || "" });
    this._ws.onerror = (event) => this._emit("error", event instanceof Error ? event : new Error(event?.message || "WebSocket connection error"));
  }
  on(event, handler) {
    if (this._events[event]) this._events[event].push(handler);
    if (event === "open" && this._ws.readyState === 1) {
      queueMicrotask(() => handler({}));
    }
    return this;
  }
  once(event, handler) {
    if (event === "open" && this._ws.readyState === 1) {
      queueMicrotask(() => handler({}));
      return this;
    }
    const wrapper = (...args) => {
      this.removeListener(event, wrapper);
      handler(...args);
    };
    return this.on(event, wrapper);
  }
  addEventListener(event, handler) {
    return this.on(event, handler);
  }
  removeListener(event, handler) {
    if (this._events[event]) {
      this._events[event] = this._events[event].filter(h => h !== handler);
    }
    return this;
  }
  _emit(event, ...args) {
    const list = this._events[event];
    if (list) {
      for (const handler of [...list]) {
        handler(...args);
      }
    }
  }
  send(data, cb) {
    try {
      this._ws.send(data);
      cb?.();
    } catch (err) {
      if (cb) cb(err);
      else throw err;
    }
  }
  close(code, reason) {
    this._ws.close(code, reason);
  }
  get readyState() {
    return this._ws.readyState;
  }
}
class WebSocketTransport {`,
    description: 'transport Bun native WebSocket class injection',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/transport.js'),
    target: /this\._ws\s*=\s*(?:\(typeof Bun !== "undefined"[\s\S]*?:\s*)?new import_utilsBundle\.ws\(url,\s*\[\],\s*\{[\s\S]*?\}\);/,
    replacement: `this._ws = (typeof Bun !== "undefined" && typeof globalThis.WebSocket !== "undefined")
      ? new BunNativeWebSocket(url, [], options)
      : new import_utilsBundle.ws(url, [], {
          maxPayload: 256 * 1024 * 1024,
          // 256Mb,
          headers: options.headers,
          followRedirects: options.followRedirects,
          agent: /^(https|wss):\\/\\//.test(url) ? import_happyEyeballs.httpsHappyEyeballsAgent : import_happyEyeballs.httpHappyEyeballsAgent,
          perMessageDeflate
        });`,
    description: 'transport Bun native WebSocket instantiation',
  },
  {
    file: join(playwrightCoreRoot, 'lib/server/transport.js'),
    target: 'if (this._ws.readyState === import_utilsBundle.ws.CLOSED)',
    replacement: 'if (this._ws.readyState === import_utilsBundle.ws.CLOSED || this._ws.readyState === 3)',
    description: 'transport WebSocket close state check',
  },
];

for (const patch of patches) {
  if (!existsSync(patch.file)) {
    console.error(`[patch-playwright] FATAL: Required source file missing: ${patch.file}`);
    process.exit(1);
  }

  const content = readFileSync(patch.file, 'utf8');
  if (content.includes(patch.replacement)) {
    console.log(`[patch-playwright] [VERIFIED] Already applied: ${patch.description} (${patch.file})`);
    continue;
  }

  let matched = false;
  if (typeof patch.target === 'string') {
    if (content.includes(patch.target)) {
      matched = true;
    }
  } else if (patch.target.test(content)) {
    matched = true;
  }

  if (!matched) {
    console.error(
      `[patch-playwright] FATAL: Target pattern for "${patch.description}" not found in ${patch.file}. Upstream code layout may have changed!`,
    );
    process.exit(1);
  }

  const updated = content.replace(patch.target, patch.replacement);
  if (!updated.includes(patch.replacement)) {
    console.error(`[patch-playwright] FATAL: Replacement failed to apply for "${patch.description}" in ${patch.file}`);
    process.exit(1);
  }

  writeFileSync(patch.file, updated, 'utf8');
  console.log(`[patch-playwright] [APPLIED] Successfully patched: ${patch.description} (${patch.file})`);
}

// Verification: scan playwright-core lib for any remaining unsafe package.json resolutions
function findUnsafePackageResolutions(dir: string): string[] {
  const unsafe: string[] = [];
  if (!statSync(dir, { throwIfNoEntry: false })?.isDirectory()) return unsafe;

  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      unsafe.push(...findUnsafePackageResolutions(fullPath));
    } else if (entry.isFile() && (entry.name.endsWith('.js') || entry.name.endsWith('.mjs'))) {
      const content = readFileSync(fullPath, 'utf8');
      if (
        content.includes('require.resolve("../../../package.json")') ||
        content.includes('require("./../../../package.json")') ||
        content.includes('require("../../../package.json")') ||
        content.includes('require("../../package.json")')
      ) {
        unsafe.push(fullPath);
      }
    }
  }
  return unsafe;
}

const remainingUnsafe = findUnsafePackageResolutions(join(playwrightCoreRoot, 'lib'));
if (remainingUnsafe.length > 0) {
  console.error(
    `[patch-playwright] FATAL: Unsafe Playwright package.json resolution remains in:\n${remainingUnsafe.join('\n')}`,
  );
  process.exit(1);
}

console.log('[patch-playwright] Verification passed: All required patches verified and no unsafe package.json resolutions remain.');
