import { Text, View } from "nsh";
import { Button } from "./Button";

/** A grey placeholder block shown while data loads. */
export function Skeleton(props: { class?: string }) {
  return <View class={`rounded-(--radius) bg-muted ${props.class ?? "h-4 w-full"}`} />;
}

/** Centered message when there is nothing to show. */
export function EmptyState(props: { title: string; hint?: string }) {
  return (
    <View testID="empty-state" class="flex-1 items-center justify-center gap-1 p-8">
      <Text class="text-base font-medium text-foreground">{props.title}</Text>
      {props.hint ? <Text class="text-sm text-muted-foreground">{props.hint}</Text> : null}
    </View>
  );
}

/** Centered failure with a retry; the message is the user's, not a stack. */
export function ErrorState(props: { message: string; onRetry: () => void }) {
  return (
    <View testID="error-state" class="flex-1 items-center justify-center gap-3 p-8">
      <Text class="text-base font-medium text-destructive">Đã xảy ra lỗi</Text>
      <Text class="text-sm text-muted-foreground">{props.message}</Text>
      <Button label="Thử lại" onPress={props.onRetry} />
    </View>
  );
}
