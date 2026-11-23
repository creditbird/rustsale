import { Input as BaseInput } from "nsh/base";

export interface InputProps {
  value: string;
  onValue: (value: string) => void;
  placeholder?: string;
  testID?: string;
  onSubmit?: () => void;
  multiline?: boolean;
  class?: string;
  /** Takes the keyboard focus once shown: a field in a dialog that opens to be typed in. */
  autoFocus?: boolean;
}

/** A single-line text field on the theme's input tokens. */
export function Input(props: InputProps) {
  return (
    <BaseInput
      testID={props.testID}
      value={props.value}
      placeholder={props.placeholder}
      autoFocus={props.autoFocus}
      multiline={props.multiline}
      returnKeyType={props.multiline ? "default" : "search"}
      onChange={({ value }) => props.onValue(value)}
      onSubmit={() => props.onSubmit?.()}
      class={props.class ?? "h-9 rounded-(--radius) border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"}
    />
  );
}
