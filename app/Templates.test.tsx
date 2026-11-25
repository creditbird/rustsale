import { expect, open, test } from "nsh/test";
import { eventually } from "./testing";
import { Host, Templates } from "rustsale/native";

const dir = `/tmp/rustsale-v3-ui2-templates-${Date.now()}`;

async function openTemplates() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-template_center");
  await app.waitFor("id:tpl-add");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openTemplates>>, s: string) => app.texts().some((t) => t.includes(s));

test("saving without a name or content shows a validation error and writes nothing", async () => {
  const app = await openTemplates();
  await app.press("id:tpl-add");
  await app.press("id:tpl-save");
  await app.waitFor("id:tpl-error");
  const before = ((await Templates.list(null)) as unknown[]).length;
  await app.press("id:tpl-save");
  expect(((await Templates.list(null)) as unknown[]).length).toBe(before);
});

test("create, edit and delete go through the core", async () => {
  const app = await openTemplates();
  await app.press("id:tpl-add");
  await app.type("id:tpl-name", "Chào khách");
  await app.type("id:tpl-body", "Chào {{name}}");
  await app.press("id:tpl-save");
  await app.waitUntil(() => has(app, "Chào khách"));
  const list = (await Templates.list(null)) as { id: number; name: string }[];
  const mine = list.find((t) => t.name === "Chào khách");
  if (!mine) throw new Error("the saved template is not in the core");
  await app.press(`id:tpl-edit-${mine.id}`);
  await app.type("id:tpl-name", " v2");
  await app.press("id:tpl-save");
  await app.waitUntil(() => has(app, "v2"));
  await app.press(`id:tpl-del-${mine.id}`);
  await app.press("id:tpl-confirm-del");
  await eventually(async () => !((await Templates.list(null)) as { id: number }[]).some((t) => t.id === mine.id));
});
