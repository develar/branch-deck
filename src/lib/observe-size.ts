/**
 * Every `ResizeObserver` of the page is made here (`pnpm lint:arch`, `resize-observer`), so no change in the rendering
 * update of a report resizes an observed element after its report (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver"): WebKit renders there a commit of React that waited in a microtask after the callback,
 * and a flushSync of any callback renders every update pending then. The update delivers its observations by depth: after
 * a report it takes only those deeper than the shallowest element it reported, and an observation resized at that depth
 * or above is one it skips, a ResizeObserver loop.
 *
 * - The page is watched from a report of an observer (ours or a library's) to the end of its delivery, the next task or
 *   the next frame, whichever comes first. A change in a callback or in the microtasks right after it can resize any
 *   element: the layout is the page's (a block above a view that grows shrinks the view, a view hidden around a table
 *   empties its cells), so every element of ours no deeper than the elements of the latest report leaves its observers
 *   at once and is observed anew as the delivery ends: its size a frame later, as the loop would have it; one the change
 *   left at its size is reported again at that size, which changes nothing (no layout is read to tell them apart, which
 *   would add one to each callback that changes the page). A change later in the delivery lets go the elements it is in:
 *   one of the update however many promises after a callback (floating-ui's flushSync after the observer of Base UI), or
 *   a render of React in a task before the delivery ends, whose new rows let go only the elements above them, never the
 *   cells of every row (docs/design/rendering-performance.md, "The observers of sizes"). An element first observed in
 *   the delivery, no deeper, is observed only as it ends (a block a flushSync in a callback mounted, observed by its ref;
 *   a table mounted again; an element in place an effect observes). A deeper element stays: the update delivers it at
 *   its depth, in the frame of its change; one let go comes back as soon as a report of shallower elements shows it
 *   deeper than they are. A change after the delivery lets nothing go: the next update delivers it, in its own frame.
 * - The observers of the libraries (Base UI, the panels) report through the engine's `ResizeObserver` with one step
 *   before their callback (`watchTheLibraries`): their report opens the delivery at its depth as one of ours does, so a
 *   change of their callback that resizes an element of ours lets it go. Passive: theirs is the engine's class, prototype
 *   and methods, with the same entries in the same frame; no element of theirs is let go or observed anew.
 * - A cycle: an element let go in the delivery of `CYCLE_RELEASES` of `CYCLE_FRAMES` frames (every frame, every other)
 *   after its report there, resized by a change of a callback of ours (measured on the frame that makes it), which no
 *   loop tells any more, is logged as an error. A drag that resizes it between frames is no cycle: its size in the
 *   delivery is the one it was reported with, and one let go before its report in the delivery does not count. A cycle that resizes
 *   its element after the delivery (in a frame callback, a timer) is no loop and is not told: it resizes the element once
 *   a frame, as an animation, a transition, a drag or a resize of the window does for as long as they last, and no size,
 *   frame or time tells it from them.
 * - A loop the engine reports anyway: the elements of ours left resized since their report (`pendingOfOurs`), which
 *   src/lib/page-errors.ts logs as an error (src/lib/resize-loops.ts).
 *
 * A loop of the observers of a library is theirs, timed as the specification says: a warning of the page, repeated an
 * error
 */
import { logError } from "@/lib/errors"
import { attributeLoopsWith, CYCLE_FRAMES, CYCLE_RELEASES } from "@/lib/resize-loops"

/** The mark of an observer made here, which the test wrapper of the observers reads (tests/e2e/setup/resize-observers.ts) */
export const SIZE_OBSERVER_MARK = Symbol.for("branch-deck.observeSize")

/** Two sizes this close are the same: less than a layout unit (1/64 px), more than the rounding of a computed style */
const SAME_SIZE = 0.01

/** A box as laid out, inline (the width of the page) and block */
interface Box {
  readonly inline: number
  readonly block: number
}

/** The content box and the border box of an element, what an observation compares (`box`, the content box by default) */
interface Boxes {
  readonly content: Box
  readonly border: Box
}

/** What an observer observes an element with, and the boxes it reported it with since */
interface Observation {
  readonly options: ResizeObserverOptions | undefined
  reported: Boxes | null
}

/** An element let go in a delivery, until it ends */
interface Release {
  readonly depth: number
  /** The observers that let it go, or that observe it first once the delivery ends */
  readonly by: Set<SizeObserver>
  /** The frame of the delivery */
  readonly frame: number
  /**
   * Let go after its report in this update by a change of a callback of ours, or of the microtasks right after it: it
   * counts toward a cycle
   */
  readonly ofOurs: boolean
  /** Resized there since its report; measured only for the frame that would make a cycle */
  readonly resized: boolean
}

