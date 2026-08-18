import { readdirSync, readFileSync } from 'node:fs';
import { spawn, type ChildProcess } from 'node:child_process';
import * as cloak from 'cloakbrowser';
import type { BinaryInfo } from 'cloakbrowser';
import * as playwright from 'playwright-core';
import type { BrowserContext, Page } from 'playwright-core';
import { inspectGoogleMapsPage } from './googleMaps';
import {
  RuntimeFault,
  type EvaluatePayload,
  type LaunchPayload,
  type NavigatePayload,
  type ScreenshotPayload,
} from './protocol';

export const PINNED_CLOAKBROWSER_WRAPPER_VERSION = '0.5.1';

const PINNED_CLOAKBROWSER_BINARY_VERSIONS: Readonly<Record<string, string>> = {
  'linux-x64': '146.0.7680.177.5',
  'linux-arm64': '146.0.7680.177.3',
  'darwin-x64': '145.0.7632.109.2',
  'darwin-arm64': '145.0.7632.109.2',
  'win32-x64': '146.0.7680.177.5',
};

export type BrowserTransport = 'playwright-pipe' | 'cdp-websocket';

export function preferredBrowserTransport(platform: string = process.platform): BrowserTransport {
  const override = process.env.RUSTSALE_BROWSER_TRANSPORT?.trim().toLowerCase();
  if (override === 'cdp-websocket' || override === 'websocket' || override === 'tcp') {
    return 'cdp-websocket';
  }
  if (override === 'playwright-pipe' || override === 'pipe') {
    return 'playwright-pipe';
  }
  if (platform === 'win32') {
    return 'cdp-websocket';
  }
  return 'playwright-pipe';
}

export function parseDevToolsWsEndpoint(logLine: string): string | null {
  const match = logLine.match(/DevTools listening on (ws:\/\/[^\s]+)/);
  if (!match) return null;
  const endpoint = match[1];
  const url = new URL(endpoint);
  if (url.hostname !== '127.0.0.1' && url.hostname !== 'localhost') {
    throw new RuntimeFault('browser_startup', `Refusing non-loopback DevTools endpoint: ${endpoint}`, false);
  }
  return endpoint;
}

