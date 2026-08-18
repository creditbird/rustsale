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
    file: join(playwrightCoreRoot, 'lib/server/transport.js'),
    target: `this._ws = new import_utilsBundle.ws(url, [], {
      maxPayload: 256 * 1024 * 1024,
      // 256Mb,
      headers: options.headers,
      followRedirects: options.followRedirects,
      agent: /^(https|wss):\\/\\//.test(url) ? import_happyEyeballs.httpsHappyEyeballsAgent : import_happyEyeballs.httpHappyEyeballsAgent,
      perMessageDeflate
    });`,
    replacement: `if (typeof Bun !== "undefined" && typeof globalThis.WebSocket !== "undefined") {
      const EventEmitter = require("events");
      class BunWSAdapter extends EventEmitter {
        constructor(u, p) {
          super();
          this.url = u;
          this.readyState = 0;
          const ws = new globalThis.WebSocket(u, p);
          this._ws = ws;
          ws.onopen = () => { this.readyState = 1; this.emit("open"); };
          ws.onmessage = (event) => { this.emit("message", event); };
          ws.onerror = (error) => { this.emit("error", error); };
          ws.onclose = (event) => { this.readyState = 3; this.emit("close", event.code, event.reason); };
        }
        send(data, cb) {
          try { this._ws.send(data); cb?.(); } catch (err) { cb?.(err); }
        }
        close(code, reason) { this._ws.close(code, reason); }
        addEventListener(type, listener) { this.on(type, listener); }
        removeEventListener(type, listener) { this.off(type, listener); }
      }
      this._ws = new BunWSAdapter(url, []);
    } else {
      this._ws = new import_utilsBundle.ws(url, [], {
        maxPayload: 256 * 1024 * 1024,
        headers: options.headers,
        followRedirects: options.followRedirects,
        agent: /^(https|wss):\\/\\//.test(url) ? import_happyEyeballs.httpsHappyEyeballsAgent : import_happyEyeballs.httpHappyEyeballsAgent,
        perMessageDeflate
      });
    }`,
    description: 'WebSocketTransport Bun native WebSocket adapter',
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
