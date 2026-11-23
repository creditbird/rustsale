import { Text, View } from "nsh";
import { EmptyState } from "../ui";

/** A screen not ported yet; it keeps the route reachable. */
export function Placeholder(props: { title: string }) {
  return (
    <View class="flex-1">
      <View class="p-6"><Text class="text-xl font-semibold text-foreground">{props.title}</Text></View>
      <EmptyState title="Đang chuyển sang v3" hint="Màn này sẽ được port từ bản Svelte." />
    </View>
  );
}
