import type { BinaryInfo } from 'cloakbrowser';
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

type CloakBrowserModule = typeof import('cloakbrowser');

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
  private modulePromise: Promise<CloakBrowserModule> | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;

  constructor(options: CloakRuntimeOptions = {}) {
    configureCloakBrowserEnvironment(options);
  }

  async runtimeStatus(): Promise<BinaryInfo & { wrapperVersion: string }> {
    const cloak = await this.loadModule();
    return {
      ...cloak.binaryInfo(PINNED_CLOAKBROWSER_BINARY_VERSION),
      wrapperVersion: PINNED_CLOAKBROWSER_WRAPPER_VERSION,
    };
  }

  async ensureRuntime(): Promise<BinaryInfo & { wrapperVersion: string }> {
    const cloak = await this.loadModule();
    await diagnosticsToStderr(() =>
      cloak.ensureBinary(undefined, PINNED_CLOAKBROWSER_BINARY_VERSION),
    );
    return this.runtimeStatus();
  }

  async launch(payload: LaunchPayload): Promise<{ launched: true; url: string }> {
    const launchOptions = persistentLaunchOptions(payload);
    await this.shutdown();
    const cloak = await this.loadModule();
    await this.ensureRuntime();
    try {
      this.context = await diagnosticsToStderr(() =>
        cloak.launchPersistentContext(launchOptions),
      );
      this.page = this.context.pages()[0] ?? (await this.context.newPage());
      if (payload.startUrl) {
        await this.navigate({ url: payload.startUrl });
      }
      return { launched: true, url: this.page.url() };
    } catch (error) {
      await this.shutdown();
      throw mapBrowserError(error, 'browser_startup');
    }
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
    const context = this.context;
    this.context = null;
    this.page = null;
    if (context) {
      await context.close().catch((error) => {
        console.error('[cloak-runtime] context close failed', error);
      });
    }
    return { stopped: true };
  }

  private requirePage(): Page {
    if (!this.page || this.page.isClosed()) {
      throw new RuntimeFault('disconnected', 'browser page is not available', true);
    }
    return this.page;
  }

  private loadModule(): Promise<CloakBrowserModule> {
    this.modulePromise ??= diagnosticsToStderr(() => import('cloakbrowser'));
    return this.modulePromise;
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
