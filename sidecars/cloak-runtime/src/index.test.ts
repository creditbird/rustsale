import { describe, expect, test } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { classifyGoogleMapsPageState } from './googleMaps';
import { dispatch, RuntimeFault, type RuntimeAdapter } from './index';
import {
  CloakRuntime,
  PINNED_CLOAKBROWSER_BINARY_VERSION,
  persistentLaunchOptions,
  pinnedCloakBrowserBinaryVersion,
  preferredBrowserTransport,
  parseDevToolsWsEndpoint,
  waitForDevToolsWsEndpoint,
  buildWindowsChromiumArgs,
} from './runtime';

function runtime(overrides: Partial<RuntimeAdapter> = {}): RuntimeAdapter {
  return {
    runtimeStatus: async () => ({ installed: false }),
    ensureRuntime: async () => ({ installed: true }),
    launch: async () => ({ launched: true }),
    navigate: async () => ({ url: 'https://example.test', title: 'Example' }),
    evaluate: async () => ({ value: null }),
    screenshot: async () => ({ path: '/tmp/screenshot.png' }),
    healthCheck: async () => ({ state: 'ready' }),
    shutdown: async () => ({ stopped: true }),
    ...overrides,
  };
}

async function waitForNoLinuxProcess(fragment: string): Promise<void> {
  if (process.platform !== 'linux') return;
  let matches: string[] = [];
  for (let attempt = 0; attempt < 40; attempt += 1) {
    matches = readdirSync('/proc', { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && /^\d+$/.test(entry.name))
      .flatMap((entry) => {
        try {
          const commandLine = readFileSync(`/proc/${entry.name}/cmdline`, 'utf8').replaceAll('\0', ' ');
          return commandLine.includes(fragment) ? [commandLine] : [];
        } catch {
          return [];
        }
      });
    if (matches.length === 0) return;
    await Bun.sleep(50);
  }
  expect(matches).toEqual([]);
}

describe('cloak-runtime protocol', () => {
  test('returns a correlated typed error', async () => {
    const response = await dispatch(
      { id: '42', command: 'navigate', payload: { url: 'https://example.test' } },
      runtime({
        navigate: async () => {
          throw new RuntimeFault('navigation_timeout', 'navigation timed out', true);
        },
      }),
    );

    expect(response).toEqual({
      id: '42',
      ok: false,
      error: {
        kind: 'navigation_timeout',
        message: 'navigation timed out',
        retryable: true,
      },
    });
  });

  test('removes terminal escape sequences from correlated runtime errors', async () => {
    const response = await dispatch(
      { id: 'ansi-1', command: 'navigate', payload: { url: 'https://example.test' } },
      runtime({
        navigate: async () => {
          throw new RuntimeFault(
            'disconnected',
            '\u001b[2mTarget page, context or browser has been closed\u001b[22m\u001b]8;;https://example.test\u0007details\u001b]8;;\u0007',
            true,
          );
        },
      }),
    );

    expect(response).toEqual({
      id: 'ansi-1',
      ok: false,
      error: {
        kind: 'disconnected',
        message: 'Target page, context or browser has been closeddetails',
        retryable: true,
      },
    });
  });

  test('rejects an unknown command without losing correlation', async () => {
    const response = await dispatch(
      { id: 'abc', command: 'unknown', payload: {} } as never,
      runtime(),
    );

    expect(response).toEqual({
      id: 'abc',
      ok: false,
      error: {
        kind: 'invalid_input',
        message: 'unsupported runtime command: unknown',
        retryable: false,
      },
    });
  });

  test('dispatches runtime status through the injected adapter', async () => {
    const response = await dispatch(
      { id: 'status-1', command: 'runtime_status', payload: {} },
      runtime({ runtimeStatus: async () => ({ installed: true, version: '146.0.7680.177.5' }) }),
    );

    expect(response).toEqual({
      id: 'status-1',
      ok: true,
      result: { installed: true, version: '146.0.7680.177.5' },
    });
  });
});

describe('Google Maps block detection', () => {
  test('pins the official CloakBrowser binary released for each supported platform', () => {
    expect(pinnedCloakBrowserBinaryVersion('linux', 'x64')).toBe('146.0.7680.177.5');
    expect(pinnedCloakBrowserBinaryVersion('linux', 'arm64')).toBe('146.0.7680.177.3');
    expect(pinnedCloakBrowserBinaryVersion('darwin', 'x64')).toBe('145.0.7632.109.2');
    expect(pinnedCloakBrowserBinaryVersion('darwin', 'arm64')).toBe('145.0.7632.109.2');
    expect(pinnedCloakBrowserBinaryVersion('win32', 'x64')).toBe('146.0.7680.177.5');
    expect(() => pinnedCloakBrowserBinaryVersion('win32', 'arm64')).toThrow(
      'unsupported CloakBrowser platform',
    );
  });

  test('classifies ready, empty, captcha, consent, rate limit, blank pages, and selector drift', () => {
    expect(classifyGoogleMapsPageState({ url: 'https://google.com/maps', hasFeed: true })).toBe(
      'ready',
    );
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps',
        bodyText: 'No results found',
      }),
    ).toBe('empty_results');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps',
        bodyText: "Google Maps can't find zzqv-production-qa-1785045000",
      }),
    ).toBe('empty_results');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/sorry/index',
        bodyText: 'Our systems have detected unusual traffic',
      }),
    ).toBe('captcha');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://consent.google.com',
        hasConsentDialog: true,
      }),
    ).toBe('consent_required');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps',
        bodyText: 'Too many requests. Try again later.',
      }),
    ).toBe('rate_limited');
    expect(classifyGoogleMapsPageState({ url: 'https://google.com/maps' })).toBe('blank_page');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps/search/cafe',
        bodyText: 'Google Maps search shell with an unrecognized result layout',
      }),
    ).toBe(
      'selector_changed',
    );
  });

  test('structural feed evidence outranks incidental body text', () => {
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps/search/cafe',
        hasFeed: true,
        bodyText: 'Pho Sam — "they sell out by noon, try again later" · No results found nearby',
      }),
    ).toBe('ready');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps/place/Pho-Sam',
        hasPlaceHeading: true,
        bodyText: 'One review mentions the rate limit on their loyalty card.',
      }),
    ).toBe('ready');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps/search/quan-an',
        hasFeed: true,
        bodyText: 'Quán ăn ngon — "tôi đồng ý với đánh giá trước, quá nhiều yêu cầu nên phải thử lại sau"',
      }),
    ).toBe('ready');
  });

  test('detects Vietnamese consent and rate limits when structural feed is absent', () => {
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps',
        hasFeed: false,
        bodyText: 'Trước khi tiếp tục đến Google, vui lòng xem xét điều khoản...',
      }),
    ).toBe('consent_required');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps/search/cafe',
        hasFeed: false,
        bodyText: 'Google Maps không thể tìm thấy bất kỳ kết quả nào cho tìm kiếm của bạn.',
      }),
    ).toBe('empty_results');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://google.com/maps',
        hasFeed: false,
        bodyText: 'Quá nhiều yêu cầu, vui lòng thử lại sau.',
      }),
    ).toBe('rate_limited');
  });

  test('a challenge overlay is detected even while the feed DOM survives', () => {
    expect(
      classifyGoogleMapsPageState({
        url: 'https://www.google.com/maps/search/cafe',
        hasFeed: true,
        hasCaptchaChallenge: true,
        bodyText: 'Cafe Sam · Cafe Lam · Cafe Trung',
      }),
    ).toBe('captcha');
  });

  test('genuine blocks still outrank structural evidence', () => {
    expect(
      classifyGoogleMapsPageState({
        url: 'https://www.google.com/sorry/index',
        hasPlaceHeading: true,
        bodyText: 'Our systems have detected unusual traffic',
      }),
    ).toBe('captcha');
    expect(
      classifyGoogleMapsPageState({
        url: 'https://www.google.com/maps',
        hasFeed: true,
        hasConsentDialog: true,
      }),
    ).toBe('consent_required');
  });

  test('captcha relaunch reuses the exact persistent profile in headed mode', () => {
    const profileDir = '/app-data/google-maps/jobs/job-1/profile';
    const headless = persistentLaunchOptions({ profileDir, headless: true });
    const headed = persistentLaunchOptions({
      profileDir,
      headless: false,
      startUrl: 'https://www.google.com/sorry/index',
    });

    expect(headless.userDataDir).toBe(profileDir);
    expect(headed.userDataDir).toBe(headless.userDataDir);
    expect(headed.headless).toBe(false);
  });

  describe('Browser transport and Windows CDP launcher', () => {
    test('selects cdp-websocket on win32 and playwright-pipe on unix', () => {
      const originalEnv = process.env.RUSTSALE_BROWSER_TRANSPORT;
      delete process.env.RUSTSALE_BROWSER_TRANSPORT;
      try {
        expect(preferredBrowserTransport('win32')).toBe('cdp-websocket');
        expect(preferredBrowserTransport('linux')).toBe('playwright-pipe');
        expect(preferredBrowserTransport('darwin')).toBe('playwright-pipe');
      } finally {
        if (originalEnv) process.env.RUSTSALE_BROWSER_TRANSPORT = originalEnv;
        else delete process.env.RUSTSALE_BROWSER_TRANSPORT;
      }
    });

    test('honors explicit RUSTSALE_BROWSER_TRANSPORT environment override', () => {
      const originalEnv = process.env.RUSTSALE_BROWSER_TRANSPORT;
      try {
        process.env.RUSTSALE_BROWSER_TRANSPORT = 'cdp-websocket';
        expect(preferredBrowserTransport('linux')).toBe('cdp-websocket');

        process.env.RUSTSALE_BROWSER_TRANSPORT = 'pipe';
        expect(preferredBrowserTransport('win32')).toBe('playwright-pipe');
      } finally {
        if (originalEnv) process.env.RUSTSALE_BROWSER_TRANSPORT = originalEnv;
        else delete process.env.RUSTSALE_BROWSER_TRANSPORT;
      }
    });

    test('parses loopback DevTools ws endpoint from Chromium output', () => {
      const logLine = 'DevTools listening on ws://127.0.0.1:49152/devtools/browser/abc-123-def';
      expect(parseDevToolsWsEndpoint(logLine)).toBe('ws://127.0.0.1:49152/devtools/browser/abc-123-def');
      expect(parseDevToolsWsEndpoint('random log line')).toBeNull();
    });

    test('parses loopback DevTools ws endpoint across split and fragmented chunks', () => {
      const chunk1 = 'Some previous Chromium noise\nDevTools listening ';
      const chunk2 = 'on ws://127.0.0.1:49152/devtools/browser/abc-split-def\nMore noise';
      expect(parseDevToolsWsEndpoint(chunk1)).toBeNull();
      const combined = chunk1 + chunk2;
      expect(parseDevToolsWsEndpoint(combined)).toBe('ws://127.0.0.1:49152/devtools/browser/abc-split-def');
    });

    test('refuses non-loopback DevTools endpoints for security', () => {
      const remoteEndpoint = 'DevTools listening on ws://0.0.0.0:9222/devtools/browser/abc';
      expect(() => parseDevToolsWsEndpoint(remoteEndpoint)).toThrow('Refusing non-loopback DevTools endpoint');

      const hostnameEndpoint = 'DevTools listening on ws://localhost:9222/devtools/browser/abc';
      expect(() => parseDevToolsWsEndpoint(hostnameEndpoint)).toThrow('Refusing non-loopback DevTools endpoint');
    });

    test('builds Windows Chromium arguments with explicit 127.0.0.1 address and protects against overrides', () => {
      const args = buildWindowsChromiumArgs({
        profileDir: 'C:\\Users\\User\\AppData\\Local\\Temp\\profile-1',
        headless: true,
        extraArgs: ['--remote-debugging-address=0.0.0.0', '--user-data-dir=C:\\evil', '--custom-flag=true'],
      });

      expect(args).toContain('--remote-debugging-address=127.0.0.1');
      expect(args).not.toContain('--remote-debugging-address=0.0.0.0');
      expect(args).toContain('--remote-debugging-port=0');
      expect(args).toContain('--user-data-dir=C:\\Users\\User\\AppData\\Local\\Temp\\profile-1');
      expect(args).not.toContain('--user-data-dir=C:\\evil');
      expect(args).toContain('--custom-flag=true');
      expect(args).toContain('--headless');
      expect(args).toContain('about:blank');
    });

    test('waitForDevToolsWsEndpoint parses endpoint across emitted stderr chunks and cleans up listeners', async () => {
      const { EventEmitter } = await import('node:events');
      const proc = new EventEmitter() as any;
      const stderr = new EventEmitter();
      proc.stderr = stderr;

      const waitPromise = waitForDevToolsWsEndpoint(proc, { timeoutMs: 1000 });

      expect(stderr.listenerCount('data')).toBe(1);
      expect(proc.listenerCount('error')).toBe(1);
      expect(proc.listenerCount('exit')).toBe(1);

      stderr.emit('data', Buffer.from('Chrome startup noise...\nDevTools listening '));
      stderr.emit('data', Buffer.from('on ws://127.0.0.1:51234/devtools/browser/abc-stream-def\n'));

      const endpoint = await waitPromise;
      expect(endpoint).toBe('ws://127.0.0.1:51234/devtools/browser/abc-stream-def');

      expect(stderr.listenerCount('data')).toBe(0);
      expect(proc.listenerCount('error')).toBe(0);
      expect(proc.listenerCount('exit')).toBe(0);
    });

    test('waitForDevToolsWsEndpoint removes listeners and rejects on timeout', async () => {
      const { EventEmitter } = await import('node:events');
      const proc = new EventEmitter() as any;
      const stderr = new EventEmitter();
      proc.stderr = stderr;

      const waitPromise = waitForDevToolsWsEndpoint(proc, { timeoutMs: 50 });

      await expect(waitPromise).rejects.toThrow('Timeout waiting for Chromium DevTools endpoint');
      expect(stderr.listenerCount('data')).toBe(0);
      expect(proc.listenerCount('error')).toBe(0);
      expect(proc.listenerCount('exit')).toBe(0);
    });

    test('waitForDevToolsWsEndpoint removes listeners and rejects on process error event', async () => {
      const { EventEmitter } = await import('node:events');
      const proc = new EventEmitter() as any;
      const stderr = new EventEmitter();
      proc.stderr = stderr;

      const waitPromise = waitForDevToolsWsEndpoint(proc, { timeoutMs: 1000 });
      proc.emit('error', new Error('Simulated binary spawn error'));

      await expect(waitPromise).rejects.toThrow('Failed to spawn Chromium: Simulated binary spawn error');
      expect(stderr.listenerCount('data')).toBe(0);
      expect(proc.listenerCount('error')).toBe(0);
      expect(proc.listenerCount('exit')).toBe(0);
    });

    test('waitForDevToolsWsEndpoint removes listeners and rejects on premature exit event', async () => {
      const { EventEmitter } = await import('node:events');
      const proc = new EventEmitter() as any;
      const stderr = new EventEmitter();
      proc.stderr = stderr;

      const waitPromise = waitForDevToolsWsEndpoint(proc, { timeoutMs: 1000 });
      proc.emit('exit', 1, null);

      await expect(waitPromise).rejects.toThrow('Chromium exited prematurely (code=1, signal=null)');
      expect(stderr.listenerCount('data')).toBe(0);
      expect(proc.listenerCount('error')).toBe(0);
      expect(proc.listenerCount('exit')).toBe(0);
    });

    test('waitForDevToolsWsEndpoint trims buffer when stderr exceeds maxBufferBytes', async () => {
      const { EventEmitter } = await import('node:events');
      const proc = new EventEmitter() as any;
      const stderr = new EventEmitter();
      proc.stderr = stderr;

      const waitPromise = waitForDevToolsWsEndpoint(proc, { timeoutMs: 1000, maxBufferBytes: 200 });
      stderr.emit('data', 'A'.repeat(300));
      stderr.emit('data', '\nDevTools listening on ws://127.0.0.1:49999/devtools/browser/trimmed\n');

      const endpoint = await waitPromise;
      expect(endpoint).toBe('ws://127.0.0.1:49999/devtools/browser/trimmed');
      expect(stderr.listenerCount('data')).toBe(0);
    });
  });
});

