// local customisation: `anchor`, `collisionPadding` and `collisionAvoidance` forwarded to the positioner in `PopoverContent`, `className="contents"` on the portal and its `container` (the issue popover inside another popup, a dialog or the toasts),
// `PopoverSectionTitle` (the one head of a section of every popover: help, Activity days, Robots groups), `lazy` on
// `Popover`: the root mounts at the first hover or focus of its trigger, a plain element until then (`useLazyRoot`), and
// the roots in `PopoverContent` mount with their triggers (`EagerRootsContext`); a popover closes when its view of the
// main window hides (`useCloseWhenViewHides`), and its hover opens it only with a move of the pointer (`hoverOpensOnMove`)
import * as React from "react"
import { Popover as PopoverPrimitive } from "@base-ui/react/popover"
import { cn } from "cn"
import { useCloseWhenViewHides } from "@/hooks/useCloseWhenViewHides"
import { childNodes, EagerRootsContext, LazyRootContext, PlainTrigger, useLazyRoot } from "@/hooks/useLazyRoot"
import { hoverOpensOnMove } from "@/lib/pointer-moves"

/** The props of a popover trigger that only Base UI reads: a plain trigger leaves them out */
const POPOVER_TRIGGER_ONLY = ["openOnHover", "delay", "closeDelay", "handle", "payload", "nativeButton"]

/**
 * `lazy`: the root mounts at the first hover or focus of its trigger, or while `open` (`useLazyRoot`); until then the
 * trigger is a plain element with the ARIA of a closed popover trigger and the content is not rendered. Open when its
 * view of the main window hides, it closes; its hover (`openOnHover`) opens it only with a move of the pointer, not a
 * trigger that came under a still one
 */
function Popover({ lazy = false, open, onOpenChange, ...props }: Omit<PopoverPrimitive.Root.Props, "actionsRef"> & { lazy?: boolean }) {
  const root = useLazyRoot(lazy, open === true || props.defaultOpen === true)
  const closing = useCloseWhenViewHides(open, hoverOpensOnMove(onOpenChange))
  return (
    // every root sets the context, so a trigger in its content never reads the root of an outer popover
    <LazyRootContext value={lazy ? root : null}>
      {root.armed ? (
        <PopoverPrimitive.Root data-slot="popover" open={open} {...props} actionsRef={closing.actionsRef} onOpenChange={closing.onOpenChange} />
      ) : (
        childNodes(props.children)
      )}
    </LazyRootContext>
  )
}

function PopoverTrigger({ ref, ...props }: PopoverPrimitive.Trigger.Props) {
  const lazy = React.use(LazyRootContext)
  if (lazy !== null && !lazy.armed) {
    // a trigger rendered as another element than a button is one for assistive technology, as Base UI makes it
    const closed = {
      "data-slot": "popover-trigger",
      "aria-haspopup": "dialog",
      "aria-expanded": "false",
      role: props.nativeButton === false ? "button" : undefined,
    }
    return <PlainTrigger root={lazy} trigger={props} closed={closed} baseUiOnly={POPOVER_TRIGGER_ONLY} ref={ref} />
  }
  return <PopoverPrimitive.Trigger data-slot="popover-trigger" {...props} ref={ref} />
}

function PopoverContent({
  className,
  align = "center",
  alignOffset = 0,
  side = "bottom",
  sideOffset = 4,
  anchor,
  collisionPadding,
  collisionAvoidance,
  container,
  children,
  ...props
}: PopoverPrimitive.Popup.Props &
  Pick<PopoverPrimitive.Positioner.Props, "align" | "alignOffset" | "side" | "sideOffset" | "anchor" | "collisionPadding" | "collisionAvoidance"> &
  Pick<PopoverPrimitive.Portal.Props, "container">) {
  // the content of a lazy root that is not mounted yet
  if (React.use(LazyRootContext)?.armed === false) {
    return null
  }
  return (
    // layout-neutral portal element: the popup is the outermost box (E2E visibility checks)
    <PopoverPrimitive.Portal className="contents" container={container}>
      <PopoverPrimitive.Positioner
        align={align}
        alignOffset={alignOffset}
        side={side}
        sideOffset={sideOffset}
        anchor={anchor}
        collisionPadding={collisionPadding}
        collisionAvoidance={collisionAvoidance}
        className="isolate z-50"
      >
        <PopoverPrimitive.Popup
          data-slot="popover-content"
          className={cn(
            "z-50 flex w-72 origin-(--transform-origin) flex-col gap-4 rounded-3xl bg-popover p-4 text-sm text-popover-foreground shadow-lg ring-1 ring-foreground/5 outline-hidden duration-100 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 dark:ring-foreground/10 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95",
            className,
          )}
          {...props}
        >
          {/* the roots of its content mount with their triggers */}
          <EagerRootsContext value>{children}</EagerRootsContext>
        </PopoverPrimitive.Popup>
      </PopoverPrimitive.Positioner>
    </PopoverPrimitive.Portal>
  )
}

function PopoverHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="popover-header" className={cn("flex flex-col gap-1 text-sm", className)} {...props} />
}

function PopoverTitle({ className, ...props }: PopoverPrimitive.Title.Props) {
  return <PopoverPrimitive.Title data-slot="popover-title" className={cn("text-base font-medium", className)} {...props} />
}

function PopoverDescription({ className, ...props }: PopoverPrimitive.Description.Props) {
  return <PopoverPrimitive.Description data-slot="popover-description" className={cn("text-muted-foreground", className)} {...props} />
}

/** The head of a section of a popover: muted `text-xs font-medium`, sentence case */
function PopoverSectionTitle({ className, ...props }: React.ComponentProps<"h3">) {
  return <h3 data-slot="popover-section-title" className={cn("text-xs font-medium text-muted-foreground", className)} {...props} />
}

export { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverSectionTitle, PopoverTitle, PopoverTrigger }