/** The observers made here that observe something */
const observers = new Set<SizeObserver>()
/** The observers of each observed element */
const observing = new Map<Element, Set<SizeObserver>>()
/**
 * The delivery: the depth of the shallowest element of the latest report, of ours or of a library, and the depth of each
 * observed element as it found it; null between deliveries
 */
let delivery: { depth: number; readonly depths: Map<Element, number>; readonly reported: Set<Element> } | null = null
/** A callback of an observer runs, or the microtasks right after it: a change now can resize any element */
let spanning = false
/** That callback is one of ours: a change of it counts toward a cycle */
let delivering = false
/** The end of the delivery is asked for */
let ending = false
/** The elements let go in the delivery, observed anew as it ends */
const released = new Map<Element, Release>()
/** The frames of the page, counted by `requestAnimationFrame` while a delivery or a cycle is open */
let frame = 0
let frameRequested = false
/** The frames within the last `CYCLE_FRAMES` whose delivery let each element go, and whether its cycle was logged */
const cycles = new Map<Element, { frames: number[]; logged: boolean }>()

/** The changes of the page from a report of an observer to the end of the delivery */
const mutations = typeof MutationObserver === "undefined" ? null : new MutationObserver(records => settle(records))
/** What can resize an element: a node added or removed, a text, a class, a style, hidden */
const CHANGES: MutationObserverInit = { subtree: true, childList: true, characterData: true, attributeFilter: ["class", "style", "hidden"] }
let watching = false

/** The parent of `node`, or the host of a shadow root: the slot a node is assigned to is not followed, the page has none */
function parentOf(node: Node): Node | null {
  return node.parentNode ?? (node.nodeType === 11 ? ((node as ShadowRoot).host ?? null) : null)
}

/** The number of ancestors of `node`, the depth by which a rendering update delivers */
function depthOf(node: Node): number {
  let depth = 0
  for (let current = parentOf(node); current !== null; current = parentOf(current)) {
    depth++
  }
  return depth
}

/** The boxes an entry reports; null where the engine gives no border box */
function boxesOf(entry: ResizeObserverEntry): Boxes | null {
  const border = entry.borderBoxSize?.[0]
  if (border === undefined) {
    return null
  }
  const content = entry.contentBoxSize?.[0]
  return {
    content: content === undefined ? { inline: entry.contentRect.width, block: entry.contentRect.height } : { inline: content.inlineSize, block: content.blockSize },
    border: { inline: border.inlineSize, block: border.blockSize },
  }
}

/** The boxes of `element` as laid out now, which no transform changes; null without a box */
function laidOut(element: Element): Boxes | null {
  if (!element.isConnected || typeof getComputedStyle === "undefined") {
    return null
  }
  const style = getComputedStyle(element)
  if (style.display === "none") {
    return { content: { inline: 0, block: 0 }, border: { inline: 0, block: 0 } }
  }
  const width = Number.parseFloat(style.width)
  const height = Number.parseFloat(style.height)
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    return null
  }
  const sum = (...values: string[]) => values.reduce((total, value) => total + (Number.parseFloat(value) || 0), 0)
  const around = {
    inline: sum(style.paddingLeft, style.paddingRight, style.borderLeftWidth, style.borderRightWidth),
    block: sum(style.paddingTop, style.paddingBottom, style.borderTopWidth, style.borderBottomWidth),
  }
  return style.boxSizing === "border-box"
    ? { content: { inline: width - around.inline, block: height - around.block }, border: { inline: width, block: height } }
    : { content: { inline: width, block: height }, border: { inline: width + around.inline, block: height + around.block } }
}

function sameBox(one: Box, other: Box): boolean {
  return Math.abs(one.inline - other.inline) <= SAME_SIZE && Math.abs(one.block - other.block) <= SAME_SIZE
}

/** Whether a box reported as `reported` is laid out as `now`, what the engine compares to tell an observation active */
function sameBoxes(reported: Boxes, now: Boxes): boolean {
  return sameBox(reported.content, now.content) && sameBox(reported.border, now.border)
}

/** An element in a log line: its test id, its slot, else its tag */
function nameOf(element: Element): string {
  return element.getAttribute("data-testid") ?? element.getAttribute("data-slot") ?? element.tagName.toLowerCase()
}

