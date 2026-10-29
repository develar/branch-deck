/*
 * Whether the pointer moved over an element (spec/branch/branch-row.spec.md, Hover and click). Content that moves under
 * a still pointer (a row expanded or collapsed above, a new branch on top, a sync that reorders, a scroll) brings
 * another element under it: WebKit tells it at once with the over and the enter of the pointer, Chromium with the next
 * move, and WebKit moves the pointer where it is after a scroll; the events are trusted, at the position the pointer
 * had. A hover-opened popup opens only once the pointer moved over its trigger (`hoverOpensOnMove`): the description of
 * a push came under the pointer with the rows and opened, its Push session Only under the pointer.
 *
 * A move is the task of the events at a new position of the pointer, the out, the over, the enter and the move of each
 * family; the events of a later task at the same position are none. Each family has its position: Chromium gives the
 * pointer events fractions of a pixel, the mouse events whole ones. The elements the last move went over are pointed
 * until the pointer leaves them. Another element is never the same control by its place: a badge that left a row
 * slid the chip after it under the pointer. A control that renders itself anew as another element in its place names
 * itself by `data-control-key`, its action and its subject ("⋯" of a row, from "Moving…" to "Waiting for …" with its
 * tooltip open): the key the last move went over points its new element. The trigger that a lazy root mounts in the
 * place of its plain one is the same control too (`markPointedOver`): WebKit sends its over and enter only in a later
 * task, and the move of that task to its parent.
 *
 * A trigger whose open came before a move hears the next move over it as the pointer that enters it (a `mouseenter`):
 * a tooltip of a warm group (another closed just before) opens at the enter only, and the pointer that moves inside the
 * trigger enters it no more.
 */

/** The attribute that names a control across its renders: its action and its subject, as `more-actions:auth` */
export const CONTROL_KEY = "data-control-key"

interface Position {
  x: number
  y: number
}

const last: Record<"pointer" | "mouse", Position | null> = { pointer: null, mouse: null }
// the events of this task came with a move; cleared after it, by a task of its own (not a timer, which a test fakes)
let moving = false
let endOfTask: MessagePort | null = null
const moves = new WeakSet<Event>()
// what the last move went over, the elements and the keys of the controls among them, until the pointer leaves them
let pointed = new WeakSet<Element>()
let pointedKeys = new Set<string>()
// the triggers whose open by the hover was canceled: the next move over one enters it (`hoverOpensOnMove`)
const refused = new WeakSet<Element>()

function track(event: Event): void {
  if (!(event instanceof MouseEvent) || !event.isTrusted) {
    return
  }
  const family = event.type.startsWith("pointer") ? "pointer" : "mouse"
  const before = last[family]
  if (before === null || before.x !== event.clientX || before.y !== event.clientY) {
    last[family] = { x: event.clientX, y: event.clientY }
    if (!moving) {
      moving = true
      pointed = new WeakSet()
      pointedKeys = new Set()
      endOfTask?.postMessage(null)
    }
  }
  if (moving) {
    moves.add(event)
  }
  const target = event.target instanceof Element ? event.target : null
  if (event.type === "mouseleave" || event.type === "pointerleave") {
    if (target !== null) {
      pointed.delete(target)
      const key = target.getAttribute(CONTROL_KEY)
      if (key !== null) {
        pointedKeys.delete(key)
      }
    }
  } else if (moving && !event.type.endsWith("out")) {
    for (let element: Element | null = target; element !== null; element = element.parentElement) {
      pointed.add(element)
      const key = element.getAttribute(CONTROL_KEY)
      if (key !== null) {
        pointedKeys.add(key)
      }
      if (refused.has(element)) {
        refused.delete(element)
        const trigger = element
        const init = { clientX: event.clientX, clientY: event.clientY, view: window }
        // after the event of the move, as the pointer that came over the trigger
        queueMicrotask(() => trigger.dispatchEvent(new MouseEvent("mouseenter", init)))
      }
    }
  }
}

// before any listener of the page, so the first event a trigger gets is told apart already; the out events too: React
// makes the enter of an element of its own from the out of the element the pointer left
if (typeof window !== "undefined") {
  const channel = new MessageChannel()
  channel.port1.onmessage = () => {
    moving = false
  }
  endOfTask = channel.port2
  for (const type of ["pointerout", "pointerover", "pointerenter", "pointermove", "pointerleave", "mouseout", "mouseover", "mouseenter", "mousemove", "mouseleave"]) {
    window.addEventListener(type, track, { capture: true, passive: true })
  }
}

/** The event came with a move of the pointer */
export function pointerMoved(event: Event | undefined): boolean {
  return event !== undefined && moves.has(event)
}

/**
 * The pointer moved over the element and has not left it, or over the control it renders anew (its `CONTROL_KEY`).
 * Content that moved under the pointer is neither
 */
export function pointerMovedOver(element: Element): boolean {
  const key = element.getAttribute(CONTROL_KEY)
  return pointed.has(element) || (key !== null && pointedKeys.has(key))
}

/** The pointer moved over the element: it took the place of one the pointer moved over (`useLazyRoot`) */
export function markPointedOver(element: Element): void {
  pointed.add(element)
}

/** What a Base UI root tells of a change of its open: the reason, the event and its trigger, and the cancel of it */
interface OpenChangeDetails {
  readonly reason: string
  readonly event: Event
  readonly trigger: Element | undefined
  cancel: () => void
}

/**
 * The `onOpenChange` of a root that opens on hover: an open by the hover before the pointer moved over the trigger is
 * canceled, so content that moved a trigger under a still pointer opens nothing; the next move over it does
 */
export function hoverOpensOnMove<Details extends OpenChangeDetails>(onOpenChange?: (open: boolean, details: Details) => void): (open: boolean, details: Details) => void {
  return (open, details) => {
    if (open && details.reason === "trigger-hover" && !(details.trigger === undefined ? pointerMoved(details.event) : pointerMovedOver(details.trigger))) {
      details.cancel()
      if (details.trigger !== undefined) {
        refused.add(details.trigger)
      }
      return
    }
    onOpenChange?.(open, details)
  }
}
