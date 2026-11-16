// local customisation: `role="tooltip"` on the popup; `disableHoverablePopup` by default: a text tooltip closes when the
// pointer leaves its trigger and never blocks what is under it (a tooltip over the submit of the form above it stayed
// open while hovered and swallowed the click); `lazy` on `Tooltip`: the root mounts at the first hover or focus of its
// trigger, a plain element until then (`useLazyRoot`), for the many tooltips of a table row; it closes when its view of
// the main window hides (`useCloseWhenViewHides`), and its hover opens it only with a move of the pointer
// (`hoverOpensOnMove`); `controlKey` on `Tooltip`, the trigger named as the control of its root (`useControlRef`); a
// tooltip enabled with the pointer resting on its trigger opens after its delay (`hoverAgain`); the words of the content,
// a string (`description` beside content that is not one), are the description of its trigger (`aria-describedby` of a
// hidden span at the end of the page), there also while the tooltip is closed, once its lazy root mounted: Base UI gives
// the trigger none, so a screen reader heard no reason of a disabled control
"use client"

import { cloneElement, createContext, Fragment, isValidElement, use, useEffect, useId, useLayoutEffect, useRef } from "react"
import type { ReactElement, ReactNode } from "react"
import { createPortal } from "react-dom"
import { Tooltip as TooltipPrimitive } from "@base-ui/react/tooltip"
import { cn } from "cn"
import { useCloseWhenViewHides } from "@/hooks/useCloseWhenViewHides"
import { ControlKeyContext, useControlKey, useControlRef } from "@/hooks/useControlKey"
import { childNodes, LazyRootContext, PlainTrigger, useLazyRoot } from "@/hooks/useLazyRoot"
import { hoverAgain, hoverOpensOnMove } from "@/lib/pointer-moves"

function TooltipProvider({ delay = 0, ...props }: TooltipPrimitive.Provider.Props) {
  return <TooltipPrimitive.Provider data-slot="tooltip-provider" delay={delay} {...props} />
}

/**
 * The id of the description of the trigger of a tooltip, the words of its content in a hidden span (`TooltipDescription`);
 * null where the tooltip says nothing then: disabled, with no words to say, or a lazy root not mounted yet
 */
const TooltipDescriptionContext = createContext<string | null>(null)

/** The props of a tooltip trigger that only Base UI reads: a plain trigger leaves them out */
const TOOLTIP_TRIGGER_ONLY = ["delay", "closeDelay", "closeOnClick", "handle", "payload", "disabled"]
const CLOSED_TOOLTIP_TRIGGER = { "data-slot": "tooltip-trigger" }

/**
 * `lazy`: the root mounts at the first hover or focus of its trigger, or while `open` (`useLazyRoot`); until then the
 * trigger is a plain element, the content is not rendered, and the trigger has no description: the tooltips of a large
 * table add no node until one is reached. Open when its view of the main window hides, it closes; its hover opens it
 * only with a move of the pointer, not a trigger that came under a still one. `controlKey`: the action and its subject
 * of a control that renders this root anew, else the root is its own control. The description of the trigger is the
 * words of the `TooltipContent` among its children (`contentWords`): a direct child of the root, or one in a fragment
 * or a list of its children; a `TooltipContent` that a component of its own renders gives the trigger no description
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
  useHoverWhenEnabled(props.disabled === true, key)
  const descriptionId = useId()
  const words = props.disabled === true || !root.armed || typeof props.children === "function" ? null : contentWords(props.children)
  return (
    // every root sets the contexts, so a trigger in its content never reads the root of an outer tooltip
    <LazyRootContext value={lazy ? root : null}>
      <ControlKeyContext value={key}>
        <TooltipDescriptionContext value={words === null ? null : descriptionId}>
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
          {words !== null && <TooltipDescription id={descriptionId} words={words} />}
        </TooltipDescriptionContext>
      </ControlKeyContext>
    </LazyRootContext>
  )
}

/**
 * The words of the first `TooltipContent` among `children` (through fragments and lists), which describe the trigger of
 * the root: its string, or its `description` beside content that is not one; null for `description={false}`, a
 * tooltip that only says the name of its trigger (a screen reader heard it twice), and for a root without one
 */
function contentWords(children: ReactNode): string | null {
  if (Array.isArray(children)) {
    for (const child of children as ReactNode[]) {
      const words = contentWords(child)
      if (words !== null) {
        return words
      }
    }
    return null
  }
  if (!isValidElement<{ children?: ReactNode; description?: string | false }>(children)) {
    return null
  }
  if (children.type === Fragment) {
    return contentWords(children.props.children)
  }
  if (children.type !== TooltipContent) {
    return null
  }
  const { description, children: content } = children.props
  if (description !== undefined) {
    return description === false || description === "" ? null : description
  }
  return typeof content === "string" && content !== "" ? content : null
}