/** A report of ours with `entries`: the delivery is open, at the depth of its shallowest element */
function delivered(observer: SizeObserver, entries: readonly ResizeObserverEntry[]): void {
  const open = openDelivery(entries)
  for (const entry of entries) {
    observer.reported(entry.target, boxesOf(entry))
    open.reported.add(entry.target)
  }
}

/** A report of `entries`, of ours or of a library: the delivery is open at the depth of its shallowest element */
function openDelivery(entries: readonly ResizeObserverEntry[]): NonNullable<typeof delivery> {
  let depth = Number.POSITIVE_INFINITY
  for (const entry of entries) {
    depth = Math.min(depth, depthOf(entry.target))
  }
  if (delivery === null) {
    delivery = { depth, depths: new Map(), reported: new Set() }
    endDeliveryLater()
  } else {
    delivery.depth = depth
  }
  const open = delivery
  // the update takes in this iteration whatever is deeper than the elements it reports now: one let go deeper than
  // them is delivered at its depth, observed anew now
  for (const [element, release] of released) {
    if (release.depth > depth) {
      released.delete(element)
      observeAgain(element, release)
    }
  }
  return open
}

/**
 * A callback of an observer begins, `own` one of ours: the page is watched for its changes until the delivery ends, as
 * however many microtasks after the callback are of the update (a flushSync of floating-ui three promises after the
 * callback of Base UI)
 */
function beforeCallback(own: boolean): void {
  spanning = true
  delivering = own
  if (mutations !== null && !watching && typeof document !== "undefined") {
    mutations.observe(document.documentElement, CHANGES)
    watching = true
  }
}

/** The changes of a callback, then those of the microtasks it left (a commit of React that waited), count for its owner */
function afterCallback(): void {
  settle(mutations?.takeRecords() ?? [])
  queueMicrotask(() =>
    queueMicrotask(() => {
      settle(mutations?.takeRecords() ?? [])
      spanning = false
      delivering = false
    }),
  )
}

/** The depth of `element` in the delivery, found once in it */
function depthIn(open: NonNullable<typeof delivery>, element: Element): number {
  let depth = open.depths.get(element)
  if (depth === undefined) {
    depth = depthOf(element)
    open.depths.set(element, depth)
  }
  return depth
}

/**
 * The page changed (`records`, watched from a report to the end of its delivery), every observed element no deeper than
 * the elements of the latest report that the change may have resized leaves its observers: in a callback of an observer
 * or the microtasks right after it, any (the layout is the page's); later, the elements the change is in (a change of the
 * update three promises after a callback; a render of React in a task after the update, whose new rows let go only the
 * elements above them, never the cells of every row)
 */
function settle(records: readonly MutationRecord[]): void {
  const open = delivery
  if (open === null || records.length === 0) {
    return
  }
  const candidates = spanning ? observing.keys() : changedAbove(records)
  for (const element of candidates) {
    const depth = depthIn(open, element)
    if (!released.has(element) && depth <= open.depth) {
      // after a report of it in this update, by a callback of ours: what a cycle is made of. One not reported in it yet
      // may wait for its report in this iteration (resized before the update, a drag), let go all the same
      release(element, depth, delivering && open.reported.has(element))
    }
  }
}

/** The observed elements at or above the nodes `records` changed */
function changedAbove(records: readonly MutationRecord[]): Set<Element> {
  const above = new Set<Element>()
  const walked = new Set<Node>()
  for (const record of records) {
    for (let current: Node | null = record.target; current !== null && !walked.has(current); current = parentOf(current)) {
      walked.add(current)
      if (observing.has(current as Element)) {
        above.add(current as Element)
      }
    }
  }
  return above
}

/** `element` leaves its observers until the delivery ends */
function release(element: Element, depth: number, ofOurs: boolean): void {
  const by = new Set<SizeObserver>()
  for (const observer of observing.get(element) ?? []) {
    if (observer.letGo(element)) {
      by.add(observer)
    }
  }
  if (by.size > 0) {
    // the frame that makes a cycle is measured (a layout now): resized by a change of its own, not by a drag between frames
    const resized = ofOurs && recentFrames(element, frame).length + 1 >= CYCLE_RELEASES && resizedSinceReport(element, by)
    released.set(element, { depth, by, frame, ofOurs, resized })
  }
}

/** Whether `element` is laid out other than one of `by` reported it */
function resizedSinceReport(element: Element, by: ReadonlySet<SizeObserver>): boolean {
  const now = laidOut(element)
  return now !== null && [...by].some(observer => observer.differs(element, now))
}

