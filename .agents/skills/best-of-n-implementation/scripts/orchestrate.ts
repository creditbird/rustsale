import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';

export type TaskSpec = {
  task_id: string;
  problem: string;
  requirements: string[];
  acceptance_criteria: string[];
  risk_tier: 1 | 2 | 3;
  mode: 'shadow' | 'enforced';
  candidate_count: number;
  focused_checks: string[][];
  ranking_backend: 'agy-ppt' | 'llm-verifier';
  agy_verifier?: {
    effort?: 'low' | 'medium' | 'high';
    model?: string;
    pivots?: number;
    seed?: number;
    tie_breaker?: boolean;
    max_parallel_pairs?: number;
  };
  verifier?: { model?: string; n_evaluations?: number; pivots?: number; seed?: number };
};

type CommandResult = { command: string[]; exit_code: number; stdout: string; stderr: string };
type CandidateRecord = {
  candidate_id: string;
  state: 'PENDING' | 'COLLECTED';
  base_sha: string;
  changed_files: string[];
  checks: CommandResult[];
  eligible: boolean;
  limitations: string[];
};
type RunManifest = {
  version: 1;
  status: 'AWAITING_CANDIDATES' | 'SELECTED' | 'APPLIED' | 'INCOMPLETE';
  run_dir: string;
  repo_root: string;
  git_common_dir: string;
  base_sha: string;
  created_at: string;
  spec: TaskSpec;
  candidates: CandidateRecord[];
  selected_candidate: string | null;
  limitations: string[];
};

const SKILL_ROOT = resolve(import.meta.dir, '..');
const LLM_VERIFIER_ADAPTER_PATH = join(import.meta.dir, 'llm-verifier-runner.py');
const AGY_PPT_ADAPTER_PATH = join(import.meta.dir, 'agy-ppt-runner.ts');
const CRITERIA_PATH = join(SKILL_ROOT, 'references', 'rustsale-criteria.md');
const FORBIDDEN_RUNTIME_COMMANDS = new Set(['node', 'npm', 'npx']);
const SHELL_COMMANDS = new Set(['bash', 'sh', 'zsh', 'fish']);

