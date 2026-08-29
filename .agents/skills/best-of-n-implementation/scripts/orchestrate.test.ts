import { afterEach, describe, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { buildCandidatePrompt, validateTaskSpec, type TaskSpec } from './orchestrate';

const ORCHESTRATOR = resolve(import.meta.dir, 'orchestrate.ts');
const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function command(args: string[], cwd: string, env: Record<string, string> = {}) {
  return Bun.spawnSync(args, { cwd, stdout: 'pipe', stderr: 'pipe', env: { ...process.env, ...env } });
}

function git(args: string[], cwd: string): string {
  const result = command(['git', ...args], cwd);
  if (result.exitCode !== 0) throw new Error(result.stderr.toString());
  return result.stdout.toString().trim();
}

function fixture(mode: 'shadow' | 'enforced' = 'enforced') {
  const root = mkdtempSync(join(tmpdir(), 'best-of-n-test-'));
  roots.push(root);
  const repo = join(root, 'repo');
  mkdirSync(repo);
  writeFileSync(join(repo, 'sample.txt'), 'base\n');
  git(['init', '-q'], repo);
  git(['config', 'user.email', 'test@example.com'], repo);
  git(['config', 'user.name', 'Best Of N Test'], repo);
  git(['add', 'sample.txt'], repo);
  git(['commit', '-qm', 'base'], repo);

  const checkScript = join(root, 'check.ts');
  writeFileSync(checkScript, "const text = await Bun.file('sample.txt').text(); process.exit(text.startsWith('candidate-') ? 0 : 1);\n");
  const verifierScript = join(root, 'verifier.ts');
  writeFileSync(
    verifierScript,
    "import { writeFileSync } from 'node:fs'; const a=Bun.argv; const out=a[a.indexOf('--output')+1]; writeFileSync(out, JSON.stringify({status:'SELECTED',backend:'llm-verifier',selected_index:1,ranking:[1,0],scores:[0.4,0.6]}));\n",
  );
  const spec: TaskSpec = {
    task_id: 'T-1',
    problem: 'Change the sample using the strongest implementation.',
    requirements: ['The sample must identify the candidate.'],
    acceptance_criteria: ['The focused check passes.'],
    risk_tier: 2,
    mode,
    candidate_count: 2,
    focused_checks: [['bun', checkScript]],
    ranking_backend: 'llm-verifier',
    verifier: { n_evaluations: 4, pivots: 2, seed: 0 },
  };
  const specPath = join(root, 'task.json');
  writeFileSync(specPath, JSON.stringify(spec));
  return { root, repo, spec, specPath, verifierScript };
}

function cli(args: string[], cwd: string, verifierScript?: string) {
  const env = verifierScript ? { BEST_OF_N_VERIFIER_COMMAND: JSON.stringify(['bun', verifierScript]) } : {};
  return command(['bun', ORCHESTRATOR, ...args], cwd, env);
}

function initAndCollect(mode: 'shadow' | 'enforced' = 'enforced') {
  const data = fixture(mode);
  const initialized = cli(['init', '--spec', data.specPath], data.repo);
  expect(initialized.exitCode).toBe(0);
  const initOutput = JSON.parse(initialized.stdout.toString());
  const runDir = initOutput.run_dir as string;
  const baseSha = initOutput.base_sha as string;
  const branches: string[] = [];

  for (let index = 1; index <= 2; index += 1) {
    const branch = join(data.root, `branch-${index}`);
    git(['worktree', 'add', '--detach', branch, baseSha], data.repo);
    branches.push(branch);
    writeFileSync(join(branch, 'sample.txt'), `candidate-${index}\n`);
    const collected = cli(['collect', '--run-dir', runDir, '--candidate', `candidate-${index}`], branch);
    expect(collected.exitCode).toBe(0);
    expect(JSON.parse(collected.stdout.toString()).status).toBe('ELIGIBLE');
    git(['worktree', 'remove', '--force', branch], data.repo);
  }
  return { ...data, runDir, baseSha, branches };
}

describe('task validation', () => {
  test('accepts a bounded argv-based task contract', () => {
    const { spec } = fixture('shadow');
    expect(validateTaskSpec(spec)).toEqual(spec);
  });

  test('defaults to agy-ppt and validates its bounded configuration', () => {
    const { spec } = fixture('shadow');
    const { ranking_backend: _backend, verifier: _verifier, ...withoutBackend } = spec;
    expect(validateTaskSpec(withoutBackend).ranking_backend).toBe('agy-ppt');
    expect(validateTaskSpec({
      ...withoutBackend,
      ranking_backend: 'agy-ppt',
      agy_verifier: { effort: 'high', pivots: 1, seed: 7, tie_breaker: true, max_parallel_pairs: 2 },
    }).ranking_backend).toBe('agy-ppt');
    expect(() => validateTaskSpec({ ...spec, ranking_backend: 'unknown' })).toThrow('ranking_backend');
    expect(() => validateTaskSpec({
      ...withoutBackend,
      ranking_backend: 'agy-ppt',
      agy_verifier: { max_parallel_pairs: 9 },
    })).toThrow('max_parallel_pairs');
  });

  test('rejects shell and forbidden JavaScript runtimes', () => {
    const { spec } = fixture('shadow');
    expect(() => validateTaskSpec({ ...spec, focused_checks: [['bash', '-c', 'true']] })).toThrow('argv array');
    expect(() => validateTaskSpec({ ...spec, focused_checks: [['npm', 'test']] })).toThrow('must use bun/bunx');
  });

  test('candidate prompt requires agy branch isolation and collection', () => {
    const { spec } = fixture('shadow');
    const prompt = buildCandidatePrompt(spec, '/tmp/run', 'candidate-1');
    expect(prompt).toContain('workspace mode `branch`');
    expect(prompt).toContain('bun run best-of-n -- collect');
    expect(prompt).toContain('Avoid full release builds');
  });
});

describe('Best-of-N state machine', () => {
  test('collects isolated branches, ranks, and explicitly applies the winner', () => {
    const data = initAndCollect('enforced');
    expect(readFileSync(join(data.repo, 'sample.txt'), 'utf8')).toBe('base\n');

    const ranked = cli(['rank', '--run-dir', data.runDir], data.repo, data.verifierScript);
    expect(ranked.exitCode).toBe(0);
    expect(JSON.parse(ranked.stdout.toString()).selected_candidate).toBe('candidate-2');
    expect(readFileSync(join(data.repo, 'sample.txt'), 'utf8')).toBe('base\n');

    const applied = cli(['apply', '--run-dir', data.runDir], data.repo);
    expect(applied.exitCode).toBe(0);
    expect(JSON.parse(applied.stdout.toString()).status).toBe('APPLIED');
    expect(readFileSync(join(data.repo, 'sample.txt'), 'utf8')).toBe('candidate-2\n');
  });

  test('refuses initialization when the primary working tree is dirty', () => {
    const data = fixture('shadow');
    writeFileSync(join(data.repo, 'untracked.txt'), 'user work');
    const initialized = cli(['init', '--spec', data.specPath], data.repo);
    expect(initialized.exitCode).toBe(2);
    expect(initialized.stderr.toString()).toContain('working tree must be clean');
    expect(readFileSync(join(data.repo, 'untracked.txt'), 'utf8')).toBe('user work');
  });

  test('refuses collection from the primary workspace', () => {
    const data = fixture('shadow');
    const initialized = cli(['init', '--spec', data.specPath], data.repo);
    const runDir = JSON.parse(initialized.stdout.toString()).run_dir;
    writeFileSync(join(data.repo, 'sample.txt'), 'candidate-main\n');
    const collected = cli(['collect', '--run-dir', runDir, '--candidate', 'candidate-1'], data.repo);
    expect(collected.exitCode).toBe(2);
    expect(collected.stderr.toString()).toContain('isolated agy branch workspace');
  });

  test('shadow selection cannot be applied', () => {
    const data = initAndCollect('shadow');
    const ranked = cli(['rank', '--run-dir', data.runDir], data.repo, data.verifierScript);
    expect(ranked.exitCode).toBe(0);
    const applied = cli(['apply', '--run-dir', data.runDir], data.repo);
    expect(applied.exitCode).toBe(2);
    expect(applied.stderr.toString()).toContain('shadow runs cannot apply');
    expect(readFileSync(join(data.repo, 'sample.txt'), 'utf8')).toBe('base\n');
  });

  test('fails closed when the verifier fails', () => {
    const data = initAndCollect('enforced');
    const failingVerifier = join(data.root, 'failing-verifier.ts');
    writeFileSync(failingVerifier, 'process.exit(7);\n');
    const ranked = cli(['rank', '--run-dir', data.runDir], data.repo, failingVerifier);
    expect(ranked.exitCode).toBe(2);
    const manifest = JSON.parse(readFileSync(join(data.runDir, 'manifest.json'), 'utf8'));
    expect(manifest.status).toBe('INCOMPLETE');
    expect(manifest.limitations[0]).toContain('llm-verifier ranking failed (7)');
  });
});