/**
 * `element`, observed first by `observer` in the rendering update no deeper than its report, is observed as the delivery
 * ends: the update would skip its first observation (a block a flushSync in a callback mounted, with its ref)
 */
function deferObservation(element: Element, observer: SizeObserver, depth: number): void {
  const release = released.get(element)
  if (release === undefined) {
    released.set(element, { depth, by: new Set([observer]), frame, ofOurs: false, resized: false })
  } else {
    release.by.add(observer)
  }
}

/** The delivery ends at the next task or the next frame, whichever comes first */
function endDeliveryLater(): void {
  if (ending) {
    return
  }
  ending = true
  nextTask(endDelivery)
  requestFrame()
}

/** The delivery ended: its elements are observed anew, and those let go in it count toward a cycle */
function endDelivery(): void {
  if (!ending) {
    return
  }
  ending = false
  delivery = null
  if (watching) {
    // what changed since is the next update's to report: every element is observed anew now
    mutations?.disconnect()
    watching = false
  }
  for (const [element, release] of released) {
    if (release.ofOurs) {
      countCycle(element, release)
    }
    observeAgain(element, release)
  }
  released.clear()
}

function observeAgain(element: Element, release: Release): void {
  for (const observer of release.by) {
    observer.observeAgain(element)
  }
}

/** The frames before `now`, within `CYCLE_FRAMES` of it, whose delivery let `element` go */
function recentFrames(element: Element, now: number): number[] {
  return (cycles.get(element)?.frames ?? []).filter(at => at > now - CYCLE_FRAMES && at < now)
}

/**
 * `element` was let go after its report in the delivery of `release.frame`: a cycle once so in `CYCLE_RELEASES` of the
 * last `CYCLE_FRAMES` frames, resized there; logged once while it goes on
 */
function countCycle(element: Element, release: Release): void {
  const logged = cycles.get(element)?.logged === true
  const frames = [...recentFrames(element, release.frame), release.frame]
  if (frames.length < CYCLE_RELEASES) {
    cycles.set(element, { frames, logged })
  } else if (release.resized) {
    if (!logged) {
      logError("A size observer of the page cycles", new Error(`${nameOf(element)} resized in its own delivery in ${frames.length} of ${CYCLE_FRAMES} frames`))
    }
    cycles.set(element, { frames, logged: true })
  } else {
    // its size there was the one it was reported with: a change between frames (a drag, a resize of the window)
    cycles.set(element, { frames: [], logged: false })
  }
  requestFrame()
}

/** The next frame is counted and ends the delivery; an element let go in none of the last frames is dropped, and frames count while one is kept */
function requestFrame(): void {
  if (frameRequested || typeof requestAnimationFrame === "undefined") {
    return
  }
  frameRequested = true
  requestAnimationFrame(() => {
    frameRequested = false
    frame++
    endDelivery()
    for (const [element, chain] of cycles) {
      if (chain.frames.every(at => at <= frame - CYCLE_FRAMES)) {
        cycles.delete(element)
      }
    }
    if (cycles.size > 0) {
      requestFrame()
    }
  })
}

/** Runs `run` in the next task, after the rendering update: a `MessageChannel`, as the timers of a page can be a test's fake clock */
function nextTask(run: () => void): void {
  const channel = new MessageChannel()
  channel.port1.onmessage = () => {
    channel.port1.close()
    run()
  }
  channel.port2.postMessage(null)
}

/** At a loop the engine reports: the elements of ours it left resized since their report, or observed and never reported */
function pendingOfOurs(): string[] {
  const names = new Set<string>()
  for (const observer of observers) {
    for (const element of observer.pending()) {
      names.add(nameOf(element))
    }
  }
  return [...names]
}

attributeLoopsWith(pendingOfOurs)

/**
 * The global `ResizeObserver`, as the libraries make theirs, is the engine's with one step before each callback: a report
 * of an observer not made here opens the delivery at its depth. Passive: `new ResizeObserver` makes an instance of the
 * engine's class (a subclass of it, `new.target`, its own), `ResizeObserver.prototype` and its methods are the engine's,
 * `instanceof` holds for every instance, and the callback gets the same entries and observer in the same frame
 */
