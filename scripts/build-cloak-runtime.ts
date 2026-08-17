import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE || detectHostTriple();
const bunTarget = bunCompileTarget(targetTriple);
const extension = targetTriple.includes('windows') ? '.exe' : '';
const outputDir = resolve('src-tauri/binaries');
const outputPath = resolve(outputDir, `cloak-runtime-${targetTriple}${extension}`);
mkdirSync(outputDir, { recursive: true });

const compileArgs = [
  process.execPath,
  'build',
  resolve('sidecars/cloak-runtime/src/index.ts'),
  '--compile',
  `--target=${bunTarget}`,
  `--outfile=${outputPath}`,
  '--external=electron',
  '--external=electron/*',
  '--no-compile-autoload-dotenv',
  '--no-compile-autoload-bunfig',
];

const result = Bun.spawnSync(compileArgs);
if (!result.success) {
  process.stderr.write(result.stderr);
  process.exit(result.exitCode || 1);
}
process.stdout.write(`${outputPath}\n`);

function detectHostTriple(): string {
  const direct = Bun.spawnSync(['rustc', '--print', 'host-tuple']);
  if (direct.success) {
    return direct.stdout.toString().trim();
  }
  const verbose = Bun.spawnSync(['rustc', '-vV']);
  if (!verbose.success) {
    throw new Error('unable to detect Rust host triple');
  }
  const host = verbose.stdout
    .toString()
    .split('\n')
    .find((line) => line.startsWith('host: '))
    ?.slice('host: '.length)
    .trim();
  if (!host) throw new Error('rustc did not report a host triple');
  return host;
}

function bunCompileTarget(triple: string): string {
  const targets: Record<string, string> = {
    'x86_64-unknown-linux-gnu': 'bun-linux-x64',
    'aarch64-unknown-linux-gnu': 'bun-linux-arm64',
    'x86_64-pc-windows-msvc': 'bun-windows-x64',
    'x86_64-apple-darwin': 'bun-darwin-x64',
    'aarch64-apple-darwin': 'bun-darwin-arm64',
  };
  const target = targets[triple];
  if (!target) {
    throw new Error(`unsupported cloak-runtime target triple: ${triple}`);
  }
  return target;
}
