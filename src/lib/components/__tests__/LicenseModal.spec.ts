import { describe, test, expect } from 'bun:test';

describe('LicenseModal component', () => {
  test('imports without error', async () => {
    const mod = await import('../LicenseModal.svelte');
    expect(mod.default).toBeDefined();
  });
});
