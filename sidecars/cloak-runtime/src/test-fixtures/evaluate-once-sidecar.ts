import { runSidecar } from '../index';
import { CloakRuntime } from '../runtime';

const runtime = new CloakRuntime();
(runtime as unknown as { page: unknown }).page = {
  isClosed: () => false,
  evaluate: async () => 'ok',
};

await runSidecar(runtime);
