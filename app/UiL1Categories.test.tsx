// Headless: the Categories screen against the real native module.
// The file name sorts it after the screens that prepare the shared demo folder (see screen-port-plan §5).
import { open, test } from "nsh/test";
import { eventually } from "./testing";
import { Categories, Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui-l1-categories-${Date.now()}`;

async function openCategories() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-categories");
  await app.waitFor("id:cat-create");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openCategories>>, s: string) => app.texts().some((t) => t.includes(s));

async function findCategory(name: string): Promise<{ id: number; name: string } | undefined> {
  return ((await Categories.list()) as { id: number; name: string }[]).find((c) => c.name === name);
}

test("a category is created and deleted through the core", async () => {
  const name = `Nhóm tạo ${Date.now()}`;
  const app = await openCategories();
  await app.press("id:cat-create");
  await app.press("id:cat-save");
  await app.waitFor("id:cat-error");
  await app.type("id:cat-name", name);
  await app.press("id:cat-save");
  await app.waitUntil(() => has(app, name));
  let category: { id: number; name: string } | undefined;
  await eventually(async () => {
    category = await findCategory(name);
    return category !== undefined;
  });
  if (!category) throw new Error("the created category is not in the core");
  await app.press(`id:cat-open-${category.id}`);
  await app.waitFor("id:cat-detail-name");
  await app.press(`id:cat-del-${category.id}`);
  await app.press("id:cat-confirm-del");
  await eventually(async () => (await findCategory(name)) === undefined);
  await app.waitUntil(() => !has(app, name));
});

test("a lead is added to and removed from a category", async () => {
  const stamp = Date.now();
  const categoryName = `Nhóm lead ${stamp}`;
  const leadName = `Lead danh mục ${stamp}`;
  await Host.start(dir);
  const leadId = (await Leads.add({ name: leadName, status: "New" })) as number;
  const app = await openCategories();
  await app.press("id:cat-create");
  await app.type("id:cat-name", categoryName);
  await app.press("id:cat-save");
  await app.waitUntil(() => has(app, categoryName));
  let category: { id: number; name: string } | undefined;
  await eventually(async () => {
    category = await findCategory(categoryName);
    return category !== undefined;
  });
  if (!category) throw new Error("the created category is not in the core");
  await app.press(`id:cat-open-${category.id}`);
  await app.waitFor("id:cat-detail-name");
  const inCategory = async () =>
    ((await Categories.leads(category!.id)) as { id: number }[]).some((l) => l.id === leadId);
  await app.type("id:cat-add-search", leadName);
  await app.waitFor(`id:cat-add-${leadId}`);
  await app.press(`id:cat-add-${leadId}`);
  await eventually(inCategory);
  await app.waitFor(`id:cat-lead-${leadId}`);
  await app.press(`id:cat-remove-${leadId}`);
  await eventually(async () => !(await inCategory()));
});

test("a category deleted elsewhere disappears when the list refreshes", async () => {
  const name = `Nhóm cũ ${Date.now()}`;
  await Host.start(dir);
  const id = (await Categories.create(name)) as number;
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-categories");
  await app.waitUntil(() => has(app, name));
  await Categories.delete(id);
  await app.press(`id:cat-del-${id}`);
  await app.press("id:cat-confirm-del");
  await app.waitUntil(() => !has(app, name));
});
