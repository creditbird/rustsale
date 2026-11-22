import { Text, View } from "nsh";
import { Button } from "./Button";
import { dismiss, toast } from "../stores/toast";

const TONE = {
  info: "border-border bg-card",
  success: "border-success bg-card",
  warning: "border-warning bg-card",
  danger: "border-destructive bg-card",
} as const;

/** The toast layer: one message at the bottom right, with an optional retry. */
export function Toast() {
  return (
    <If when={toast()}>
      {(t) => (
        <View testID="toast" class="absolute bottom-6 right-6 max-w-[420px]">
          <View class={`flex-row items-center gap-3 rounded-(--radius) border px-4 py-3 ${TONE[t().tone]}`}>
            <Text testID="toast-text" class="flex-1 text-sm text-foreground">{t().text}</Text>
            <If when={t().retry}>
              {(retry) => <Button testID="toast-retry" size="sm" label="Thử lại" onPress={() => { dismiss(); retry()(); }} />}
            </If>
          </View>
        </View>
      )}
    </If>
  );
}
