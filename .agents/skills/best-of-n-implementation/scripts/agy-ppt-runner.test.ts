import { afterEach, expect, test } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ringPairs } from './agy-ppt-runner';

const RUNNER = resolve(import.meta.dir, 'agy-ppt-runner.ts');
const CRITERIA = resolve(import.meta.dir, '..', 'references', 'rustsale-criteria.md');
const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(options: { tieBreaker?: boolean; candidates?: string[] } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'agy-ppt-runner-test-'));
  roots.push(root);
  const fakeAgy = join(root, 'fake-agy.ts');
  const logPath = join(root, 'agy-calls.jsonl');
  writeFileSync(fakeAgy, `
import { appendFileSync, readFileSync } from 'node:fs';
const args = Bun.argv.slice(2);
const prompt = args[args.indexOf('--print') + 1] ?? '';
const match = prompt.match(/Judge input file: ([^\\n]+)/);
if (!match) process.exit(9);
const pair = JSON.parse(readFileSync(match[1].trim(), 'utf8'));
const mode = process.env.FAKE_AGY_MODE ?? 'quality';
const quality = (value) => Number(value.match(/QUALITY=(\\d+)/)?.[1] ?? 0);
const qualityA = quality(pair.candidate_a);
const qualityB = quality(pair.candidate_b);
let preferred = qualityA === qualityB ? 'TIE' : qualityA > qualityB ? 'A' : 'B';
if (mode === 'position-bias') preferred = 'A';
if (mode === 'position-then-quality' && pair.judge_sequence <= 2) preferred = 'A';
let scoreA = Math.min(20, 4 + qualityA * 4);
let scoreB = Math.min(20, 4 + qualityB * 4);
if (preferred === 'A' && scoreA <= scoreB) [scoreA, scoreB] = [18, 10];
if (preferred === 'B' && scoreB <= scoreA) [scoreA, scoreB] = [10, 18];
const criteria = pair.criteria_ids.map((id) => ({
  id,
  score_a: scoreA,
  score_b: scoreB,
  evidence: [\`observed \${id}\`],
  blockers_a: pair.candidate_a.includes('BLOCKED=true') ? ['candidate A has a blocking defect'] : [],
  blockers_b: pair.candidate_b.includes('BLOCKED=true') ? ['candidate B has a blocking defect'] : [],
}));
const structured_output = mode === 'malformed'
  ? { preferred: 'A' }
  : { preferred, summary: 'synthetic independent judge', criteria, limitations: [] };
appendFileSync(process.env.FAKE_AGY_LOG, JSON.stringify({ args, pair, structured_output }) + '\\n');
console.log(JSON.stringify({
  conversation_id: \`fake-\${pair.pair_id}-\${pair.judge_sequence}\`,
  status: 'SUCCESS',
  structured_output,
  usage: { input_tokens: 100, output_tokens: 20, thinking_tokens: 10, cache_read_tokens: 0, total_tokens: 120 },
}));
`);
  const input = join(root, 'input.json');
  const output = join(root, 'output.json');
  writeFileSync(input, JSON.stringify({
    problem: 'Select the strongest implementation.',
    candidates: options.candidates ?? ['QUALITY=1', 'QUALITY=3', 'QUALITY=2'],
    criteria_path: CRITERIA,
    run_dir: root,
    agy_verifier: {
      effort: 'high',
      pivots: 1,
      seed: 0,
      tie_breaker: options.tieBreaker ?? true,
      max_parallel_pairs: 1,
    },
  }));
  return { root, fakeAgy, logPath, input, output };
}

function run(data: ReturnType<typeof fixture>, mode = 'quality') {
  return Bun.spawnSync(['bun', RUNNER, '--input', data.input, '--output', data.output], {
    stdout: 'pipe',
    stderr: 'pipe',
    env: {
      ...process.env,
      BEST_OF_N_AGY_COMMAND: JSON.stringify(['bun', data.fakeAgy]),
      FAKE_AGY_LOG: data.logPath,
      FAKE_AGY_MODE: mode,
    },
  });
}

test('does not duplicate the only ring comparison for two candidates', () => {
  expect(ringPairs(2, 0)).toHaveLength(1);
});

test('selects with blinded reversed judges and persists auditable agy-ppt evidence', () => {
  const data = fixture();
  const result = run(data);
  expect(result.exitCode).toBe(0);
  const payload = JSON.parse(readFileSync(data.output, 'utf8'));
  expect(payload.status).toBe('SELECTED');
  expect(payload.backend).toBe('agy-ppt');
  expect(payload.selected_index).toBe(1);
  expect(payload.ranking[0]).toBe(1);
  expect(payload.judge_calls).toBeGreaterThanOrEqual(2);
  expect(existsSync(payload.evidence_path)).toBe(true);

  const calls = readFileSync(data.logPath, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
  expect(calls.every((call) => call.args.includes('--mode') && call.args.includes('plan'))).toBe(true);
  expect(calls.every((call) => call.args.includes('--sandbox') && call.args.includes('--json-schema'))).toBe(true);
  const firstPair = calls.filter((call) => call.pair.pair_id === calls[0].pair.pair_id);
  expect(firstPair[0].pair.candidate_a).toBe(firstPair[1].pair.candidate_b);
  expect(firstPair[0].pair.candidate_b).toBe(firstPair[1].pair.candidate_a);
});

test('uses an adaptive third judge when the reversed judges disagree', () => {
  const data = fixture({ candidates: ['QUALITY=1', 'QUALITY=3'] });
  const result = run(data, 'position-then-quality');
  expect(result.exitCode).toBe(0);
  const payload = JSON.parse(readFileSync(data.output, 'utf8'));
  expect(payload.selected_index).toBe(1);
  expect(payload.n_comparisons).toBe(1);
  expect(payload.judge_calls).toBe(3);
  const calls = readFileSync(data.logPath, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
  expect(calls.some((call) => call.pair.judge_sequence === 3)).toBe(true);
});

test('fails closed when position-biased judges disagree and tie-breaker is disabled', () => {
  const data = fixture({ tieBreaker: false, candidates: ['QUALITY=1', 'QUALITY=3'] });
  writeFileSync(data.output, 'stale');
  const result = run(data, 'position-bias');
  expect(result.exitCode).toBe(2);
  expect(result.stderr.toString()).toContain('disagreed');
  expect(existsSync(data.output)).toBe(false);
});

test('fails closed on malformed agy structured output', () => {
  const data = fixture();
  writeFileSync(data.output, 'stale');
  const result = run(data, 'malformed');
  expect(result.exitCode).toBe(2);
  expect(result.stderr.toString()).toContain('structured output');
  expect(existsSync(data.output)).toBe(false);
});

test('refuses to overwrite evidence from an earlier ranking attempt', () => {
  const data = fixture();
  mkdirSync(join(data.root, 'agy-ppt-evidence'));
  const result = run(data);
  expect(result.exitCode).toBe(2);
  expect(result.stderr.toString()).toContain('evidence directory already exists');
  expect(existsSync(data.output)).toBe(false);
});

test('does not select a tournament winner with observed blocking findings', () => {
  const data = fixture({ candidates: ['QUALITY=1', 'QUALITY=3 BLOCKED=true', 'QUALITY=2'] });
  const result = run(data);
  expect(result.exitCode).toBe(2);
  expect(result.stderr.toString()).toContain('blocking findings');
  expect(existsSync(data.output)).toBe(false);
});
