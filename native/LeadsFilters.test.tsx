// `nsh test` runs this against the real native module on a throwaway data folder: the advanced
// lead filters the table sends (owner, value range, Zalo state, outreach state) and the owners list.
import { expect, test } from "nsh/test";
import { Host, Leads } from "rustsale/native";

const dir = `/tmp/rustsale-v3-nsh-test-lead-filters-${Date.now()}`;

interface Page<T> {
  items: T[];
  total: number;
}

async function add(name: string, fields: Record<string, unknown>): Promise<number> {
  return (await Leads.add({ name, status: "New", ...fields })) as number;
}

test("the owners export is distinct, sorted and skips blank owners", async () => {
  await Host.start(dir);
  await add(`Owner A ${Date.now()}`, { owner: "Binh" });
  await add(`Owner B ${Date.now()}`, { owner: "an" });
  await add(`Owner C ${Date.now()}`, { owner: "an" });
  const owners = (await Leads.owners()) as string[];
  expect(owners).toContain("an");
  expect(owners).toContain("Binh");
  expect(owners.indexOf("an")).toBeLessThan(owners.indexOf("Binh"));
  expect(owners.filter((owner) => owner === "an").length).toBe(1);
  expect(owners.every((owner) => owner.trim() !== "")).toBe(true);
});

test("the owner and value filters narrow the page in the core", async () => {
  await Host.start(dir);
  const stamp = Date.now();
  const cheap = await add(`Loc re ${stamp}`, { owner: `Chi ${stamp}`, value: 5_000_000 });
  const rich = await add(`Loc dat ${stamp}`, { owner: `Chi ${stamp}`, value: 80_000_000 });
  const other = await add(`Loc khac ${stamp}`, { owner: `Dung ${stamp}`, value: 80_000_000 });

  const byOwner = (await Leads.page({ owner: `chi ${stamp}` }, null, 50)) as Page<{ id: number }>;
  expect(byOwner.total).toBe(2);
  expect(byOwner.items.map((lead) => lead.id).sort()).toEqual([cheap, rich].sort());

  const byValue = (await Leads.page(
    { owner: `Chi ${stamp}`, minValue: 10_000_000, maxValue: 90_000_000 },
    null,
    50
  )) as Page<{ id: number }>;
  expect(byValue.total).toBe(1);
  expect(byValue.items[0].id).toBe(rich);
  expect(byValue.items.some((lead) => lead.id === other)).toBe(false);

  const notFound = (await Leads.page({ owner: `Khong ai ${stamp}` }, null, 50)) as Page<{ id: number }>;
  expect(notFound.total).toBe(0);
});

test("a Zalo state outside the three values is invalid_request", async () => {
  await Host.start(dir);
  try {
    await Leads.page({ zalo: "bogus" }, null, 50);
  } catch (error) {
    const err = error as { code?: unknown };
    expect(err.code).toBe("invalid_request");
    return;
  }
  throw new Error("an unknown Zalo state must be refused");
});
