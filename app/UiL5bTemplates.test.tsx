// Headless: template folders in the Thư viện Mẫu screen — create, filter templates by folder, delete.
import { expect, open, test } from "nsh/test";
import { Host, Templates } from "rustsale/native";
import { eventually } from "./testing";

const dir = `/tmp/rustsale-v3-ui-l5b-templates-${Date.now()}`;

interface FolderRow {
  id: number;
  name: string;
}
interface TemplateRow {
  id: number;
  name: string;
}

async function openTemplates() {
  await Host.start(dir);
  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-template_center");
  await app.waitFor("id:tpl-add");
  return app;
}
const has = (app: Awaited<ReturnType<typeof openTemplates>>, text: string) => app.texts().some((t) => t.includes(text));

async function makeFolder(app: Awaited<ReturnType<typeof openTemplates>>, name: string): Promise<number> {
  await app.press("id:tpl-folders-open");
  await app.waitFor("id:tpl-folder-new-name");
  await app.type("id:tpl-folder-new-name", name);
  await app.press("id:tpl-folder-create");
  await eventually(async () => ((await Templates.folders()) as FolderRow[]).some((f) => f.name === name));
  const id = ((await Templates.folders()) as FolderRow[]).find((f) => f.name === name)!.id;
  await app.waitFor(`id:tpl-folder-row-${id}`);
  await app.press("id:tpl-folders-close");
  await app.waitFor("id:tpl-folders-close", { state: "gone" });
  return id;
}

async function makeTemplate(
  app: Awaited<ReturnType<typeof openTemplates>>,
  name: string,
  folderId: number | null,
): Promise<number> {
  await app.press("id:tpl-add");
  await app.waitFor("id:tpl-name");
  await app.type("id:tpl-name", name);
  await app.type("id:tpl-body", `Nội dung ${name}`);
  await app.press("id:tpl-folder-trigger");
  await app.press(`id:tpl-folder-item-${folderId === null ? "none" : folderId}`);
  await app.press("id:tpl-save");
  await eventually(async () => ((await Templates.list(null)) as TemplateRow[]).some((t) => t.name === name));
  return ((await Templates.list(null)) as TemplateRow[]).find((t) => t.name === name)!.id;
}

test("a folder is created and a template is saved into it", async () => {
  const stamp = Date.now();
  const app = await openTemplates();
  const folderName = `Nhóm ${stamp}`;
  const folderId = await makeFolder(app, folderName);
  await app.waitFor(`id:tpl-folder-${folderId}`);
  expect(has(app, folderName)).toBe(true);

  const templateName = `Mẫu trong nhóm ${stamp}`;
  const templateId = await makeTemplate(app, templateName, folderId);
  await app.waitFor(`id:tpl-item-${templateId}`);
  expect(has(app, folderName)).toBe(true);
});

test("choosing a folder shows only its templates", async () => {
  const stamp = Date.now();
  const app = await openTemplates();
  const folderName = `Lọc ${stamp}`;
  const folderId = await makeFolder(app, folderName);
  const inside = await makeTemplate(app, `Trong thư mục ${stamp}`, folderId);
  const outside = await makeTemplate(app, `Ngoài thư mục ${stamp}`, null);

  await app.press(`id:tpl-folder-${folderId}`);
  await app.waitFor(`id:tpl-item-${inside}`);
  await app.waitFor(`id:tpl-item-${outside}`, { state: "gone" });

  await app.press("id:tpl-folder-all");
  await app.waitFor(`id:tpl-item-${outside}`);
});

test("deleting a folder keeps its templates and returns to all templates", async () => {
  const stamp = Date.now();
  const app = await openTemplates();
  const folderName = `Xoá ${stamp}`;
  const folderId = await makeFolder(app, folderName);
  const templateName = `Mẫu còn lại ${stamp}`;
  const templateId = await makeTemplate(app, templateName, folderId);

  await app.press(`id:tpl-folder-${folderId}`);
  await app.waitFor(`id:tpl-item-${templateId}`);

  await app.press("id:tpl-folders-open");
  await app.waitFor(`id:tpl-folder-del-${folderId}`);
  await app.press(`id:tpl-folder-del-${folderId}`);
  await eventually(async () => ((await Templates.folders()) as FolderRow[]).every((f) => f.id !== folderId));
  await app.waitFor(`id:tpl-folder-row-${folderId}`, { state: "gone" });
  await app.press("id:tpl-folders-close");

  await app.waitFor(`id:tpl-folder-${folderId}`, { state: "gone" });
  await app.waitFor(`id:tpl-item-${templateId}`);
  expect(((await Templates.list(null)) as TemplateRow[]).some((t) => t.id === templateId && t.name === templateName)).toBe(true);
});
