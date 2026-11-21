import { describe, expect, test } from 'bun:test';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { classifyGoogleMapsPageState, inspectGoogleMapsPage } from './googleMaps';
import { dispatch, redirectConsoleToStderr, RuntimeFault, type RuntimeAdapter } from './index';
import {
  CloakRuntime,
  PINNED_CLOAKBROWSER_BINARY_VERSION,
  killProcessTree,
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

const linuxTest = process.platform === 'linux' ? test : test.skip;

function linuxProcessCommandLine(pid: number): string {
  if (process.platform !== 'linux') return '';
  try {
    return readFileSync(`/proc/${pid}/cmdline`, 'utf8');
  } catch {
    return '';
  }
}

async function waitForLinuxCommandLine(pid: number, fragment: string): Promise<void> {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (linuxProcessCommandLine(pid).includes(fragment)) return;
    await Bun.sleep(25);
  }
  expect(linuxProcessCommandLine(pid)).toContain(fragment);
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

async function inspectFixture(html: string, url: string) {
  const { JSDOM } = await import('jsdom');
  const dom = new JSDOM(
    `<!doctype html><html><head><title>Google Maps</title></head><body>${html}</body></html>`,
    { url },
  );
  const body = dom.window.document.body;
  Object.defineProperty(body, 'innerText', {
    configurable: true,
    value: body.textContent ?? '',
  });

  const globals = globalThis as unknown as {
    document?: Document;
    window?: Window & typeof globalThis;
  };
  const previousDocument = globals.document;
  const previousWindow = globals.window;
  globals.document = dom.window.document as unknown as Document;
  globals.window = dom.window as unknown as Window & typeof globalThis;
  try {
    return await inspectGoogleMapsPage({
      evaluate: async (pageFunction: () => unknown) => pageFunction(),
    } as never);
  } finally {
    if (previousDocument === undefined) delete globals.document;
    else globals.document = previousDocument;
    if (previousWindow === undefined) delete globals.window;
    else globals.window = previousWindow;
    dom.window.close();
  }
}

describe('browser process cleanup', () => {
  linuxTest('launch_cleans_stale_profile_processes_before_start', async () => {
    const root = mkdtempSync(join(tmpdir(), 'rustsale-launch-clean-'));
    const profileDir = join(root, 'profile');
    mkdirSync(profileDir, { recursive: true });
    const singletonFiles = ['SingletonLock', 'SingletonCookie', 'SingletonSocket'];
    for (const name of singletonFiles) {
      await Bun.write(join(profileDir, name), 'stale');
    }
    const child = Bun.spawn(
      [process.execPath, '-e', 'setInterval(() => {}, 1000)', `--user-data-dir=${profileDir}`],
      { stdin: 'ignore', stdout: 'ignore', stderr: 'ignore' },
    );
    const instance = new CloakRuntime({ cacheDir: join(root, 'cache') });
    let launchStarted = false;
    let staleProcessAliveAtStart = true;
    let singletonLockAtStart = true;
    (instance as unknown as {
      launchWindowsCdp: () => Promise<never>;
    }).launchWindowsCdp = async () => {
      launchStarted = true;
      staleProcessAliveAtStart = linuxProcessCommandLine(child.pid).length > 0;
      singletonLockAtStart = existsSync(join(profileDir, 'SingletonLock'));
      throw new RuntimeFault('browser_startup', 'stop-after-cleanup', false);
    };

    const originalTransport = process.env.RUSTSALE_BROWSER_TRANSPORT;
    process.env.RUSTSALE_BROWSER_TRANSPORT = 'cdp-websocket';
    try {
      await waitForLinuxCommandLine(child.pid, profileDir);
      await expect(instance.launch({ profileDir, headless: true })).rejects.toThrow(
        'stop-after-cleanup',
      );
      expect(launchStarted).toBe(true);
      expect(staleProcessAliveAtStart).toBe(false);
      expect(singletonLockAtStart).toBe(false);
    } finally {
      if (originalTransport) process.env.RUSTSALE_BROWSER_TRANSPORT = originalTransport;
      else delete process.env.RUSTSALE_BROWSER_TRANSPORT;
      child.kill('SIGKILL');
      await child.exited;
      rmSync(root, { recursive: true, force: true });
    }
  }, 30_000);

  linuxTest('kill_process_tree_ignores_unrelated_process_with_path_in_argv', async () => {
    const root = mkdtempSync(join(tmpdir(), 'rustsale-kill-tree-'));
    const profileDir = join(root, 'profile');
    const child = Bun.spawn([process.execPath, '-e', 'setInterval(() => {}, 1000)', profileDir], {
      stdin: 'ignore',
      stdout: 'ignore',
      stderr: 'ignore',
    });
    try {
      await waitForLinuxCommandLine(child.pid, profileDir);
      killProcessTree(undefined, profileDir);
      await Bun.sleep(300);
      expect(linuxProcessCommandLine(child.pid)).toContain(profileDir);
    } finally {
      child.kill('SIGKILL');
      await child.exited;
      rmSync(root, { recursive: true, force: true });
    }
  }, 30_000);
});

describe('stdout protocol channel', () => {
  test('console.log and friends go to stderr, not the protocol stdout', () => {
    const errors: unknown[][] = [];
    const fake = {
      log: (..._args: unknown[]) => {
        throw new Error('log must be redirected');
      },
      info: (..._args: unknown[]) => {
        throw new Error('info must be redirected');
      },
      debug: (..._args: unknown[]) => {
        throw new Error('debug must be redirected');
      },
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    };

    redirectConsoleToStderr(fake);
    fake.log('a');
    fake.info('b');
    fake.debug('c');

    expect(errors).toEqual([['a'], ['b'], ['c']]);
  });
});

describe('sidecar lifecycle', () => {
  test(
    'evaluate_clears_timeout_timer',
    async () => {
      const fixture = join(import.meta.dir, 'test-fixtures', 'evaluate-once-sidecar.ts');
      const child = Bun.spawn([process.execPath, fixture], {
        stdin: 'pipe',
        stdout: 'pipe',
        stderr: 'pipe',
      });
      const reader = child.stdout.getReader();
      const decoder = new TextDecoder();
      let stdout = '';
      child.stdin.write(
        `${JSON.stringify({
          id: 'eval-1',
          command: 'evaluate',
          payload: { expression: '1', timeoutMs: 3_000 },
        })}\n`,
      );
      child.stdin.end();

      const responseDeadline = Date.now() + 10_000;
      while (!stdout.includes('"ok":true') && Date.now() < responseDeadline) {
        const { value, done } = await reader.read();
        if (done) break;
        stdout += decoder.decode(value, { stream: true });
      }

      const outcome = await Promise.race([
        child.exited.then((code) => ({ code })),
        Bun.sleep(2_000).then(() => ({ code: 'timeout' as const })),
      ]);
      if (outcome.code === 'timeout') {
        child.kill('SIGKILL');
      }
      await child.exited;
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        stdout += decoder.decode(value, { stream: true });
      }

      expect(stdout).toContain('"id":"eval-1"');
      expect(stdout).toContain('"ok":true');
      expect(outcome.code).toBe(0);
    },
    20_000,
  );

  test('evaluate_reports_evaluation_timeout_when_expression_stalls', async () => {
    const instance = new CloakRuntime();
    (instance as unknown as { page: unknown }).page = {
      isClosed: () => false,
      evaluate: () => new Promise(() => {}),
    };

    await expect(
      instance.evaluate({ expression: 'await new Promise(() => {})', timeoutMs: 25 }),
    ).rejects.toMatchObject({ kind: 'evaluation_timeout', retryable: true });
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

  test('classify_feed_with_harmless_aria_label_is_ready', async () => {
    const harmless = await inspectFixture(
      '<div role="feed"><h1>Results</h1></div><button aria-label="Chấp nhận quảng cáo">Ads</button>',
      'https://www.google.com/maps/search/cafe',
    );
    expect(harmless.state).toBe('ready');

    const consent = await inspectFixture(
      '<div role="dialog"><button aria-label="Chấp nhận tất cả">OK</button></div>',
      'https://www.google.com/maps',
    );
    expect(consent.state).toBe('consent_required');
  }, 30_000);

  test('classify_error_page_with_h1_is_not_ready', () => {
    expect(
      classifyGoogleMapsPageState({
        url: 'https://www.google.com/maps/place/Example',
        title: 'Error 500 (Server Error)',
        hasPlaceHeading: true,
        bodyText: 'Error Server Error',
      }),
    ).toBe('selector_changed');
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

    test('Playwright WebSocketTransport in Bun connects, sends/receives messages, forwards headers, and closes cleanly', async () => {
      let receivedHeader = '';
      const server = Bun.serve({
        port: 0,
        fetch(req, server) {
          receivedHeader = req.headers.get('x-custom-cdp-header') ?? '';
          if (server.upgrade(req)) {
            return undefined;
          }
          return new Response('Upgrade failed', { status: 400 });
        },
        websocket: {
          open(ws) {
            ws.send(JSON.stringify({ id: 1, method: 'Target.attachedToTarget', params: { sessionId: 's1' } }));
          },
          message(ws, message) {
            const parsed = JSON.parse(String(message));
            ws.send(JSON.stringify({ id: parsed.id, result: { success: true } }));
          },
          close() {},
        },
      });

      try {
        const wsUrl = `ws://127.0.0.1:${server.port}/devtools/browser`;
        const { resolve } = await import('node:path');
        const transportFilePath = resolve('node_modules/playwright-core/lib/server/transport.js');
        const transportSource = readFileSync(transportFilePath, 'utf8');
        expect(transportSource).toContain('class BunNativeWebSocket');
        expect(transportSource).toContain('new BunNativeWebSocket');

        // @ts-ignore
        const { WebSocketTransport } = await import(transportFilePath);
        const progress = {
          log: () => {},
          time: () => {},
          timeEnd: () => {},
          race: async (promises: Promise<any>[] | Promise<any>) =>
            Promise.race(Array.isArray(promises) ? promises : [promises]),
        };
        const transport = await WebSocketTransport.connect(progress as any, wsUrl, {
          headers: { 'x-custom-cdp-header': 'rustsale-test-value' },
        });

        expect(receivedHeader).toBe('rustsale-test-value');

        const receivedMessages: any[] = [];
        transport.onmessage = (msg: any) => receivedMessages.push(msg);

        // Send a message
        transport.send({ id: 2, method: 'Browser.getVersion', params: {} });

        // Wait for responses
        for (let i = 0; i < 20; i++) {
          if (receivedMessages.length >= 2) break;
          await Bun.sleep(20);
        }

        expect(receivedMessages).toContainEqual({ id: 1, method: 'Target.attachedToTarget', params: { sessionId: 's1' } });
        expect(receivedMessages).toContainEqual({ id: 2, result: { success: true } });

        await transport.closeAndWait();
      } finally {
        server.stop(true);
      }
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