/**
 * A disabled tooltip enabled with the pointer resting on its trigger (the description of Approve, known once the
 * code-owner review of the pull request came): Base UI listens on the trigger only now, after the pointer came, so its
 * hover is replayed (`hoverAgain`) and the tooltip opens after its delay, as for a pointer that came now. After this
 * task: the root tells its trigger in a layout effect, which renders the trigger again and attaches its listeners in a
 * commit of its own, before the task ends
 */
function useHoverWhenEnabled(disabled: boolean, key: string): void {
  const wasDisabled = useRef(disabled)
  useEffect(() => {
    if (wasDisabled.current && !disabled) {
      queueMicrotask(() => hoverAgain(key))
    }
    wasDisabled.current = disabled
  }, [disabled, key])
}

function TooltipTrigger({ ref, ...props }: TooltipPrimitive.Trigger.Props) {
  const lazy = use(LazyRootContext)
  const controlRef = useControlRef(ref)
  const description = use(TooltipDescriptionContext)
  // its own description first (a reason it names itself), then the content of the tooltip
  const describedBy = [props["aria-describedby"], description].filter(id => id != null && id !== "").join(" ") || undefined
  if (lazy !== null && !lazy.armed) {
    const trigger = { ...props, "aria-describedby": describedBy } as TooltipPrimitive.Trigger.Props
    return <PlainTrigger root={lazy} trigger={trigger} closed={CLOSED_TOOLTIP_TRIGGER} baseUiOnly={TOOLTIP_TRIGGER_ONLY} ref={controlRef} />
  }
  return <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} aria-describedby={describedBy} ref={controlRef} />
}

/**
 * A control inside the trigger of its tooltip (a span that takes the hover of a disabled button, which gets no pointer
 * events): the words of the tooltip are its description too, where the focus is
 */
function TooltipDescribedChild({ children }: { children: ReactElement<{ "aria-describedby"?: string }> }) {
  const id = use(TooltipDescriptionContext)
  if (id === null) {
    return children
  }
  const own = children.props["aria-describedby"]
  return cloneElement(children, { "aria-describedby": own === undefined ? id : `${own} ${id}` })
}

/**
 * The words of a tooltip as the description of its trigger, there also while the tooltip is closed: a span of text
 * alone, no copy of the content (no second mount, no `id` or `data-testid` twice), in a hidden element of the end of the
 * page (`descriptions`), so neither the eye nor the keys reach it, nor a screen reader browsing the page, which
 * hears it as the description of the trigger only (a hidden element that `aria-describedby` names is read). Not beside
 * the trigger: its text joined the text of the trigger's parent, and an element after the trigger was what a lazy root
 * took for its trigger as it mounted
 */
function TooltipDescription({ id, words }: { id: string; words: string }) {
  // the render only portals into the element, which is not in the page yet; the page gets it before its paint, and
  // again after a page that replaced its body
  const container = descriptionsElement()
  useLayoutEffect(() => {
    if (!container.isConnected) {
      document.body.append(container)
    }
  })
  return createPortal(
    <span id={id} data-slot="tooltip-description">
      {words}
    </span>,
    container,
  )
}

let descriptions: HTMLElement | null = null

/**
 * The hidden element of the end of the page that holds the descriptions of the tooltips of its window: made once, at
 * the first description, out of the page; the layout effect of a description puts it in (`TooltipDescription`), so
 * no render changes the page and a page without a description has none
 */
function descriptionsElement(): HTMLElement {
  if (descriptions === null) {
    descriptions = document.createElement("div")
    descriptions.hidden = true
    descriptions.dataset.slot = "tooltip-descriptions"
  }
  return descriptions
}

/**
 * The words of a tooltip, which describe its trigger too: its content when that is a string, else a `description` of
 * its own, a short phrase of what the content says; `false` for a tooltip that only says the name of its trigger; no
 * string, nothing to say
 */
type TooltipWords = { children: string | null | undefined; description?: false } | { children: ReactNode; description: string | false }

type TooltipContentProps = Omit<TooltipPrimitive.Popup.Props, "children"> & Pick<TooltipPrimitive.Positioner.Props, "align" | "alignOffset" | "side" | "sideOffset"> & TooltipWords

function TooltipContent({ className, side = "top", sideOffset = 4, align = "center", alignOffset = 0, children, description: _description, ...props }: TooltipContentProps) {
  // the content of a lazy root that is not mounted yet: nothing
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

export { Tooltip, TooltipTrigger, TooltipContent, TooltipDescribedChild, TooltipProvider }
