import { Text, View } from "nsh";
import { Toast } from "./ui";
import { Button as Base } from "nsh/base";
import { NAV, route, type RouteId } from "./stores/nav";
import { LeadsScreen } from "./screens/LeadsScreen";
import { ConversationScreen } from "./screens/ConversationScreen";
import { TemplatesScreen } from "./screens/TemplatesScreen";
import { ScraperScreen } from "./screens/ScraperScreen";
import { CategoriesScreen } from "./screens/CategoriesScreen";
import { ContactsScreen } from "./screens/ContactsScreen";
import { OutreachScreen } from "./screens/OutreachScreen";
import { EmailOutreachScreen } from "./screens/EmailOutreachScreen";
import { WhatsAppOutreachScreen } from "./screens/WhatsAppOutreachScreen";
import { AiApprovalScreen } from "./screens/AiApprovalScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { Placeholder } from "./screens/Placeholder";

function SidebarItem(props: { id: (typeof NAV)[number]["id"]; label: string; glyph: string }) {
  return (
    <Base
      testID={`nav-${props.id}`}
      selected={route() === props.id}
      accessibilityLabel={props.label}
      onPress={() => route.set(props.id)}
      class="group flex-row items-center gap-3 rounded-(--radius) px-3 py-2 hover:bg-sidebar-accent selected:bg-sidebar-accent"
    >
      <Text class="w-4 text-center text-sm text-muted-foreground group-selected:text-sidebar-primary">{props.glyph}</Text>
      <Text class="truncate text-sm text-sidebar-foreground group-selected:font-medium">{props.label}</Text>
    </Base>
  );
}

function pick(r: RouteId) {
  if (r === "leads") return <LeadsScreen />;
  if (r === "conversation") return <ConversationScreen />;
  if (r === "template_center") return <TemplatesScreen />;
  if (r === "scraper") return <ScraperScreen />;
  if (r === "categories") return <CategoriesScreen />;
  if (r === "contacts") return <ContactsScreen />;
  if (r === "outreach") return <OutreachScreen />;
  if (r === "email_outreach") return <EmailOutreachScreen />;
  if (r === "whatsapp_outreach") return <WhatsAppOutreachScreen />;
  if (r === "ai_approval") return <AiApprovalScreen />;
  if (r === "settings") return <SettingsScreen />;
  return <Placeholder title={NAV.find((n) => n.id === r)?.label ?? ""} />;
}

/** The app frame: sidebar on the left, the routed screen on the right. */
export function Shell() {
  return (
    <View class="flex-1 flex-row bg-background">
      <View testID="sidebar" class="w-60 gap-1 border-r border-sidebar-border bg-sidebar p-3">
        <View class="px-3 py-3"><Text class="text-base font-semibold text-sidebar-foreground">RustSale</Text></View>
        <Each of={NAV} key={(n) => n.id}>
          {(n) => <SidebarItem id={n().id} label={n().label} glyph={n().glyph} />}
        </Each>
      </View>
      <View class="flex-1">{pick(route())}</View>
      <Toast />
    </View>
  );
}
