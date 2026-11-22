// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder. Every native failure crossing into TSX must carry a stable `code`.
import { expect, test } from "nsh/test";
import {
  Ai,
  Campaigns,
  ConversationNetwork,
  Facebook,
  Host,
  ImportLeads,
  Templates,
  Zalo
} from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-errcode-${Date.now()}`;

async function expectError(promise: Promise<unknown>, code: string): Promise<void> {
  try {
    await promise;
  } catch (error) {
    const err = error as { code?: unknown; message?: unknown };
    expect(err.code).toBe(code);
    expect(String(err.message).startsWith(`${code}: `)).toBe(false);
    expect(String(err.message).length).toBeGreaterThan(0);
    return;
  }
  throw new Error(`expected a rejection with code ${code}`);
}

test("native errors reach TSX with a code and a clean message", async () => {
  await Host.start(dir);
  await expectError(Ai.leadEvidenceContext(4_294_967_297), "invalid_request");
  await expectError(Zalo.verifySession("qa-errcode-missing"), "not_found");
  await expectError(Templates.testSmtpConnection("127.0.0.1", 1), "internal");
  await expectError(Templates.deleteSmtpServer(4_294_967_297), "invalid_request");
  await expectError(Templates.deleteEmailTemplate(4_294_967_297), "invalid_request");
  await expectError(Campaigns.whatsappCampaign(4_294_967_297), "not_found");
  await expectError(Facebook.syncConversations("qa-errcode-missing-page"), "not_found");
  await expectError(
    ConversationNetwork.startDirect({
      channelAccountId: 424242,
      phoneOrExternalId: "0900000000",
      displayName: null,
      initialMessage: "qa errcode"
    }),
    "not_found"
  );
  await expectError(ImportLeads.inspectFile("/tmp/qa-errcode-missing.csv", null), "not_found");
});
