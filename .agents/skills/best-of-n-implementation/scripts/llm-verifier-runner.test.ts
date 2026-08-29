import { afterEach, expect, test } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ADAPTER = resolve(import.meta.dir, 'llm-verifier-runner.py');
const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

test('adapter enforces fail-closed selection and emits token evidence', () => {
  const root = mkdtempSync(join(tmpdir(), 'llm-verifier-adapter-test-'));
  roots.push(root);
  const moduleDir = join(root, 'module');
  mkdirSync(moduleDir);
  writeFileSync(
    join(moduleDir, 'llm_verifier.py'),
    `
class Usage:
    def reset(self): pass
USAGE = Usage()
class Result:
    index = 1
    ranking = [1, 0]
    scores = [0.25, 0.75]
    n_comparisons = 2
    criteria = ["requirements"]
def select(problem, candidates, **kwargs):
    assert kwargs["on_error"] == "raise"
    assert len(candidates) == 2
    return Result()
def token_usage(): return {"calls": 8}
`,
  );
  const input = join(root, 'input.json');
  const output = join(root, 'output.json');
  writeFileSync(input, JSON.stringify({
    problem: 'task',
    candidates: ['a', 'b'],
    criteria_path: '/tmp/criteria.md',
    verifier: { n_evaluations: 4, pivots: 2, seed: 0 },
  }));
  const result = Bun.spawnSync(['python3', ADAPTER, '--input', input, '--output', output], {
    stdout: 'pipe',
    stderr: 'pipe',
    env: { ...process.env, PYTHONPATH: moduleDir },
  });
  expect(result.exitCode).toBe(0);
  const payload = JSON.parse(readFileSync(output, 'utf8'));
  expect(payload.status).toBe('SELECTED');
  expect(payload.on_error).toBe('raise');
  expect(payload.token_usage.calls).toBe(8);
});

test('adapter removes output and exits incomplete for fewer than two candidates', () => {
  const root = mkdtempSync(join(tmpdir(), 'llm-verifier-adapter-test-'));
  roots.push(root);
  const input = join(root, 'input.json');
  const output = join(root, 'output.json');
  writeFileSync(input, JSON.stringify({ problem: 'task', candidates: ['a'], criteria_path: '/tmp/criteria.md' }));
  writeFileSync(output, 'stale');
  const result = Bun.spawnSync(['python3', ADAPTER, '--input', input, '--output', output], {
    stdout: 'pipe',
    stderr: 'pipe',
  });
  expect(result.exitCode).toBe(2);
  expect(result.stderr.toString()).toContain('at least two eligible candidates');
  expect(existsSync(output)).toBe(false);
});
