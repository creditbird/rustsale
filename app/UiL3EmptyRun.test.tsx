// Headless: the Zalo screen refuses to start when the core's resolved lead list is empty,
// so the core's missing completion event for an empty partition cannot leave the UI running.
import { open, test } from "nsh/test";
import { Campaigns, Host } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l3-empty-${Date.now()}`;

test("no resolved leads shows an error and never calls start", async () => {
  const stamp = Date.now();
  const templateName = `Tpl empty ${stamp}`;
  await Host.start(dir);
  await Campaigns.createOutreachTemplate(templateName, `Nội dung ${stamp}`, null);

  const started: { imeis: string[]; templateId: number; leadIds: number[] }[] = [];
  await Host.start(dir);
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "Zalo.accounts": () => [
          {
            account_key: "acc-empty",
            display_name: "Demo Zalo",
            auth_status: "logged_in",
            runtime_status: "connected",
            last_error: null,
          },
        ],
        "LeadsMore.outreachPage": () => ({
          items: [
            {
              id: 1,
              name: `Lead empty ${stamp}`,
              company: null,
              status: "New",
              email: null,
              phone: "0901234567",
              has_zalo: 1,
              zalo_status: "VERIFIED",
              zalo_reached: 0,
            },
          ],
          total: 1,
          next_cursor: null,
        }),
        "LeadsMore.outreachIds": () => [],
        "Campaigns.startZaloOutreach": (
          imeis: string[],
          templateId: number,
          leadIds: number[],
        ) => {
          started.push({ imeis, templateId, leadIds });
          return null;
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press("id:nav-outreach");
  await app.waitFor("id:zalo-search");
  await app.waitUntil(() => app.texts().some((t) => t.includes("Đã chọn 1 / 1 lead")));
  await app.press("id:zalo-run");
  await app.waitFor("id:zalo-run-error");
  await app.waitUntil(() => app.texts().some((t) => t.includes("Không có lead nào phù hợp để gửi.")));
  if (started.length !== 0) throw new Error("startZaloOutreach was called for an empty lead list");
  await app.waitUntil(() => app.texts().some((t) => t.includes("Chưa chạy")));
});
