import { Scroll, cleanup, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Host } from "rustsale/native";
import { Toast } from "./ui";
import { UpdateModal } from "./components/UpdateModal";
import { LicenseModal } from "./components/LicenseModal";
import { NAV, route, type RouteId } from "./stores/nav";
import {
  answerCoreUpdaterRequest,
  checkForAppUpdate,
  openUpdateModal,
  updaterState,
} from "./stores/updater";
import {
  cleanupLicenseTimers,
  licenseConfigured,
  licenseInitials,
  licenseUser,
  licenseVerification,
  licenseVerified,
  openLicenseModal,
  runStartupLicenseGate,
} from "./stores/license";
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
import { PipelineScreen } from "./screens/PipelineScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { DashboardScreen } from "./screens/DashboardScreen";
import { CampaignCenterScreen } from "./screens/CampaignCenterScreen";
import { Placeholder } from "./screens/Placeholder";

function SidebarItem(props: { id: (typeof NAV)[number]["id"]; label: string; glyph: string }) {
  return (
    <Base
      testID={`nav-${props.id}`}
      selected={route() === props.id}
      accessibilityLabel={props.label}
      onPress={() => route.set(props.id)}
      class="group flex-row items-center gap-3 rounded-(--radius) px-3 py-1.5 hover:bg-sidebar-accent selected:bg-sidebar-accent"
    >
      <Text class="w-4 text-center text-sm text-muted-foreground group-selected:text-sidebar-primary">{props.glyph}</Text>
      <Text class="truncate text-sm text-sidebar-foreground group-selected:font-medium">{props.label}</Text>
    </Base>
  );
}

function pick(r: RouteId) {
  if (r === "dashboard") return <DashboardScreen />;
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
  if (r === "pipeline") return <PipelineScreen />;
  if (r === "settings") return <SettingsScreen />;
  if (r === "campaign_center") return <CampaignCenterScreen />;
  return <Placeholder title={NAV.find((n) => n.id === r)?.label ?? ""} />;
}

function hasUpdateBadge(): boolean {
  const s = updaterState().status;
  return (
    updaterState().availableVersion !== null &&
    (s === "available" || s === "downloading" || s === "downloaded")
  );
}

function updateBadgeLabel(): string {
  const s = updaterState();
  if (s.status === "downloaded") return "Cài lại";
  if (s.status === "downloading") return `${s.progress.percentage}%`;
  return `v${s.availableVersion ?? ""}`;
}

function licenseStatusSubtitle(): string {
  const user = licenseUser();
  const verification = licenseVerification();
  if (user && licenseVerified()) {
    return verification?.plan_id
      ? `${verification.plan_id.toUpperCase()} • Đã kích hoạt`
      : "Đã kích hoạt";
  }
  if (licenseConfigured() && !licenseVerified()) {
    return "Cần kích hoạt";
  }
  return "HWID & Bản quyền";
}

/** The app frame: sidebar on the left, the routed screen on the right. */
export function Shell() {
  // Global updater bridge so core requests work regardless of which tab is mounted.
  const stopUpdater = Host.onUpdaterRequest((id, op) => {
    void answerCoreUpdaterRequest(id, op);
  });

  // Non-blocking background update check after 30 seconds (matches v2 +layout.svelte).
  const updateTimer = setTimeout(() => {
    void checkForAppUpdate({ silent: true, openOnAvailable: false });
  }, 30_000);

  cleanup(() => {
    stopUpdater();
    clearTimeout(updateTimer);
    cleanupLicenseTimers();
  });

  // Startup license gate (runs only when a license server is configured).
  void runStartupLicenseGate();

  return (
    <View class="flex-1 flex-row bg-background">
      <View testID="sidebar" class="w-60 gap-2 border-r border-sidebar-border bg-sidebar p-3">
        <View class="flex-row items-center justify-between gap-2 px-2 py-1">
          <View class="min-w-0 flex-1">
            <View class="flex-row items-center gap-1.5">
              <Text class="text-base font-semibold text-sidebar-foreground">RustSale</Text>
              <Text testID="sidebar-version" class="font-mono text-xs text-muted-foreground">
                {`v${updaterState().currentVersion}`}
              </Text>
            </View>
          </View>
          <If when={hasUpdateBadge()}>
            <Base
              testID="sidebar-update-badge"
              accessibilityLabel="Có bản cập nhật mới"
              onPress={openUpdateModal}
              class="rounded-full border border-primary/40 bg-primary/15 px-2 py-0.5 hover:bg-primary/25"
            >
              <Text class="text-xs font-semibold text-primary">{updateBadgeLabel()}</Text>
            </Base>
          </If>
        </View>

        <Scroll class="flex-1">
          <View class="gap-0.5">
            <Each of={NAV} key={(n) => n.id}>
              {(n) => <SidebarItem id={n().id} label={n().label} glyph={n().glyph} />}
            </Each>
          </View>
        </Scroll>

        <View class="border-t border-sidebar-border pt-2">
          <Base
            testID="sidebar-license-btn"
            accessibilityLabel="Bản quyền & HWID"
            onPress={openLicenseModal}
            class="flex-row items-center gap-2.5 rounded-(--radius) px-2 py-1.5 hover:bg-sidebar-accent"
          >
            <View class="h-7 w-7 items-center justify-center rounded-full border border-success/30 bg-success/15">
              <Text class="text-xs font-semibold text-success">
                {licenseInitials(licenseUser()?.email)}
              </Text>
            </View>
            <View class="min-w-0 flex-1">
              <Text class="truncate text-xs font-medium text-sidebar-foreground">
                {licenseUser()?.email ?? "Bản quyền & HWID"}
              </Text>
              <Text testID="sidebar-license-status" class="truncate text-xs text-muted-foreground">
                {licenseStatusSubtitle()}
              </Text>
            </View>
          </Base>
        </View>
      </View>

      <View class="flex-1">{pick(route())}</View>
      <UpdateModal />
      <LicenseModal />
      <Toast />
    </View>
  );
}
