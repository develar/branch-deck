/*
 * Whether the pointer moved over an element (openspec/specs/branch/branch-row/spec.md, Hover and click). Content that moves under
 * a still pointer (a row expanded or collapsed above, a new branch on top, a sync that reorders, a scroll) brings
 * another element under it: WebKit tells it at once with the over and the enter of the pointer, Chromium with the next
 * move, and WebKit moves the pointer where it is after a scroll; the events are trusted, at the position the pointer
 * had. A hover-opened popup opens only once the pointer moved over its trigger (`hoverOpensOnMove`): the description of
 * a push came under the pointer with the rows and opened, its Push session Only under the pointer.
 *
 * A move is the task of the events at a new position of the pointer, the out, the over, the enter and the move of each
 * family; the events of a later task at the same position are none. Each family has its position: Chromium gives the
 * pointer events fractions of a pixel, the mouse events whole ones. The elements the last move went over are pointed
 * until the pointer leaves them, and so is the element an out of that move went to (React makes the enter of its
 * element from it, which a lazy root mounts its trigger at, in the place of the plain one).
 *
 * Another element is never the same control by its place: a badge that left a row slid the chip after it under the
 * pointer. It is the same control by its key (`registerControl`): every root of `src/components/ui` names its trigger
 * by its own key, so a trigger that renders as another element in the same root (a badge or a text by the state, the
 * plain trigger of a lazy root and the one it mounts) keeps the hover; a control that renders a root anew names it by
 * its action and its subject (`controlKey`: "⋯" of a row, from "Moving…" to "Waiting for …", its tooltip open). The
 * keys the last move went over point every element of theirs.
 *
 * A trigger whose open came before a move hears the next move over it as the pointer that enters it (a `mouseenter`):
 * a tooltip of a warm group (another closed just before) opens at the enter only, and the pointer that moves inside the
 * trigger enters it no more.
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
// what the last move went over, the elements and the keys of the controls among them, until the pointer leaves them
let pointed = new WeakSet<Element>()
let pointedKeys = new Set<string>()
// the triggers whose open by the hover was canceled: the next move over one enters it (`hoverOpensOnMove`)
const refused = new WeakSet<Element>()
// the triggers whose close by the hover was canceled: the next move leaves each, unless it is over it
// (`hoverClosesOnMove`)
const kept = new Set<Element>()
// the keys of the control each element is (`registerControl`), and the elements of each key (`hoverAgain`)
const controlKeys = new WeakMap<Element, Set<string>>()
const keyElements = new Map<string, Set<Element>>()

/** Points `target` and the elements around it, with the keys of their controls; `event` the move that went there */
function point(target: Element | null, event: MouseEvent): void {
  for (let element: Element | null = target; element !== null; element = element.parentElement) {
    pointed.add(element)
    for (const key of controlKeys.get(element) ?? []) {
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
      for (const key of controlKeys.get(target) ?? []) {
        pointedKeys.delete(key)
      }
    }
  } else if (moving) {
    if (!event.type.endsWith("out")) {
      point(target, event)
    } else if (event.relatedTarget instanceof Element) {
      point(event.relatedTarget, event)
    }
  }
  // the last event of a move: what it pointed is known
  if (moving && event.type === "mousemove" && kept.size > 0) {
    leaveKept(event)
  }
}

/**
 * The kept triggers the move went past get the leave of the pointer, at its position and as an event of the move, so
 * Base UI decides by the pointer that moved (the safe polygon of a submenu: toward the submenu it stays, away it
 * closes); a kept trigger the pointer came back over keeps its popup as it is
 */
function leaveKept(move: MouseEvent): void {
  const triggers = [...kept]
  kept.clear()
  for (const trigger of triggers) {
    if (trigger.isConnected && !pointerMovedOver(trigger)) {
      const relatedTarget = move.target instanceof Element ? move.target : null
      const leave = new MouseEvent("mouseleave", { clientX: move.clientX, clientY: move.clientY, relatedTarget, view: window })
      moves.add(leave)
      trigger.dispatchEvent(leave)
    }
  }
}

/**
 * Names `element` the control `key` (a root of `src/components/ui` its trigger, `useControlRef`): a move over one of the
 * elements of a key points the others too. Returns the cleanup of the name
 */
export function registerControl(element: Element, key: string): () => void {
  let keys = controlKeys.get(element)
  if (keys === undefined) {
    keys = new Set()
    controlKeys.set(element, keys)
  }
  keys.add(key)
  let elements = keyElements.get(key)
  if (elements === undefined) {
    elements = new Set()
    keyElements.set(key, elements)
  }
  elements.add(element)
  const named = keys
  const of = elements
  return () => {
    named.delete(key)
    of.delete(element)
    if (of.size === 0 && keyElements.get(key) === of) {
      keyElements.delete(key)
    }
  }
}

/**
 * The pointer at (`x`, `y`) as Base UI expects it on a trigger that listens on it only now: `mouseenter` on the element
 * (its listener unblocks the hover), then a `mousemove` that bubbles to React (its handler starts the delay, after which
 * the popup opens)
 */
export function enterAndMove(element: Element, x: number, y: number): void {
  const init = { clientX: x, clientY: y, view: window }
  element.dispatchEvent(new MouseEvent("mouseenter", init))
  element.dispatchEvent(new MouseEvent("mousemove", { ...init, bubbles: true, cancelable: true }))
}

/**
 * The hover of the control `key` once more, for a root that listens on its trigger only now: a tooltip enabled as it got
 * something to say, with the pointer that came while it had nothing resting on it. Each shown element of the control,
 * which the pointer moved over and has not left, gets the pointer where it is (`enterAndMove`); a control the pointer
 * did not move over gets nothing, as content under a still pointer opens nothing
 */
export function hoverAgain(key: string): void {
  const at = last.mouse
  if (at === null) {
    return
  }
  for (const element of keyElements.get(key) ?? []) {
    if (pointerMovedOver(element) && element.getClientRects().length > 0) {
      enterAndMove(element, at.x, at.y)
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
 * The pointer moved over the element and has not left it, or over another element of its control (`registerControl`).
 * Content that moved under the pointer is neither
 */
export function pointerMovedOver(element: Element): boolean {
  if (pointed.has(element)) {
    return true
  }
  for (const key of controlKeys.get(element) ?? []) {
    if (pointedKeys.has(key)) {
      return true
    }
  }
  return false
}

/** What a Base UI root tells of a change of its open: the reason, the event and its trigger, and the cancel of it */
interface OpenChangeDetails {
  readonly reason: string
  readonly event: Event
  readonly trigger: Element | undefined
  cancel: () => void
}

/**
 * The `onOpenChange` of a submenu: a close by the hover that no move of the pointer brought is canceled, so content that
 * moved the trigger of an open submenu from under a still pointer closes nothing (a group that came above it: the
 * browser tells the trigger that the pointer left, and the item now there that it came, at the same position, the
 * safe polygon of Base UI closes on that leave). The next move leaves the trigger (`leaveKept`), unless it is over it
 */
export function hoverClosesOnMove<Details extends OpenChangeDetails>(onOpenChange?: (open: boolean, details: Details) => void): (open: boolean, details: Details) => void {
  return (open, details) => {
    // as `pointerMoved`: a close of Base UI may come without its event
    const event: Event | undefined = details.event
    if (!open && details.reason === "trigger-hover" && event?.isTrusted === true && !moving && !moves.has(event) && event.target instanceof Element) {
      details.cancel()
      kept.add(event.target)
      return
    }
    onOpenChange?.(open, details)
  }
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