function assertObject(value: unknown, name: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${name} must be an object`);
}

function assertKnownKeys(value: Record<string, unknown>, allowed: string[], name: string): void {
  const unknown = Object.keys(value).filter((key) => !allowed.includes(key));
  if (unknown.length) throw new Error(`${name} has unknown keys: ${unknown.join(', ')}`);
}

function assertStringArray(value: unknown, name: string): asserts value is string[] {
  if (!Array.isArray(value) || !value.length || value.some((item) => typeof item !== 'string' || !item.trim())) {
    throw new Error(`${name} must be a non-empty string array`);
  }
}

export function validateTaskSpec(value: unknown): TaskSpec {
  assertObject(value, 'task spec');
  assertKnownKeys(
    value,
    ['task_id', 'problem', 'requirements', 'acceptance_criteria', 'risk_tier', 'mode', 'candidate_count', 'focused_checks', 'ranking_backend', 'agy_verifier', 'verifier'],
    'task spec',
  );
  if (typeof value.task_id !== 'string' || !value.task_id.trim()) throw new Error('task_id is required');
  if (typeof value.problem !== 'string' || !value.problem.trim()) throw new Error('problem is required');
  assertStringArray(value.requirements, 'requirements');
  assertStringArray(value.acceptance_criteria, 'acceptance_criteria');
  if (![1, 2, 3].includes(value.risk_tier as number)) throw new Error('risk_tier must be 1, 2, or 3');
  if (value.mode !== 'shadow' && value.mode !== 'enforced') throw new Error('mode must be shadow or enforced');
  if (!Number.isInteger(value.candidate_count) || (value.candidate_count as number) < 2 || (value.candidate_count as number) > 5) {
    throw new Error('candidate_count must be an integer from 2 to 5');
  }
  if (!Array.isArray(value.focused_checks) || !value.focused_checks.length) {
    throw new Error('focused_checks must contain at least one command');
  }
  for (const [index, command] of value.focused_checks.entries()) {
    assertStringArray(command, `focused_checks[${index}]`);
    const executable = basename(command[0]);
    if (FORBIDDEN_RUNTIME_COMMANDS.has(executable)) throw new Error(`focused_checks[${index}] must use bun/bunx instead of ${executable}`);
    if (SHELL_COMMANDS.has(executable)) throw new Error(`focused_checks[${index}] must be an argv array, not a shell command`);
  }
  const rankingBackend = value.ranking_backend ?? 'agy-ppt';
  if (rankingBackend !== 'agy-ppt' && rankingBackend !== 'llm-verifier') {
    throw new Error('ranking_backend must be agy-ppt or llm-verifier');
  }
  if (value.agy_verifier !== undefined) {
    assertObject(value.agy_verifier, 'agy_verifier');
    assertKnownKeys(value.agy_verifier, ['effort', 'model', 'pivots', 'seed', 'tie_breaker', 'max_parallel_pairs'], 'agy_verifier');
    if (rankingBackend !== 'agy-ppt') throw new Error('agy_verifier is only valid with ranking_backend agy-ppt');
    if (value.agy_verifier.effort !== undefined && !['low', 'medium', 'high'].includes(value.agy_verifier.effort as string)) {
      throw new Error('agy_verifier.effort must be low, medium, or high');
    }
    if (value.agy_verifier.model !== undefined && (typeof value.agy_verifier.model !== 'string' || !value.agy_verifier.model.trim())) {
      throw new Error('agy_verifier.model must be a non-empty string');
    }
    const pivots = value.agy_verifier.pivots ?? 1;
    if (!Number.isInteger(pivots) || pivots < 1 || pivots > (value.candidate_count as number)) {
      throw new Error('agy_verifier.pivots must be an integer from 1 to candidate_count');
    }
    if (value.agy_verifier.seed !== undefined && !Number.isInteger(value.agy_verifier.seed)) {
      throw new Error('agy_verifier.seed must be an integer');
    }
    if (value.agy_verifier.tie_breaker !== undefined && typeof value.agy_verifier.tie_breaker !== 'boolean') {
      throw new Error('agy_verifier.tie_breaker must be a boolean');
    }
    const parallel = value.agy_verifier.max_parallel_pairs ?? 2;
    if (!Number.isInteger(parallel) || parallel < 1 || parallel > 3) {
      throw new Error('agy_verifier.max_parallel_pairs must be an integer from 1 to 3');
    }
  }
  if (value.verifier !== undefined) {
    assertObject(value.verifier, 'verifier');
    assertKnownKeys(value.verifier, ['model', 'n_evaluations', 'pivots', 'seed'], 'verifier');
    if (rankingBackend !== 'llm-verifier') throw new Error('verifier is only valid with ranking_backend llm-verifier');
    if (value.verifier.model !== undefined && (typeof value.verifier.model !== 'string' || !value.verifier.model.trim())) {
      throw new Error('verifier.model must be a non-empty string');
    }
    const evaluations = value.verifier.n_evaluations ?? 4;
    if (!Number.isInteger(evaluations) || evaluations < 1 || evaluations > 16) {
      throw new Error('verifier.n_evaluations must be an integer from 1 to 16');
    }
    const pivots = value.verifier.pivots ?? 2;
    if (!Number.isInteger(pivots) || pivots < 1 || pivots > 5 || pivots > (value.candidate_count as number)) {
      throw new Error('verifier.pivots must be an integer from 1 to candidate_count');
    }
    if (value.verifier.seed !== undefined && !Number.isInteger(value.verifier.seed)) {
      throw new Error('verifier.seed must be an integer');
    }
  }
  return { ...value, ranking_backend: rankingBackend } as TaskSpec;
}

function syncCommand(command: string[], cwd: string): CommandResult {
  const result = Bun.spawnSync(command, { cwd, stdout: 'pipe', stderr: 'pipe', env: process.env });
  return { command, exit_code: result.exitCode, stdout: result.stdout.toString(), stderr: result.stderr.toString() };
}

function git(args: string[], cwd: string): CommandResult {
  const result = syncCommand(['git', ...args], cwd);
  if (result.exit_code !== 0) throw new Error(`git ${args.join(' ')} failed (${result.exit_code}): ${result.stderr.trim()}`);
  return result;
}

function repoRoot(cwd: string): string {
  return realpathSync(git(['rev-parse', '--show-toplevel'], cwd).stdout.trim());
}

function gitCommonDir(cwd: string): string {
  const raw = git(['rev-parse', '--git-common-dir'], cwd).stdout.trim();
  return realpathSync(resolve(cwd, raw));
}

function assertClean(root: string): void {
  const status = git(['status', '--porcelain=v1', '--untracked-files=all'], root).stdout;
  if (status.trim()) throw new Error(`working tree must be clean:\n${status.trim()}`);
}

function persistManifest(manifest: RunManifest): void {
  writeFileSync(join(manifest.run_dir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
}

function loadManifest(runDir: string): RunManifest {
  const resolved = realpathSync(runDir);
  const manifest = JSON.parse(readFileSync(join(resolved, 'manifest.json'), 'utf8')) as RunManifest;
  if (manifest.version !== 1 || realpathSync(manifest.run_dir) !== resolved) throw new Error('invalid or relocated run manifest');
  return manifest;
}

export function buildCandidatePrompt(spec: TaskSpec, runDir: string, candidateId: string): string {
  return `You are ${candidateId}, an independent implementation candidate in an agy native subagent workspace.

The parent must invoke you with workspace mode \`branch\`. Read AGENTS.md, Agent Memory, and docs/IMPLEMENTATION_COMPLETION_HARNESS.md before editing. Follow GitNexus impact rules before changing existing symbols. Work only in your isolated branch workspace. Do not commit, push, merge, release, inspect another candidate, or modify the primary workspace.

Task:\n${spec.problem}

Requirements:\n${spec.requirements.map((item, index) => `${index + 1}. ${item}`).join('\n')}

Acceptance criteria:\n${spec.acceptance_criteria.map((item, index) => `${index + 1}. ${item}`).join('\n')}

Risk tier: ${spec.risk_tier}

Implement a complete root-cause solution. Avoid full release builds in this candidate workspace; the parent will run those only for the winner. When implementation is ready, collect the exact diff and lightweight checks with:

\`bun run best-of-n -- collect --run-dir ${runDir} --candidate ${candidateId}\`

Report the approach, observed results, and limitations to the parent only after that command succeeds. Eligibility is not approval.`;
}

