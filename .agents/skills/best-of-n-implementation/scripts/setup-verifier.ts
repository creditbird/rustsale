import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

function run(command: string[]): void {
  const result = Bun.spawnSync(command, { stdout: 'inherit', stderr: 'inherit' });
  if (result.exitCode !== 0) {
    throw new Error(`Command failed (${result.exitCode}): ${command.join(' ')}`);
  }
}

const skillRoot = join(import.meta.dir, '..');
const requirements = join(import.meta.dir, 'requirements.txt');
const venvRoot = process.env.BEST_OF_N_VENV ?? join(homedir(), '.cache', 'rustsale-best-of-n', 'venv');
const python = join(venvRoot, 'bin', 'python');

if (!existsSync(python)) {
  run([process.env.BEST_OF_N_BOOTSTRAP_PYTHON ?? 'python3', '-m', 'venv', venvRoot]);
}

run([python, '-m', 'pip', 'install', '--requirement', requirements]);
run([python, '-c', 'import llm_verifier; print(llm_verifier.__version__)']);

console.log(JSON.stringify({ status: 'READY', skill_root: skillRoot, python }, null, 2));
