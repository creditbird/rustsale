/** Waits until the async `check` holds, polling every 50 ms; throws after `timeout` ms. */
export async function eventually(check: () => Promise<boolean>, timeout = 8000): Promise<void> {
  const end = Date.now() + timeout;
  for (;;) {
    if (await check()) return;
    if (Date.now() > end) throw new Error("eventually: the condition did not hold in time");
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
