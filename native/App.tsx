import { mount, signal, Text, View } from "nsh";
import { Host, Leads } from "rustsale/native";

// P1 smoke screen: starts the core and shows the first page of leads.
// The real screens come in P2; this proves TSX reaches the core.
function App() {
  const status = signal("Starting…");
  const names = signal<string[]>([]);

  (async () => {
    try {
      const info = await Host.start();
      const page = (await Leads.page({ sort: "name" }, null, 50)) as { items: { name: string }[]; total: number };
      names.set(page.items.map((lead) => lead.name));
      status.set(`Core on port ${info.localApiPort}: ${page.total} leads`);
    } catch (error) {
      status.set(`Core failed: ${(error as Error).message}`);
    }
  })();

  return (
    <View style={{ flex: 1, padding: 16, gap: 8 }}>
      <Text class="text-foreground" style={{ fontSize: 20 }}>{status()}</Text>
      {names().map((name) => (
        <Text class="text-foreground">{name}</Text>
      ))}
    </View>
  );
}

mount(App);
