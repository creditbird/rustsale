import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const binaryPath = process.argv[2] || process.env.SIDECAR_PATH;
if (!binaryPath) {
  console.error('[assert-portable-sidecar] Usage: bun run scripts/assert-portable-sidecar.ts <path-to-binary>');
  process.exit(1);
}

const resolvedPath = resolve(binaryPath);
if (!existsSync(resolvedPath)) {
  console.error(`[assert-portable-sidecar] Binary not found at: ${resolvedPath}`);
  process.exit(1);
}

console.log(`[assert-portable-sidecar] Scanning binary for build path leaks: ${resolvedPath}`);

const forbiddenPatterns = [
  'D:\\a\\rustsale\\rustsale',
  'D:/a/rustsale/rustsale',
  '/home/runner/work/',
  '/Users/runner/work/',
  'playwright-core\\package.json',
  'playwright-core/package.json',
];

const binaryBuffer = readFileSync(resolvedPath);

const violations: string[] = [];

for (const pattern of forbiddenPatterns) {
  const patternBuffer = Buffer.from(pattern);
  if (binaryBuffer.includes(patternBuffer)) {
    violations.push(pattern);
  }
}

if (violations.length > 0) {
  throw new Error(
    `[assert-portable-sidecar] FATAL: Non-portable build path leaked into sidecar binary:\n` +
      violations.map((v) => `  - ${v}`).join('\n'),
  );
}

console.log('[assert-portable-sidecar] Portable check passed: No leaked CI/build paths found.');