const runtimeSmokeTest = process.env.RUN_CLOAK_BROWSER_SMOKE === '1' ? test : test.skip;

runtimeSmokeTest(
  'smoke downloads, reuses, launches, and cleans CloakBrowser runtime',
  async () => {
    const root = mkdtempSync(join(tmpdir(), 'rustsale-cloak-smoke-'));
    const cacheDir = join(root, 'cache');
    const profileDir = join(root, 'profile');
    const screenshotPath = join(root, 'smoke.png');
    const server = Bun.serve({
      hostname: '127.0.0.1',
      port: 0,
      fetch: () =>
        new Response('<!doctype html><title>RustSale Smoke</title><div role="feed">Ready</div>', {
          headers: { 'content-type': 'text/html; charset=utf-8' },
        }),
    });
    const runtime = new CloakRuntime({ cacheDir });

    try {
      const initial = await runtime.runtimeStatus();
      expect(initial.installed).toBe(false);

      const downloaded = await runtime.ensureRuntime();
      expect(downloaded.installed).toBe(true);
      expect(downloaded.version).toBe(PINNED_CLOAKBROWSER_BINARY_VERSION);
      expect(downloaded.cacheDir.startsWith(cacheDir)).toBe(true);
      expect(statSync(downloaded.binaryPath).size).toBeGreaterThan(0);

      const reused = await runtime.ensureRuntime();
      expect(reused.binaryPath).toBe(downloaded.binaryPath);
      expect(reused.version).toBe(downloaded.version);

      const url = `http://${server.hostname}:${server.port}/maps`;
      const launched = await runtime.launch({ profileDir, headless: true, startUrl: url });
      expect(launched.launched).toBe(true);
      const evaluated = await runtime.evaluate({
        expression: 'document.querySelector(\'[role="feed"]\')?.textContent',
      });
      expect(evaluated.value).toBe('Ready');
      expect((await runtime.healthCheck()).state).toBe('ready');
      await runtime.screenshot({ path: screenshotPath, fullPage: true });
      expect(existsSync(screenshotPath)).toBe(true);
      expect(statSync(screenshotPath).size).toBeGreaterThan(0);
    } finally {
      await runtime.shutdown();
      server.stop(true);
      await waitForNoLinuxProcess(profileDir);
      rmSync(root, { recursive: true, force: true });
    }
  },
  180_000,
);
