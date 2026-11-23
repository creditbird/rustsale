// Headless: the shared lead picker sends the category and "successful template" filters the
// core's outreach query takes, on both the Zalo and the Email screen.
import { open, test } from "nsh/test";
import { Campaigns, Categories, Host, Templates } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l2-filters-${Date.now()}`;

type Filters = Record<string, unknown>;
type App = Awaited<ReturnType<typeof open>>;

async function openWithCalls(route: "outreach" | "email_outreach", waitFor: string) {
  await Host.start(dir);
  const calls: Filters[] = [];
  const app = await open("./App.tsx", {
    modules: {
      "rustsale/native": {
        "LeadsMore.outreachPage": (filters: unknown) => {
          calls.push(filters as Filters);
          return { items: [], total: 0, next_cursor: null };
        },
      },
    },
  });
  await app.setViewport(1280, 800);
  await app.press(`id:nav-${route}`);
  await app.waitFor(`id:${waitFor}`);
  await app.wait(400);
  return { app, calls };
}

async function choose(app: App, trigger: string, item: string) {
  await app.press(`id:${trigger}`);
  await app.press(`id:${item}`);
}

test("the Zalo picker filters by category and by successful template", async () => {
  const stamp = Date.now();
  const categoryName = `Cat Z ${stamp}`;
  const firstTemplate = `Tpl Z1 ${stamp}`;
  const wantedTemplate = `Tpl Z2 ${stamp}`;
  await Host.start(dir);
  const categoryId = (await Categories.create(categoryName)) as number;
  await Campaigns.createOutreachTemplate(firstTemplate, `Nội dung ${stamp}`, null);
  const templateId = (await Campaigns.createOutreachTemplate(wantedTemplate, `Nội dung ${stamp}`, null)) as number;

  const { app, calls } = await openWithCalls("outreach", "zalo-search");
  await choose(app, "zalo-category-trigger", `zalo-category-item-${categoryId}`);
  await app.waitUntil(() => calls.some((f) => f.categoryId === categoryId));

  await app.press("id:zalo-seg-template_success");
  await app.waitFor("id:zalo-seg-template");
  await choose(app, "zalo-seg-template-trigger", `zalo-seg-template-item-${templateId}`);
  await app.waitUntil(() => calls.some(
    (f) => f.categoryId === categoryId
      && f.segmentation === "template_success"
      && f.filterTemplateId === templateId,
  ));
});

test("the Email picker filters by category and by successful template", async () => {
  const stamp = Date.now();
  const categoryName = `Cat E ${stamp}`;
  const firstTemplate = `Tpl E1 ${stamp}`;
  const wantedTemplate = `Tpl E2 ${stamp}`;
  await Host.start(dir);
  const categoryId = (await Categories.create(categoryName)) as number;
  await Templates.createEmailTemplate(firstTemplate, `Tiêu đề ${stamp}`, `Nội dung ${stamp}`);
  const templateId = (await Templates.createEmailTemplate(wantedTemplate, `Tiêu đề ${stamp}`, `Nội dung ${stamp}`)) as number;

  const { app, calls } = await openWithCalls("email_outreach", "email-search");
  await choose(app, "email-category-trigger", `email-category-item-${categoryId}`);
  await app.waitUntil(() => calls.some((f) => f.categoryId === categoryId));

  await app.press("id:email-seg-template_success");
  await app.waitFor("id:email-seg-template");
  await choose(app, "email-seg-template-trigger", `email-seg-template-item-${templateId}`);
  await app.waitUntil(() => calls.some(
    (f) => f.categoryId === categoryId
      && f.segmentation === "template_success"
      && f.filterTemplateId === templateId,
  ));
});