function initRun(specPath: string): Record<string, unknown> {
  const primaryRoot = repoRoot(process.cwd());
  assertClean(primaryRoot);
  const spec = validateTaskSpec(JSON.parse(readFileSync(resolve(specPath), 'utf8')));
  const baseSha = git(['rev-parse', 'HEAD'], primaryRoot).stdout.trim();
  const runDir = mkdtempSync(join(tmpdir(), 'rustsale-best-of-n-'));
  mkdirSync(join(runDir, 'candidates'), { recursive: true });
  writeFileSync(join(runDir, 'task.json'), `${JSON.stringify(spec, null, 2)}\n`);

  const candidates = Array.from({ length: spec.candidate_count }, (_, index): CandidateRecord => ({
    candidate_id: `candidate-${index + 1}`,
    state: 'PENDING',
    base_sha: baseSha,
    changed_files: [],
    checks: [],
    eligible: false,
    limitations: [],
  }));
  const manifest: RunManifest = {
    version: 1,
    status: 'AWAITING_CANDIDATES',
    run_dir: runDir,
    repo_root: primaryRoot,
    git_common_dir: gitCommonDir(primaryRoot),
    base_sha: baseSha,
    created_at: new Date().toISOString(),
    spec,
    candidates,
    selected_candidate: null,
    limitations: [],
  };
  for (const candidate of candidates) {
    const candidateDir = join(runDir, 'candidates', candidate.candidate_id);
    mkdirSync(candidateDir, { recursive: true });
    writeFileSync(join(candidateDir, 'prompt.md'), buildCandidatePrompt(spec, runDir, candidate.candidate_id));
  }
  persistManifest(manifest);
  return {
    status: manifest.status,
    run_dir: runDir,
    base_sha: baseSha,
    candidates: candidates.map((candidate) => ({
      candidate_id: candidate.candidate_id,
      prompt_path: join(runDir, 'candidates', candidate.candidate_id, 'prompt.md'),
      workspace: 'branch',
    })),
    next: 'Invoke all candidate prompts concurrently as agy self subagents with workspace=branch, then run rank.',
  };
}

