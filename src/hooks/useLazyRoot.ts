import { cloneElement, createContext, createElement, isValidElement, use, useEffect, useLayoutEffect, useRef, useState } from "react"
import type { ReactElement, ReactNode, Ref } from "react"
import { flushSync } from "react-dom"
import { mergeProps } from "@base-ui/react/merge-props"

/**
 * What the user did first on the plain trigger: the pointer came over it (there), or the focus came into it; and where
 * the trigger is, so its successor is found in its place (the root renders no element of its own while closed)
 */
interface Intent {
  kind: "pointer" | "focus"
  x: number
  y: number
  parent: Node
  next: Node | null
}

/** The event of a plain trigger that mounts its root */
interface ArmEvent {
  currentTarget: EventTarget & Element
  clientX?: number
  clientY?: number
}

/** The handlers of a plain trigger that mount its root: the pointer over it, or the focus in it */
export interface ArmProps {
  onPointerEnter: (event: ArmEvent) => void
  onMouseMove: (event: ArmEvent) => void
  onFocus: (event: ArmEvent) => void
}

export interface LazyRoot {
  /** The root is mounted: its trigger is Base UI's, with its hover, focus and press */
  armed: boolean
  /** The handlers of the plain trigger, until the root is mounted */
  armProps: ArmProps
}

/**
 * A Base UI root (a tooltip, a popover, a menu) mounted when it is first wanted, not with its trigger: a branch row has a
 * dozen of them, and the 141 rows of a large table spent most of their first mount on roots nobody opened
 * (docs/design/rendering-performance.md, Key Decision 7). Until then the trigger is a plain element with the same look and
 * the same ARIA (`PlainTrigger`); the first pointer over it or focus in it mounts the root at once (`flushSync`,
 * so a press that follows meets the real trigger) and replays itself on the trigger of the root, which takes the place
 * of the plain one: the focus moves to it, the pointer enters and moves over it, so Base UI starts the delay of its
 * hover as if it had seen the pointer come. The root stays mounted from then on, and while `wanted` (open from code: a
 * confirm, a request) it mounts without an intent. Nothing here asks the backend: a hover costs no IPC.
 *
 * The intent replays once: a view of the main window that shows again (React's `Activity`, `IndexPage`) runs the
 * effects of its components again, and the first hover of every trigger opened its popup with the pointer elsewhere,
 * which no pointer leave closed then, and the first focus took the focus back from where the user had put it.
 *
 * `lazy` false is the plain root, mounted with its trigger, as is every root in the content of a popover
 * (`EagerRootsContext`).
 */
export function useLazyRoot(lazy: boolean, wanted = false): LazyRoot {
  const eager = use(EagerRootsContext)
  const [armedState, setArmed] = useState(!lazy || eager)
  // what the plain trigger saw, replayed on the trigger of the root once it is mounted
  const [pending, setPending] = useState<Intent | null>(null)
  // the intent replayed: the effects that run again for it (the view shows again) replay nothing
  const replayed = useRef<Intent | null>(null)
  if (wanted && !armedState) {
    setArmed(true)
  }
  const armed = armedState || wanted

  // the focus first: the element that had it left with the plain trigger
  useLayoutEffect(() => {
    if (pending?.kind === "focus" && replayed.current !== pending) {
      replayed.current = pending
      triggerIn(pending)?.focus({ preventScroll: true })
    }
  }, [pending])
  // the effects of the trigger ran before this one (children first): Base UI listens on it now
  useEffect(() => {
    if (pending?.kind === "pointer" && replayed.current !== pending) {
      replayed.current = pending
      const element = triggerIn(pending)
      if (element !== null) {
        // outside the effect, so a plain trigger nested in it may mount its own root at once
        queueMicrotask(() => replayPointer(element, pending))
      }
    }
  }, [pending])

  const arm = (kind: Intent["kind"], event: ArmEvent) => {
    const parent = event.currentTarget.parentNode
    if (armed || parent === null) {
      return
    }
    const intent = { kind, x: event.clientX ?? 0, y: event.clientY ?? 0, parent, next: event.currentTarget.nextSibling }
    flushSync(() => {
      setPending(intent)
      setArmed(true)
    })
  }
  const armProps: ArmProps = {
    onPointerEnter: event => arm("pointer", event),
    // a trigger that appeared under a pointer at rest, or one nested in a trigger that just mounted its root
    onMouseMove: event => arm("pointer", event),
    onFocus: event => arm("focus", event),
  }
  return { armed, armProps }
}

