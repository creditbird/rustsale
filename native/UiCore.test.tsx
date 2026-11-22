// `nsh test` runs this in a headless runner linked with the crate: the module is the real one, on a
// throwaway data folder. Every case is one fault Alice's UI2 QA found; each name starts with the
// item of the ui2-core brief.
import { expect, test } from "nsh/test";
import { ConversationActions, Deals, GoogleMaps, Host, Leads, Templates } from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-ui2-core-${Date.now()}`;

async function expectError(
  promise: Promise<unknown>,
  code: string,
  message?: string
): Promise<void> {
  try {
    await promise;
  } catch (error) {
    const err = error as { code?: unknown; message?: unknown };
    expect(err.code).toBe(code);
    if (message !== undefined) expect(err.message).toBe(message);
    return;
  }
  throw new Error(`expected a rejection with code ${code}`);
}

test("ui2 item1: sendMessage to a missing conversation is not_found with a clean message", async () => {
  await Host.start(dir);
  await expectError(
    ConversationActions.sendMessage({
      conversationId: 424242,
      channelAccountId: 1,
      text: "hello"
    }),
    "not_found",
    "conversation 424242 not found"
  );
});

test("ui2 item2: a template with an unknown channel is invalid_request", async () => {
  await Host.start(dir);
  await expectError(
    Templates.createTemplate({
      name: "Bogus channel",
      channel: "bogus",
      content: "hi"
    }),
    "invalid_request"
  );
});

test("ui2 item3: editing a row that no longer exists is not_found, never a silent success", async () => {
  await Host.start(dir);
  await expectError(Leads.updateStatus(987654, "Qualified"), "not_found", "lead 987654 not found");
  await expectError(Deals.updateStage(987654, "Won"), "not_found", "deal 987654 not found");
  await expectError(
    Templates.updateTemplate({
      id: 987654,
      folder_id: null,
      name: "Gone",
      channel: "zalo",
      stage: null,
      subject: null,
      content: "x",
      media_url: null,
      ai_prompt: null,
      tags_json: null
    }),
    "not_found",
    "template 987654 not found"
  );
  await expectError(
    Templates.updateEmailTemplate(987654, "Gone", "s", "c"),
    "not_found",
    "email template 987654 not found"
  );
});

test("ui2 item6: GoogleMaps.runtimeStatus without the sidecar is unavailable", async () => {
  await Host.start(dir);
  await expectError(GoogleMaps.runtimeStatus(), "unavailable");
});
