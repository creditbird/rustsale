// Headless: the "Chưa xếp" template filter lists only templates without a folder.
import { expect, open, test } from "nsh/test";
import { Host, Templates } from "rustsale/native";
import { eventually } from "./testing";

const dir = `/tmp/rustsale-v3-ui-l5c-unfiled-${Date.now()}`;

interface TemplateRow {
  id: number;
  name: string;
  folder_id: number | null;
}

const rows = async () => (await Templates.list(null)) as TemplateRow[];

test("the unfiled filter shows only templates without a folder", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const folderId = (await Templates.createFolder(`Nhóm ${stamp}`, null)) as number;
  const filed = (await Templates.createTemplate({
    folder_id: folderId,
    name: `Trong nhóm ${stamp}`,
    channel: "zalo",
    content: "Nội dung",
  })) as number;
  const loose = (await Templates.createTemplate({
    name: `Chưa xếp ${stamp}`,
    channel: "zalo",
    content: "Nội dung",
  })) as number;

  const onlyLoose = (await Templates.list({ unfiled: true })) as TemplateRow[];
  expect(onlyLoose.some((row) => row.id === loose)).toBe(true);
  expect(onlyLoose.some((row) => row.id === filed)).toBe(false);

  const app = await open("./App.tsx");
  await app.setViewport(1280, 800);
  await app.press("id:nav-template_center");
  await app.waitFor("id:tpl-folder-unfiled");
  await app.waitFor(`id:tpl-item-${filed}`);
  await app.waitFor(`id:tpl-item-${loose}`);

  await app.press("id:tpl-folder-unfiled");
  await app.waitFor(`id:tpl-item-${loose}`);
  await app.waitFor(`id:tpl-item-${filed}`, { state: "gone" });

  await app.press("id:tpl-folder-all");
  await app.waitFor(`id:tpl-item-${filed}`);
  await app.waitFor(`id:tpl-item-${loose}`);
  await eventually(async () => (await rows()).length > 0);
});
