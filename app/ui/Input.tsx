import { Input as BaseInput } from "nsh/base";

export interface InputProps {
  value: string;
  onValue: (value: string) => void;
  placeholder?: string;
  testID?: string;
  onSubmit?: () => void;
}

/** A single-line text field on the theme's input tokens. */
export function Input(props: InputProps) {
  return (
    <BaseInput
      testID={props.testID}
      value={props.value}
      placeholder={props.placeholder}
      returnKeyType="search"
      onChange={({ value }) => props.onValue(value)}
      onSubmit={() => props.onSubmit?.()}
      class="h-9 rounded-(--radius) border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring"
    />
  );
}
