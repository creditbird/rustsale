import { Text, View } from "nsh";

export type BadgeTone = "neutral" | "success" | "warning" | "danger";
const TONE: Record<BadgeTone, string> = {
  neutral: "bg-muted",
  success: "bg-success/15",
  warning: "bg-warning/15",
  danger: "bg-destructive/15",
};
const TEXT: Record<BadgeTone, string> = {
  neutral: "text-muted-foreground",
  success: "text-success",
  warning: "text-warning",
  danger: "text-destructive",
};

/** A small status pill; neutral unless the status carries meaning. */
export function Badge(props: { label: string; tone?: BadgeTone }) {
  const tone = props.tone ?? "neutral";
  return (
    <View class={`self-start rounded-full px-2 py-0.5 ${TONE[tone]}`}>
      <Text class={`text-xs font-medium ${TEXT[tone]}`}>{props.label}</Text>
    </View>
  );
}
