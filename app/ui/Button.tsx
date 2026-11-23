import { Text } from "nsh";
import { Button as BaseButton } from "nsh/base";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "destructive";
export type ButtonSize = "sm" | "md";

const VARIANT: Record<ButtonVariant, string> = {
  primary: "bg-primary active:bg-primary/80 hover:bg-primary/90",
  secondary: "border border-border bg-card hover:bg-accent active:bg-accent/70",
  ghost: "hover:bg-accent active:bg-accent/70",
  destructive: "bg-destructive active:bg-destructive/80 hover:bg-destructive/90",
};
const LABEL: Record<ButtonVariant, string> = {
  primary: "text-primary-foreground",
  secondary: "text-foreground",
  ghost: "text-foreground",
  destructive: "text-primary-foreground",
};
const SIZE: Record<ButtonSize, string> = { sm: "h-8 px-3", md: "h-9 px-4" };

export interface ButtonProps {
  label: string;
  onPress: () => void;
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  testID?: string;
}

/** The app's button: variants and sizes, never colors. */
export function Button(props: ButtonProps) {
  // Read inside the JSX expressions below, not here: the component runs once, so a value read in the
  // body would freeze the first variant/size and a chosen-state button would never change colour.
  return (
    <BaseButton
      testID={props.testID}
      disabled={props.disabled}
      accessibilityLabel={props.label}
      onPress={() => props.onPress()}
      class={`flex-row items-center justify-center rounded-(--radius) focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 ${SIZE[props.size ?? "md"]} ${VARIANT[props.variant ?? "secondary"]}`}
    >
      <Text class={`text-sm font-medium ${LABEL[props.variant ?? "secondary"]}`}>{props.label}</Text>
    </BaseButton>
  );
}
