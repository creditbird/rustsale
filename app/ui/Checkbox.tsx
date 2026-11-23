import { Text } from "nsh";
import { Checkbox as Base } from "nsh/base";

export interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  testID?: string;
}

/** A checkbox with the theme's box; the label is part of the press target. */
export function Checkbox(props: CheckboxProps) {
  return (
    <Base
      testID={props.testID}
      checked={props.checked}
      onChange={(e) => props.onChange(e.checked)}
      class="flex-row items-center gap-2"
      indicatorClass="h-4 w-4 items-center justify-center rounded-sm border border-input bg-background checked:border-primary checked:bg-primary"
    >
      {props.label ? <Text class="text-sm text-foreground">{props.label}</Text> : null}
    </Base>
  );
}
