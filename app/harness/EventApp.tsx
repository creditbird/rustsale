// A test-only entry: the whole app plus buttons that make the core's events arrive on this engine.
// (Each test's app runs on an engine of its own, and the core's events reach only the engine that
// started the core, so a test cannot deliver an event to the app from outside.)
import "../assets/fonts/Inter-Regular.ttf";
import "../assets/fonts/Inter-Medium.ttf";
import "../assets/fonts/Inter-SemiBold.ttf";
import { mount, View } from "nsh";
import { Demo } from "rustsale/native";
import { Button } from "../ui";
import { Shell } from "../Shell";

const message = (id: number, text: string) => ({
  id, direction: "inbound", contentType: "text", textContent: text, occurredAt: "2026-07-20T12:00:00.000Z", status: "received",
});

function EventApp() {
  return (
    <View class="flex-1">
      <Shell />
      <View class="absolute bottom-0 left-0 flex-row gap-1">
        <Button testID="emit-delta" size="sm" label="delta" onPress={() => Demo.emit("conversation-message-delta", { conversationId: 1, message: message(987654, "tin mới từ sự kiện") })} />
        <Button testID="emit-invalidated" size="sm" label="invalidated" onPress={() => Demo.emit("conversation-invalidated", { conversationId: 1 })} />
        <Button testID="emit-updated" size="sm" label="updated" onPress={() => Demo.emit("conversation-updated", { conversationId: 1, message: message(987655, "không được hiện") })} />
        <Button testID="emit-out-progress" size="sm" label="out-progress" onPress={() => Demo.emit("outreach-progress", { progress: 40, status: "running", message: "Đang gửi 2/5 L2P" })} />
        <Button testID="emit-out-log" size="sm" label="out-log" onPress={() => Demo.emit("outreach-log", { message: "Dòng log L2Q khi rời màn" })} />
        <Button testID="emit-out-failed" size="sm" label="out-failed" onPress={() => Demo.emit("outreach-progress", { progress: 0, status: "failed", message: "Session not found L2R" })} />
        <Button testID="emit-email-log" size="sm" label="email-log" onPress={() => Demo.emit("email-outreach-log", { message: "Successfully sent email to An (an@example.test)" })} />
        <Button testID="emit-email-progress" size="sm" label="email-progress" onPress={() => Demo.emit("email-outreach-progress", { progress: 50, status: "running", message: "Processed 1/2 leads" })} />
        <Button testID="emit-email-complete" size="sm" label="email-complete" onPress={() => Demo.emit("email-outreach-complete", { total_sent: 2, message: "Email outreach completed. 2 emails sent." })} />
        <Button testID="emit-scrape" size="sm" label="scrape" onPress={() => Demo.emit("google-maps-scrape-job-event", {})} />
      </View>
    </View>
  );
}
mount(EventApp);
