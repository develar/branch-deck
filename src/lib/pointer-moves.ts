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
 * pointer events fractions of a pixel, the mouse events whole ones. The elements a move went over stay pointed until
 * the pointer leaves them. An element that took the place of the one the pointer moved over, in its parent, is pointed
 * as it was: a control that renders its state anew ("⋯" of a row that waits for the lock, its tooltip open before);
 * WebKit sends the over and the enter of such an element only in a later task, and the move of that task to its
 * parent, so the replay of the first hover of a lazy trigger points it itself (`markPointedOver`).
 */

interface Position {
  x: number
  y: number
}

const last: Record<"pointer" | "mouse", Position | null> = { pointer: null, mouse: null }
// the events of this task came with a move; cleared after it, by a task of its own (not a timer, which a test fakes)
let moving = false
let endOfTask: MessagePort | null = null
const moves = new WeakSet<Event>()
const pointed = new WeakSet<Element>()
// the element the last move went over, the deepest
let under: Element | null = null

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
    }
  } else if (moving && target !== null && !event.type.endsWith("out")) {
    under = target
    for (let element: Element | null = target; element !== null; element = element.parentElement) {
      pointed.add(element)
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
 * The pointer moved over the element and has not left it, or over the one it replaced: the element the last move went
 * over left the page, and the parent of this one is pointed. Content that moved under the pointer is neither
 */
export function pointerMovedOver(element: Element): boolean {
  if (pointed.has(element)) {
    return true
  }
  const parent = element.parentElement
  return under !== null && !under.isConnected && parent !== null && pointed.has(parent)
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
      return
    }
    onOpenChange?.(open, details)
  }
}
