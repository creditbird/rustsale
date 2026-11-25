// int8 (#55): deletes of rows that are gone, positional ids of the wrong type, and the WhatsApp campaign
// without an account. `nsh test` runs this in a headless runner linked with the crate, on a throwaway data folder.
import { expect, test } from "nsh/test";
import {
  Ai,
  Campaigns,
  ConversationActions,
  Host,
  Leads,
  LeadsMore,
  Templates
} from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-int8-${Date.now()}`;
const WRONG_TYPE = "id has the wrong type: it must be a whole number";

async function failure(promise: Promise<unknown>): Promise<{ code: unknown; message: string }> {
  try {
    await promise;
  } catch (error) {
    const err = error as { code?: unknown; message?: unknown };
    return { code: err.code, message: String(err.message) };
  }
  throw new Error("expected a rejection");
}

test("int8 item1: deleting a lead twice, or a campaign or quick template that is gone, is not_found", async () => {
  await Host.start(dir);
  const leadId = Number(await Leads.add({ name: "QA int8 lead", status: "New" }));
  await LeadsMore.delete(leadId);
  expect((await failure(LeadsMore.delete(leadId))).code).toBe("not_found");
  expect((await failure(LeadsMore.delete(987654))).code).toBe("not_found");
  expect((await failure(Campaigns.deleteUnifiedCampaign(987654))).code).toBe("not_found");
  expect((await failure(ConversationActions.deleteQuickTemplate({ id: 987654 }))).code).toBe(
    "not_found"
  );
});

test("int8 item2: a positional id of the wrong type is invalid_request with a wrong-type message", async () => {
  await Host.start(dir);
  for (const bad of ["7", true, null, { id: 1 }, [1], 1.5, Number.NaN]) {
    for (const call of [
      () => LeadsMore.delete(bad as never),
      () => Campaigns.deleteUnifiedCampaign(bad as never),
      () => Templates.deleteEmailTemplate(bad as never),
      () => Ai.reviewProposal(bad as never, "APPROVED", "qa")
    ]) {
      const error = await failure(call());
      expect(error.code).toBe("invalid_request");
      expect(error.message).toBe(WRONG_TYPE);
    }
  }
  const range = await failure(LeadsMore.delete(-1));
  expect(range.code).toBe("invalid_request");
  expect(range.message).toBe("id is out of range");
});

test("int8 item3: a WhatsApp campaign without an account says an account is needed", async () => {
  await Host.start(dir);
  const error = await failure(
    Campaigns.createWhatsappCampaign({ name: "QA int8 no account", assigned_account_ids: [] })
  );
  expect(error.code).toBe("invalid_request");
  expect(error.message).toContain("at least one WhatsApp account is required");
});

test("int8 item4: reviewing a proposal that is gone stays not_found", async () => {
  await Host.start(dir);
  expect((await failure(Ai.reviewProposal(987654, "APPROVED", "qa"))).code).toBe("not_found");
});