function collectCandidate(runDir: string, candidateId: string): Record<string, unknown> {
  const manifest = loadManifest(runDir);
  if (manifest.status !== 'AWAITING_CANDIDATES') throw new Error(`cannot collect while run status is ${manifest.status}`);
  const candidate = manifest.candidates.find((item) => item.candidate_id === candidateId);
  if (!candidate) throw new Error(`unknown candidate: ${candidateId}`);
  if (candidate.state !== 'PENDING') throw new Error(`${candidateId} was already collected`);

  const branchRoot = repoRoot(process.cwd());
  if (gitCommonDir(branchRoot) !== manifest.git_common_dir) throw new Error('candidate workspace belongs to a different Git repository');
  const head = git(['rev-parse', 'HEAD'], branchRoot).stdout.trim();
  if (head !== manifest.base_sha) throw new Error(`candidate HEAD drifted from locked base ${manifest.base_sha}`);
  if (branchRoot === manifest.repo_root) throw new Error('candidate collection must run in an isolated agy branch workspace');

  git(['add', '-N', '--', '.'], branchRoot);
  const patch = git(['diff', '--binary', 'HEAD', '--'], branchRoot).stdout;
  const changedFiles = git(['diff', '--name-only', 'HEAD', '--'], branchRoot).stdout
    .split('\n').map((item) => item.trim()).filter(Boolean);
  const checks = manifest.spec.focused_checks.map((command) => syncCommand(command, branchRoot));
  const limitations: string[] = [];
  if (!patch.trim()) limitations.push('candidate produced an empty diff');
  for (const check of checks) {
    if (check.exit_code !== 0) limitations.push(`focused check failed (${check.exit_code}): ${check.command.join(' ')}`);
  }

  const candidateDir = join(manifest.run_dir, 'candidates', candidateId);
  writeFileSync(join(candidateDir, 'candidate.patch'), patch);
  writeFileSync(join(candidateDir, 'checks.json'), `${JSON.stringify(checks, null, 2)}\n`);
  candidate.state = 'COLLECTED';
  candidate.changed_files = changedFiles;
  candidate.checks = checks;
  candidate.limitations = limitations;
  candidate.eligible = limitations.length === 0;
  writeFileSync(join(candidateDir, 'candidate.json'), `${JSON.stringify(candidate, null, 2)}\n`);
  persistManifest(manifest);
  return { status: candidate.eligible ? 'ELIGIBLE' : 'INELIGIBLE', run_dir: manifest.run_dir, ...candidate };
}

function candidateBundle(spec: TaskSpec, record: CandidateRecord, candidateDir: string): string {
  const patch = readFileSync(join(candidateDir, 'candidate.patch'), 'utf8');
  return `Task:\n${spec.problem}

Requirements:\n${spec.requirements.map((item) => `- ${item}`).join('\n')}

Acceptance criteria:\n${spec.acceptance_criteria.map((item) => `- ${item}`).join('\n')}

Changed files:\n${record.changed_files.map((item) => `- ${item}`).join('\n')}

Observed lightweight checks:\n${JSON.stringify(record.checks, null, 2)}

Final patch:\n${patch}`;
}

