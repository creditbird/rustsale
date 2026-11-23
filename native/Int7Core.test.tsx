// int7 (#52): AI Approval lists of every status, saving the edited text, missing ids, and N3 deletes.
// `nsh test` runs this in a headless runner linked with the crate, on a throwaway data folder.
import { expect, test } from "nsh/test";
import { Ai, Host, Templates } from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-int7-${Date.now()}`;

async function codeOf(promise: Promise<unknown>): Promise<string> {
  try {
    await promise;
  } catch (error) {
    return String((error as { code?: unknown }).code);
  }
  return "no error";
}

test("proposals list takes a status and refuses an unknown one", async () => {
  await Host.start(dir);
  expect(await Ai.proposals(null)).toEqual([]);
  expect(await Ai.proposals("ALL")).toEqual([]);
  expect(await Ai.proposals("approved")).toEqual([]);
  expect(await codeOf(Ai.proposals("DONE"))).toBe("invalid_request");
});

test("editing or reviewing a proposal that does not exist is not_found", async () => {
  await Host.start(dir);
  expect(await codeOf(Ai.editProposal(987654, "text"))).toBe("not_found");
  expect(await codeOf(Ai.reviewProposal(987654, "APPROVED", "qa"))).toBe("not_found");
  expect(await codeOf(Ai.editProposal(1, "   "))).toBe("invalid_request");
  expect(await codeOf(Ai.editProposal(1.5, "text"))).toBe("invalid_request");
});

test("deleting a template that is gone is not_found", async () => {
  await Host.start(dir);
  expect(await codeOf(Templates.deleteTemplate(987654))).toBe("not_found");
});
