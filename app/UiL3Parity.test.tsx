// The v2 Zalo screen only runs the batched campaign export; the single-account export is gone.
import { expect, test } from "nsh/test";
import { Campaigns } from "rustsale/native";

test("Zalo exposes the batched campaign export and not the single-account one", () => {
  const exports = Campaigns as unknown as Record<string, unknown>;
  expect(typeof exports.startZaloOutreach).toBe("function");
  expect("sendZaloOutreach" in exports).toBe(false);
});
