import type { ComponentProps } from "react"
import { Popover } from "@/components/ui/popover"

/**
 * The root of a popover that describes on hover a trigger with a click action of its own (a push, a chip that opens its
 * pull request or robot, the AI toggle, a landing node): the press does the action and never pins or closes the
 * description, which follows the pointer ("Hover descriptions" in docs/design/ui-theme.md). Base UI takes a press on the
 * trigger of a hover-opened popover as a toggle: within 500 ms of the open it pins it (it stays after the pointer
 * left), later it closes it, and the pointer that stays opens it no more. A press that opens the description on purpose
 * (the question before a push that drops remote commits) opens it itself, with `preventBaseUIHandler`.
 */
export function HoverDescription({ onOpenChange, ...props }: ComponentProps<typeof Popover>) {
  return (
    <Popover
      {...props}
      onOpenChange={(open, eventDetails) => {
        if (eventDetails.reason === "trigger-press") {
          eventDetails.cancel()
          return
        }
        onOpenChange?.(open, eventDetails)
      }}
    />
  )
}
