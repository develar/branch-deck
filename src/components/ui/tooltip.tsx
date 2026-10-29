// local customisation: `role="tooltip"` on the popup; `disableHoverablePopup` by default: a text tooltip closes when the
// pointer leaves its trigger and never blocks what is under it (a tooltip over the submit of the form above it stayed
// open while hovered and swallowed the click); `lazy` on `Tooltip`: the root mounts at the first hover or focus of its
// trigger, a plain element until then (`useLazyRoot`), for the many tooltips of a table row; it closes when its view of
// the main window hides (`useCloseWhenViewHides`), and its hover opens it only with a move of the pointer
// (`hoverOpensOnMove`); `controlKey` on `Tooltip`, the trigger named as the control of its root (`useControlRef`)
"use client"

import { use } from "react"
import { Tooltip as TooltipPrimitive } from "@base-ui/react/tooltip"
import { cn } from "cn"
import { useCloseWhenViewHides } from "@/hooks/useCloseWhenViewHides"
import { ControlKeyContext, useControlKey, useControlRef } from "@/hooks/useControlKey"
import { childNodes, LazyRootContext, PlainTrigger, useLazyRoot } from "@/hooks/useLazyRoot"
import { hoverOpensOnMove } from "@/lib/pointer-moves"

function TooltipProvider({ delay = 0, ...props }: TooltipPrimitive.Provider.Props) {
  return <TooltipPrimitive.Provider data-slot="tooltip-provider" delay={delay} {...props} />
}

/** The props of a tooltip trigger that only Base UI reads: a plain trigger leaves them out */
const TOOLTIP_TRIGGER_ONLY = ["delay", "closeDelay", "closeOnClick", "handle", "payload", "disabled"]
const CLOSED_TOOLTIP_TRIGGER = { "data-slot": "tooltip-trigger" }

/**
 * `lazy`: the root mounts at the first hover or focus of its trigger, or while `open` (`useLazyRoot`); until then the
 * trigger is a plain element and the content is not rendered. Open when its view of the main window hides, it closes;
 * its hover opens it only with a move of the pointer, not a trigger that came under a still one. `controlKey`: the
 * action and its subject of a control that renders this root anew, else the root is its own control
 */
function Tooltip({
  disableHoverablePopup = true,
  lazy = false,
  controlKey,
  open,
  onOpenChange,
  ...props
}: Omit<TooltipPrimitive.Root.Props, "actionsRef"> & { lazy?: boolean; controlKey?: string }) {
  const root = useLazyRoot(lazy, open === true || props.defaultOpen === true)
  const closing = useCloseWhenViewHides(open, hoverOpensOnMove(onOpenChange))
  const key = useControlKey(controlKey)
  return (
    // every root sets the context, so a trigger in its content never reads the root of an outer tooltip
    <LazyRootContext value={lazy ? root : null}>
      <ControlKeyContext value={key}>
        {root.armed ? (
          <TooltipPrimitive.Root
            data-slot="tooltip"
            disableHoverablePopup={disableHoverablePopup}
            open={open}
            {...props}
            actionsRef={closing.actionsRef}
            onOpenChange={closing.onOpenChange}
          />
        ) : (
          childNodes(props.children)
        )}
      </ControlKeyContext>
    </LazyRootContext>
  )
}

function TooltipTrigger({ ref, ...props }: TooltipPrimitive.Trigger.Props) {
  const lazy = use(LazyRootContext)
  const controlRef = useControlRef(ref)
  if (lazy !== null && !lazy.armed) {
    return <PlainTrigger root={lazy} trigger={props} closed={CLOSED_TOOLTIP_TRIGGER} baseUiOnly={TOOLTIP_TRIGGER_ONLY} ref={controlRef} />
  }
  return <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} ref={controlRef} />
}

function TooltipContent({
  className,
  side = "top",
  sideOffset = 4,
  align = "center",
  alignOffset = 0,
  children,
  ...props
}: TooltipPrimitive.Popup.Props & Pick<TooltipPrimitive.Positioner.Props, "align" | "alignOffset" | "side" | "sideOffset">) {
  // the content of a lazy root that is not mounted yet
  if (use(LazyRootContext)?.armed === false) {
    return null
  }
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Positioner align={align} alignOffset={alignOffset} side={side} sideOffset={sideOffset} className="isolate z-50">
        <TooltipPrimitive.Popup
          data-slot="tooltip-content"
          // Base UI sets no role on the popup
          role="tooltip"
          className={cn(
            "z-50 inline-flex w-fit max-w-xs origin-(--transform-origin) items-center gap-1.5 rounded-xl bg-foreground px-3 py-1.5 text-xs text-background has-data-[slot=kbd]:pr-1.5 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 **:data-[slot=kbd]:relative **:data-[slot=kbd]:isolate **:data-[slot=kbd]:z-50 **:data-[slot=kbd]:rounded-lg data-[state=delayed-open]:animate-in data-[state=delayed-open]:fade-in-0 data-[state=delayed-open]:zoom-in-95 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95",
            className,
          )}
          {...props}
        >
          {children}
          <TooltipPrimitive.Arrow className="z-50 size-2.5 translate-y-[calc(-50%-2px)] rotate-45 rounded-[2px] bg-foreground fill-foreground data-[side=bottom]:top-1 data-[side=inline-end]:top-1/2! data-[side=inline-end]:-left-1 data-[side=inline-end]:translate-x-[1.5px] data-[side=inline-end]:-translate-y-1/2 data-[side=inline-start]:top-1/2! data-[side=inline-start]:-right-1 data-[side=inline-start]:translate-x-[-1.5px] data-[side=inline-start]:-translate-y-1/2 data-[side=left]:top-1/2! data-[side=left]:-right-1 data-[side=left]:translate-x-[-1.5px] data-[side=left]:-translate-y-1/2 data-[side=right]:top-1/2! data-[side=right]:-left-1 data-[side=right]:translate-x-[1.5px] data-[side=right]:-translate-y-1/2 data-[side=top]:-bottom-2.5" />
        </TooltipPrimitive.Popup>
      </TooltipPrimitive.Positioner>
    </TooltipPrimitive.Portal>
  )
}

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider }
