import { Text, View } from "nsh";
import { Dialog } from "nsh/base";
import type { JSX } from "nsh";

export interface ModalProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: JSX.Element;
}

/** A centered dialog on the card surface with a dimmed backdrop. */
export function Modal(props: ModalProps) {
  return (
    <Dialog
      open={props.open}
      animation="fade"
      presentation="formSheet"
      accessibilityLabel={props.title}
      onOpenChange={(e) => { if (!e.open) props.onClose(); }}
      backdropClass="bg-foreground/40"
      class="m-auto w-[420px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
    >
      <View class="gap-4">
        <Text class="text-lg font-semibold text-popover-foreground">{props.title}</Text>
        {props.children}
      </View>
    </Dialog>
  );
}