function watchTheLibraries(): void {
  if (typeof ResizeObserver === "undefined" || Reflect.get(ResizeObserver, WATCHED) === true) {
    return
  }
  const Engine = ResizeObserver
  const Watched = function ResizeObserver(this: unknown, callback: ResizeObserverCallback) {
    if (new.target === undefined) {
      throw new TypeError("Failed to construct 'ResizeObserver': Please use the 'new' operator")
    }
    const watched: ResizeObserverCallback = (entries, observer) => {
      if (Reflect.get(observer, SIZE_OBSERVER_MARK) === true) {
        callback(entries, observer)
        return
      }
      openDelivery(entries)
      beforeCallback(false)
      try {
        callback(entries, observer)
      } finally {
        afterCallback()
      }
    }
    return Reflect.construct(Engine, [watched], new.target) as ResizeObserver
  } as unknown as typeof ResizeObserver
  Object.defineProperty(Watched, "prototype", { value: Engine.prototype })
  Reflect.set(Watched, WATCHED, true)
  globalThis.ResizeObserver = Watched
}

/** The mark of the watched global, so a second copy of this module watches nothing twice */
const WATCHED = Symbol.for("branch-deck.observeSize.watched")

watchTheLibraries()

/** An observer of the sizes of elements, as a `ResizeObserver` (`observeSize`) */
export class SizeObserver {
  private readonly resize: ResizeObserver | null
  private readonly observed = new Map<Element, Observation>()
  /** The elements of `observed` let go in the delivery */
  private readonly gone = new Set<Element>()

  constructor(callback: ResizeObserverCallback) {
    this.resize =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver((entries, observer) => {
            delivered(this, entries)
            beforeCallback(true)
            try {
              callback(entries, observer)
            } finally {
              afterCallback()
            }
          })
    if (this.resize !== null) {
      Reflect.set(this.resize, SIZE_OBSERVER_MARK, true)
    }
  }

  observe(element: Element, options?: ResizeObserverOptions): void {
    if (this.resize === null) {
      return
    }
    this.observed.set(element, { options, reported: null })
    this.gone.delete(element)
    observers.add(this)
    let by = observing.get(element)
    if (by === undefined) {
      by = new Set()
      observing.set(element, by)
    }
    by.add(this)
    // in the rendering update, no deeper than its report: the update would skip this first observation
    const open = delivery
    if (open !== null) {
      const depth = depthIn(open, element)
      if (depth <= open.depth) {
        this.gone.add(element)
        deferObservation(element, this, depth)
        return
      }
    }
    this.resize.observe(element, options)
  }

  unobserve(element: Element): void {
    this.observed.delete(element)
    this.gone.delete(element)
    this.resize?.unobserve(element)
    this.leave(element)
  }

  disconnect(): void {
    for (const element of this.observed.keys()) {
      this.leave(element)
    }
    this.observed.clear()
    this.gone.clear()
    this.resize?.disconnect()
    observers.delete(this)
  }

  /** Its report of `element` gave `boxes` */
  reported(element: Element, boxes: Boxes | null): void {
    const observation = this.observed.get(element)
    if (observation !== undefined) {
      observation.reported = boxes
    }
  }

  /** Whether `element` is laid out as `now`, other boxes than it reported, or a size never reported */
  differs(element: Element, now: Boxes): boolean {
    const reported = this.observed.get(element)?.reported
    return reported === undefined ? false : reported === null ? now.border.inline > 0 || now.border.block > 0 : !sameBoxes(reported, now)
  }

  /** `element` leaves it until the delivery ends; false when it does not observe it, or let it go already */
  letGo(element: Element): boolean {
    if (this.resize === null || !this.observed.has(element) || this.gone.has(element)) {
      return false
    }
    this.resize.unobserve(element)
    this.gone.add(element)
    return true
  }

  /** The delivery ended: `element`, let go in it, is observed anew, its first report its size then */
  observeAgain(element: Element): void {
    const observation = this.observed.get(element)
    if (this.resize === null || observation === undefined || !this.gone.delete(element)) {
      return
    }
    observation.reported = null
    this.resize.observe(element, observation.options)
  }

  /** Its elements a loop left pending: resized since their report, or observed with a size and never reported */
  pending(): Element[] {
    return [...this.observed.keys()].filter(element => {
      const now = this.gone.has(element) ? null : laidOut(element)
      return now !== null && this.differs(element, now)
    })
  }

  private leave(element: Element): void {
    const by = observing.get(element)
    by?.delete(this)
    if (by?.size === 0) {
      observing.delete(element)
    }
  }
}

/** A `ResizeObserver` of the page with `callback` (module docs) */
export function observeSize(callback: ResizeObserverCallback): SizeObserver {
  return new SizeObserver(callback)
}
