import { Text, View } from "nsh";
import { Dialog } from "nsh/base";
import type { JSX } from "nsh";

export interface ModalProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: JSX.Element;
  /** Overrides the default 420px card for wide forms and ledgers. */
  class?: string;
  /** Blocks Escape, backdrop press and swipe-to-dismiss while true (e.g. startup gate or download). */
  preventClose?: boolean;
  /** Optional testID passed to the underlying Dialog. */
  testID?: string;
}

/** A centered dialog on the card surface with a dimmed backdrop. */
export function Modal(props: ModalProps) {
  return (
    <Dialog
      testID={props.testID}
      open={props.open}
      animation="fade"
      presentation="formSheet"
      accessibilityLabel={props.title}
      closeOnEscape={!props.preventClose}
      closeOnBackdropPress={!props.preventClose}
      closeOnSwipe={!props.preventClose}
      onOpenChange={(e) => { if (!e.open && !props.preventClose) props.onClose(); }}
      backdropClass="bg-foreground/40"
      class={props.class ?? "m-auto w-[420px] max-w-full rounded-(--radius) border border-border bg-popover p-5"}
    >
      <View class="gap-4">
        <Text class="text-lg font-semibold text-popover-foreground">{props.title}</Text>
        {props.children}
      </View>
    </Dialog>
  );
}