function rankingCommand(backend: TaskSpec['ranking_backend']): string[] {
  const override = process.env.BEST_OF_N_RANKER_COMMAND ?? process.env.BEST_OF_N_VERIFIER_COMMAND;
  if (override) {
    const parsed = JSON.parse(override);
    assertStringArray(parsed, 'BEST_OF_N_RANKER_COMMAND');
    return parsed;
  }
  if (backend === 'agy-ppt') return ['bun', AGY_PPT_ADAPTER_PATH];
  const cachedPython = join(homedir(), '.cache', 'rustsale-best-of-n', 'venv', 'bin', 'python');
  return [existsSync(cachedPython) ? cachedPython : 'python3', LLM_VERIFIER_ADAPTER_PATH];
}

function rankCandidates(runDir: string): { exitCode: number; output: Record<string, unknown> } {
  const manifest = loadManifest(runDir);
  if (manifest.status !== 'AWAITING_CANDIDATES') throw new Error(`cannot rank while run status is ${manifest.status}`);
  const primaryRoot = repoRoot(process.cwd());
  if (primaryRoot !== manifest.repo_root || gitCommonDir(primaryRoot) !== manifest.git_common_dir) {
    throw new Error('rank must run from the locked primary repository workspace');
  }
  assertClean(primaryRoot);
  const head = git(['rev-parse', 'HEAD'], primaryRoot).stdout.trim();
  if (head !== manifest.base_sha) throw new Error(`primary HEAD drifted from locked base ${manifest.base_sha}`);

  const eligible = manifest.candidates.filter((candidate) => candidate.state === 'COLLECTED' && candidate.eligible);
  if (eligible.length < 2) {
    manifest.status = 'INCOMPLETE';
    manifest.limitations.push(`Best-of-N requires at least two eligible candidates; observed ${eligible.length}`);
    persistManifest(manifest);
    return { exitCode: 2, output: manifest as unknown as Record<string, unknown> };
  }

  const rankingInput = {
    problem: manifest.spec.problem,
    candidates: eligible.map((candidate) => candidateBundle(manifest.spec, candidate, join(runDir, 'candidates', candidate.candidate_id))),
    criteria_path: CRITERIA_PATH,
    run_dir: manifest.run_dir,
    ...(manifest.spec.ranking_backend === 'agy-ppt'
      ? { agy_verifier: manifest.spec.agy_verifier ?? {} }
      : { cache_path: join(runDir, 'verifier-cache.json'), verifier: manifest.spec.verifier ?? {} }),
  };
  const inputPath = join(runDir, 'ranking-input.json');
  const outputPath = join(runDir, 'selection.json');
  writeFileSync(inputPath, `${JSON.stringify(rankingInput)}\n`);
  const result = syncCommand([...rankingCommand(manifest.spec.ranking_backend), '--input', inputPath, '--output', outputPath], primaryRoot);
  writeFileSync(join(runDir, 'verifier.stdout.txt'), result.stdout);
  writeFileSync(join(runDir, 'verifier.stderr.txt'), result.stderr);
  if (result.exit_code !== 0 || !existsSync(outputPath)) {
    manifest.status = 'INCOMPLETE';
    manifest.limitations.push(`${manifest.spec.ranking_backend} ranking failed (${result.exit_code}): ${result.stderr.trim()}`);
    persistManifest(manifest);
    return { exitCode: 2, output: manifest as unknown as Record<string, unknown> };
  }

  const selection = JSON.parse(readFileSync(outputPath, 'utf8')) as Record<string, unknown>;
  const selectedIndex = selection.selected_index;
  if (selection.status !== 'SELECTED' || selection.backend !== manifest.spec.ranking_backend || !Number.isInteger(selectedIndex) || (selectedIndex as number) < 0 || (selectedIndex as number) >= eligible.length) {
    manifest.status = 'INCOMPLETE';
    manifest.limitations.push(`${manifest.spec.ranking_backend} returned an invalid selection contract`);
    persistManifest(manifest);
    return { exitCode: 2, output: manifest as unknown as Record<string, unknown> };
  }
  manifest.status = 'SELECTED';
  manifest.selected_candidate = eligible[selectedIndex as number].candidate_id;
  persistManifest(manifest);
  return {
    exitCode: 0,
    output: {
      status: 'SELECTED',
      mode: manifest.spec.mode,
      run_dir: manifest.run_dir,
      base_sha: manifest.base_sha,
      ranking_backend: manifest.spec.ranking_backend,
      selected_candidate: manifest.selected_candidate,
      ranking: selection.ranking,
      scores: selection.scores,
      evidence_path: selection.evidence_path,
      note: manifest.spec.mode === 'shadow' ? 'Shadow mode: the selected patch was not applied.' : 'Inspect artifacts, then run apply.',
    },
  };
}