export function buildWindowsChromiumArgs(options: {
  profileDir: string;
  headless: boolean;
  cdpPort?: number;
  extraArgs?: string[];
}): string[] {
  const cdpPort = options.cdpPort ?? 0;
  const baseArgs = [
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${options.profileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-background-timer-throttling',
    '--disable-backgrounding-occluded-windows',
    '--disable-breakpad',
    '--disable-client-side-phishing-detection',
    '--disable-component-update',
    '--disable-default-apps',
    '--disable-dev-shm-usage',
    '--disable-extensions',
    '--disable-hang-monitor',
    '--disable-ipc-flooding-protection',
    '--disable-popup-blocking',
    '--disable-prompt-on-repost',
    '--disable-renderer-backgrounding',
    '--disable-sync',
    '--force-color-profile=srgb',
    '--metrics-recording-only',
    '--password-store=basic',
    '--use-mock-keychain',
    '--no-sandbox',
    '--ignore-gpu-blocklist',
  ];

  const stealthArgs = cloak.getDefaultStealthArgs();
  for (const arg of stealthArgs) {
    if (!baseArgs.some((existing) => existing.split('=')[0] === arg.split('=')[0])) {
      baseArgs.push(arg);
    }
  }
  if (options.extraArgs) {
    for (const arg of options.extraArgs) {
      if (!baseArgs.some((existing) => existing.split('=')[0] === arg.split('=')[0])) {
        baseArgs.push(arg);
      }
    }
  }

  if (options.headless) {
    baseArgs.push(
      '--headless',
      '--hide-scrollbars',
      '--mute-audio',
      '--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4',
    );
  }

  baseArgs.push('about:blank');
  return baseArgs;
}

export function pinnedCloakBrowserBinaryVersion(
  platform: string = process.platform,
  arch: string = process.arch,
): string {
  const key = `${platform}-${arch}`;
  const version = PINNED_CLOAKBROWSER_BINARY_VERSIONS[key];
  if (!version) {
    throw new Error(`unsupported CloakBrowser platform: ${platform} ${arch}`);
  }
  return version;
}

export const PINNED_CLOAKBROWSER_BINARY_VERSION = pinnedCloakBrowserBinaryVersion();

export interface CloakRuntimeOptions {
  cacheDir?: string;
}

export interface PersistentLaunchOptions {
  userDataDir: string;
  headless: boolean;
  browserVersion: string;
}

export function persistentLaunchOptions(payload: LaunchPayload): PersistentLaunchOptions {
  if (!payload.profileDir) {
    throw new RuntimeFault('invalid_input', 'profileDir is required', false);
  }
  return {
    userDataDir: payload.profileDir,
    headless: payload.headless,
    browserVersion: PINNED_CLOAKBROWSER_BINARY_VERSION,
  };
}

export class CloakRuntime {
  private browser: playwright.Browser | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
  private browserProcess: ChildProcess | null = null;
  private profileDir: string | null = null;

  constructor(options: CloakRuntimeOptions = {}) {
    configureCloakBrowserEnvironment(options);
  }

  async runtimeStatus(): Promise<BinaryInfo & { wrapperVersion: string }> {
    return {
      ...cloak.binaryInfo(PINNED_CLOAKBROWSER_BINARY_VERSION),
      wrapperVersion: PINNED_CLOAKBROWSER_WRAPPER_VERSION,
    };
  }

  async ensureRuntime(): Promise<BinaryInfo & { wrapperVersion: string }> {
    await diagnosticsToStderr(() =>
      cloak.ensureBinary(undefined, PINNED_CLOAKBROWSER_BINARY_VERSION),
    );
    return this.runtimeStatus();
  }

  async launch(payload: LaunchPayload): Promise<{ launched: true; url: string }> {
    await this.shutdown();
    this.profileDir = payload.profileDir;
    const transport = preferredBrowserTransport();
    try {
      if (transport === 'cdp-websocket') {
        const { browser, context, page, proc } = await this.launchWindowsCdp(payload);
        this.browser = browser;
        this.context = context;
        this.page = page;
        this.browserProcess = proc;
      } else {
        const launchOptions = persistentLaunchOptions(payload);
        await this.ensureRuntime();
        this.context = await diagnosticsToStderr(() =>
          cloak.launchPersistentContext(launchOptions),
        );
        this.page = this.context.pages()[0] ?? (await this.context.newPage());
      }

      if (payload.startUrl) {
        await this.navigate({ url: payload.startUrl });
      }
      return { launched: true, url: this.page.url() };
    } catch (error) {
      await this.shutdown();
      throw mapBrowserError(error, 'browser_startup');
    }
  }

  private async launchWindowsCdp(
    payload: LaunchPayload,
  ): Promise<{ browser: playwright.Browser; context: BrowserContext; page: Page; proc: ChildProcess }> {
    if (!payload.profileDir) {
      throw new RuntimeFault('invalid_input', 'profileDir is required', false);
    }
    const binaryInfo = await this.ensureRuntime();
    const binaryPath = binaryInfo.binaryPath;
    if (!binaryPath) {
      throw new RuntimeFault('runtime_unavailable', 'Chromium binary path is not available', false);
    }

    const args = buildWindowsChromiumArgs({
      profileDir: payload.profileDir,
      headless: payload.headless,
    });

    const isWindows = process.platform === 'win32';
    const proc = spawn(binaryPath, args, {
      windowsHide: true,
      detached: !isWindows,
      stdio: ['ignore', 'ignore', 'pipe'],
    });

    const wsEndpoint = await new Promise<string>((resolve, reject) => {
      let resolved = false;
      let accumulatedStderr = '';
      const timeoutId = setTimeout(() => {
        if (!resolved) {
          resolved = true;
          try { proc.kill(); } catch {}
          reject(new RuntimeFault('browser_startup', `Timeout waiting for Chromium DevTools endpoint on Windows. Stderr: ${accumulatedStderr}`, true));
        }
      }, 30_000);

      proc.stderr?.on('data', (chunk: Buffer) => {
        const text = chunk.toString();
        accumulatedStderr += text;
        try {
          const endpoint = parseDevToolsWsEndpoint(text);
          if (endpoint && !resolved) {
            resolved = true;
            clearTimeout(timeoutId);
            resolve(endpoint);
          }
        } catch (err) {
          if (!resolved) {
            resolved = true;
            clearTimeout(timeoutId);
            reject(err);
          }
        }
      });

      proc.on('error', (err) => {
        if (!resolved) {
          resolved = true;
          clearTimeout(timeoutId);
          reject(new RuntimeFault('browser_startup', `Failed to spawn Chromium: ${err.message}`, true));
        }
      });

      proc.on('exit', (code, signal) => {
        if (!resolved) {
          resolved = true;
          clearTimeout(timeoutId);
          reject(new RuntimeFault('browser_startup', `Chromium exited prematurely (code=${code}, signal=${signal}). Stderr: ${accumulatedStderr}`, true));
        }
      });
    });

    const browser = await playwright.chromium.connectOverCDP(wsEndpoint);
    const context = browser.contexts()[0] ?? (await browser.newContext());
    const page = context.pages()[0] ?? (await context.newPage());
    return { browser, context, page, proc };
  }

  async navigate(payload: NavigatePayload): Promise<{ url: string; title: string }> {
    const page = this.requirePage();
    const url = validateHttpUrl(payload.url);
    try {
      await page.goto(url, {
        waitUntil: 'domcontentloaded',
        timeout: payload.timeoutMs ?? 30_000,
      });
      return { url: page.url(), title: await page.title() };
    } catch (error) {
      throw mapBrowserError(error, 'navigation_timeout');
    }
  }

  async evaluate(payload: EvaluatePayload): Promise<{ value: unknown }> {
    const page = this.requirePage();
    if (!payload.expression) {
      throw new RuntimeFault('invalid_input', 'expression is required', false);
    }
    try {
      const value = await Promise.race([
        page.evaluate((expression) => globalThis.eval(expression), payload.expression),
        timeout(payload.timeoutMs ?? 30_000, 'evaluation_timeout'),
      ]);
      return { value };
    } catch (error) {
      throw mapBrowserError(error, 'evaluation_timeout');
    }
  }

  async screenshot(payload: ScreenshotPayload): Promise<{ path: string }> {
    const page = this.requirePage();
    if (!payload.path) {
      throw new RuntimeFault('invalid_input', 'screenshot path is required', false);
    }
    await page.screenshot({ path: payload.path, fullPage: payload.fullPage ?? true });
    return { path: payload.path };
  }

  async healthCheck() {
    return inspectGoogleMapsPage(this.requirePage());
  }

  async shutdown(): Promise<{ stopped: true }> {
    const browser = this.browser;
    const context = this.context;
    const proc = this.browserProcess;
    const profileDir = this.profileDir;
    this.browser = null;
    this.context = null;
    this.page = null;
    this.browserProcess = null;
    this.profileDir = null;

    if (context) {
      const contextBrowser = context.browser();
      await context.close().catch((error) => {
        console.error('[cloak-runtime] context close failed', error);
      });
      if (contextBrowser) {
        await contextBrowser.close().catch((error) => {
          console.error('[cloak-runtime] context browser close failed', error);
        });
      }
    }
    if (browser) {
      await browser.close().catch((error) => {
        console.error('[cloak-runtime] browser close failed', error);
      });
    }
    if (proc || profileDir) {
      killProcessTree(proc ?? undefined, profileDir ?? undefined);
    }
    return { stopped: true };
  }

  private requirePage(): Page {
    if (!this.page || this.page.isClosed()) {
      throw new RuntimeFault('disconnected', 'browser page is not available', true);
    }
    return this.page;
  }
}

export function killProcessTree(proc?: ChildProcess, userDataDir?: string): void {
  if (process.platform === 'win32') {
    if (proc) {
      try {
        proc.kill('SIGTERM');
      } catch {}
    }
    return;
  }
  const rootPid = proc?.pid;
  try {
    const pidsToKill = new Set<number>();
    if (rootPid) {
      pidsToKill.add(rootPid);
    }
    if (process.platform === 'linux') {
      try {
        let addedNew = true;
        while (addedNew) {
          addedNew = false;
          const entries = readdirSync('/proc');
          for (const entry of entries) {
            if (!/^\d+$/.test(entry)) continue;
            const pid = parseInt(entry, 10);
            if (pidsToKill.has(pid)) continue;
            try {
              if (userDataDir) {
                const cmdline = readFileSync(`/proc/${entry}/cmdline`, 'utf8').replaceAll('\0', ' ');
                if (cmdline.includes(userDataDir)) {
                  pidsToKill.add(pid);
                  addedNew = true;
                  continue;
                }
              }
              const stat = readFileSync(`/proc/${entry}/stat`, 'utf8');
              const closeParen = stat.lastIndexOf(')');
              if (closeParen !== -1) {
                const rest = stat.slice(closeParen + 2).split(' ');
                const ppid = parseInt(rest[1], 10);
                if (pidsToKill.has(ppid)) {
                  pidsToKill.add(pid);
                  addedNew = true;
                }
              }
            } catch {}
          }
        }
      } catch {}
    }

    if (rootPid) {
      try {
        process.kill(-rootPid, 'SIGKILL');
      } catch {}
    }
    for (const pid of pidsToKill) {
      try {
        process.kill(pid, 'SIGKILL');
      } catch {}
    }
  } catch (error) {
    console.error('[cloak-runtime] killProcessTree failed', error);
  }
}

function configureCloakBrowserEnvironment(options: CloakRuntimeOptions) {
  process.env.CLOAKBROWSER_AUTO_UPDATE = 'false';
  process.env.CLOAKBROWSER_SKIP_CHECKSUM = 'false';
  process.env.CLOAKBROWSER_VERSION = PINNED_CLOAKBROWSER_BINARY_VERSION;
  delete process.env.CLOAKBROWSER_BINARY_PATH;
  delete process.env.CLOAKBROWSER_DOWNLOAD_URL;
  if (options.cacheDir) {
    process.env.CLOAKBROWSER_CACHE_DIR = options.cacheDir;
  }
}

async function diagnosticsToStderr<T>(operation: () => Promise<T>): Promise<T> {
  const originalLog = console.log;
  console.log = (...args: unknown[]) => console.error(...args);
  try {
    return await operation();
  } finally {
    console.log = originalLog;
  }
}

function validateHttpUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new RuntimeFault('invalid_input', 'url must be valid', false);
  }
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) {
    throw new RuntimeFault('invalid_input', 'url must be credential-free HTTP(S)', false);
  }
  return url.toString();
}

function timeout(timeoutMs: number, kind: 'evaluation_timeout'): Promise<never> {
  return new Promise((_, reject) => {
    setTimeout(() => reject(new RuntimeFault(kind, 'evaluation timed out', true)), timeoutMs);
  });
}

function mapBrowserError(
  error: unknown,
  timeoutKind: 'navigation_timeout' | 'evaluation_timeout' | 'browser_startup',
): RuntimeFault {
  if (error instanceof RuntimeFault) {
    return error;
  }
  const message = error instanceof Error ? error.message : String(error);
  const normalized = message.toLowerCase();
  if (normalized.includes('timeout')) {
    return new RuntimeFault(timeoutKind, message, true);
  }
  if (
    normalized.includes('closed') ||
    normalized.includes('disconnected') ||
    normalized.includes('target page')
  ) {
    return new RuntimeFault('disconnected', message, true);
  }
  if (timeoutKind === 'browser_startup') {
    return new RuntimeFault('browser_startup', message, true);
  }
  return new RuntimeFault('transient_network', message, true);
}