/** The trigger of the mounted root, where the plain one was */
function triggerIn({ parent, next }: Intent): HTMLElement | null {
  const node = next !== null && next.parentNode === parent ? next.previousSibling : parent.lastChild
  return node instanceof HTMLElement && node.isConnected ? node : null
}

/**
 * The pointer as Base UI expects it on a trigger it just got: `mouseenter` on the element (its listener unblocks the
 * hover), then a `mousemove` that bubbles to React (its handler starts the delay, after which the popup opens)
 */
function replayPointer(element: HTMLElement, intent: Intent) {
  if (!element.isConnected) {
    return
  }
  const init = { clientX: intent.x, clientY: intent.y, view: window }
  element.dispatchEvent(new MouseEvent("mouseenter", init))
  element.dispatchEvent(new MouseEvent("mousemove", { ...init, bubbles: true, cancelable: true }))
}

/**
 * True in the content of a popover (`PopoverContent`): its roots mount with their triggers. A popup mounts its content
 * as it opens, one at a time, so it has no mount to save; and a trigger replaced under the pointer would close a popup a
 * hover opened: its safe polygon keeps it open for the element the pointer went to (`relatedTarget` of the `mouseleave`
 * of its trigger), which the swap took out of the page.
 */
export const EagerRootsContext = createContext(false)

/** The lazy root of a tooltip or a popover for its trigger and its content; null for a root mounted with its trigger */
export const LazyRootContext = createContext<LazyRoot | null>(null)

/** The props of a Base UI trigger, as far as the plain trigger reads them */
interface TriggerProps {
  render?: unknown
  className?: unknown
  style?: unknown
  children?: ReactNode
}

export interface PlainTriggerProps {
  root: LazyRoot
  /** The props of the trigger of the root */
  trigger: TriggerProps
  /** The attributes its root gives a closed trigger: `data-slot`, `aria-haspopup`, `aria-expanded` */
  closed: Readonly<Record<string, string | undefined>>
  /** The props of the trigger only Base UI reads, left out */
  baseUiOnly: readonly string[]
  ref?: Ref<HTMLElement>
}

/**
 * The trigger of a lazy root that is not mounted yet, as Base UI renders it: its `render` element (else a button) with
 * the props of the trigger merged in (`mergeProps`, the element's own last), with the attributes of a closed trigger and
 * the handlers that mount the root
 */
export function PlainTrigger({ root, trigger, closed, baseUiOnly, ref }: PlainTriggerProps): ReactElement {
  const { render, className, style, children, ...rest } = trigger
  const element: ReactElement<Record<string, unknown>> = isValidElement<Record<string, unknown>>(render) ? render : createElement("button", { type: "button" })
  const own: Record<string, unknown> = Object.fromEntries(Object.entries(rest).filter(([name]) => !baseUiOnly.includes(name)))
  // the state of a closed trigger, for a class or a style given as a function of it
  const state = { open: false }
  own.className = typeof className === "function" ? (className as (state: object) => string)(state) : className
  own.style = typeof style === "function" ? (style as (state: object) => object)(state) : style
  own.children = children
  const merged: Record<string, unknown> = mergeProps<"button">(closed, root.armProps, own, element.props)
  // the ref of the trigger, else the element's own (in its props)
  return cloneElement(element, ref == null ? merged : { ...merged, ref })
}

/**
 * The children of a root that is not mounted yet: its trigger and its content; a root whose children render its payload
 * (the detached triggers of a handle) has none of its own until it is mounted
 */
export function childNodes(children: ReactNode | ((...args: never[]) => ReactNode)): ReactNode {
  return typeof children === "function" ? null : children
}