function applySelection(runDir: string): Record<string, unknown> {
  const manifest = loadManifest(runDir);
  const primaryRoot = repoRoot(process.cwd());
  if (primaryRoot !== manifest.repo_root || gitCommonDir(primaryRoot) !== manifest.git_common_dir) {
    throw new Error('run manifest belongs to a different primary repository workspace');
  }
  if (manifest.spec.mode !== 'enforced') throw new Error('shadow runs cannot apply a candidate');
  if (manifest.status !== 'SELECTED' || !manifest.selected_candidate) throw new Error('run has no unapplied selected candidate');
  assertClean(primaryRoot);
  const head = git(['rev-parse', 'HEAD'], primaryRoot).stdout.trim();
  if (head !== manifest.base_sha) throw new Error(`HEAD drifted from ${manifest.base_sha} to ${head}`);

  const patchPath = resolve(runDir, 'candidates', manifest.selected_candidate, 'candidate.patch');
  const candidateRoot = `${resolve(runDir, 'candidates')}\/`;
  if (!patchPath.startsWith(candidateRoot) || !existsSync(patchPath)) throw new Error('selected patch path is invalid');
  git(['apply', '--check', '--binary', patchPath], primaryRoot);
  git(['apply', '--binary', patchPath], primaryRoot);
  manifest.status = 'APPLIED';
  persistManifest(manifest);
  return {
    status: 'APPLIED',
    run_dir: manifest.run_dir,
    selected_candidate: manifest.selected_candidate,
    next: 'Run a fresh independent agy review, repair accepted findings, then close repository quality gates and Completion Lock.',
  };
}

function flag(args: string[], name: string): string {
  const index = args.indexOf(name);
  if (index < 0 || !args[index + 1]) throw new Error(`${name} is required`);
  return args[index + 1];
}

async function main(): Promise<number> {
  const [command, ...args] = Bun.argv.slice(2);
  let exitCode = 0;
  let output: Record<string, unknown>;
  if (command === 'init') output = initRun(flag(args, '--spec'));
  else if (command === 'collect') output = collectCandidate(flag(args, '--run-dir'), flag(args, '--candidate'));
  else if (command === 'rank') {
    const result = rankCandidates(flag(args, '--run-dir'));
    exitCode = result.exitCode;
    output = result.output;
  } else if (command === 'apply') output = applySelection(flag(args, '--run-dir'));
  else if (command === 'status') {
    output = loadManifest(flag(args, '--run-dir')) as unknown as Record<string, unknown>;
    exitCode = output.status === 'INCOMPLETE' ? 2 : 0;
  } else {
    throw new Error('Usage: bun run best-of-n -- <init --spec FILE | collect --run-dir DIR --candidate ID | rank --run-dir DIR | apply --run-dir DIR | status --run-dir DIR>');
  }
  console.log(JSON.stringify(output, null, 2));
  return exitCode;
}

if (import.meta.main) {
  main().then((code) => process.exit(code)).catch((error) => {
    console.error(JSON.stringify({ status: 'INCOMPLETE', error: error instanceof Error ? error.message : String(error) }, null, 2));
    process.exit(2);
  });
}
