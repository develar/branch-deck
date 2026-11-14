/**
 * Every `ResizeObserver` of the page is made here (`pnpm lint:arch`, `resize-observer`), so no change in the rendering
 * update of a report resizes an observed element after its report (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver"): WebKit renders there a commit of React that waited in a microtask after the callback,
 * and a flushSync of any callback renders every update pending then. The update delivers its observations by depth: after
 * a report it takes only those deeper than the shallowest element it reported, and an observation resized at that depth
 * or above is one it skips, a ResizeObserver loop.
 *
 * - The delivery of an update, from a report of ours to the next task or the next frame, whichever comes first: an
 *   element a change of its subtree touches leaves its observers at once when it is no deeper than the elements of the
 *   latest report (no observation of it is left to skip), and is observed anew as the delivery ends, its size in the next
 *   report: a frame later for a change of the delivery, as the loop would have it; in its own frame for a change after
 *   the update (a task that ran before the end, as React renders, is followed by the frame that observes it first). A
 *   deeper element stays: the update delivers it at its depth, in the frame of its change. One let go comes back as soon
 *   as a report of shallower elements shows it deeper than they are.
 * - A cycle: an element let go in the delivery of `CYCLE_FRAMES` frames in a row, resized there by a change of its own
 *   (its callback resizes it again), which no loop tells any more, is logged as an error. A drag that resizes it between
 *   frames is no cycle: its size in the delivery is the one it was reported with.
 * - A loop the engine reports anyway: the elements of ours left resized since their report (`pendingOfOurs`), which
 *   src/lib/page-errors.ts logs as an error (src/lib/resize-loops.ts).
 *
 * The observers of the libraries (Base UI, the panels) are theirs, timed as the specification says; a loop of theirs is
 * a warning of the page, repeated an error
 */
import { logError } from "@/lib/errors"
import { attributeLoopsWith, CYCLE_FRAMES } from "@/lib/resize-loops"

/** The mark of an observer made here, which the test wrapper of the observers reads (tests/e2e/setup/resize-observers.ts) */
export const SIZE_OBSERVER_MARK = Symbol.for("branch-deck.observeSize")

/** Two sizes this close are the same: a layout unit is 1/64 px */
const SAME_SIZE = 0.05

/** A border box as laid out, inline (the width of the page) and block */
interface BorderBox {
  readonly inline: number
  readonly block: number
}

/** What an observer observes an element with, and the border box it reported it with since */
interface Observation {
  readonly options: ResizeObserverOptions | undefined
  reported: BorderBox | null
}

/** An element let go in a delivery, until it ends */
interface Release {
  readonly depth: number
  /** The observers that let it go */
  readonly by: ReadonlySet<SizeObserver>
  /** The frame of the delivery */
  readonly frame: number
  /** Let go by a change of the rendering update: a callback of ours ran, or the microtasks right after it */
  readonly inDelivery: boolean
  /** Resized there since its report; measured only for the frame that would make a cycle */
  readonly resized: boolean
}

/** The observed elements at or above a node with their depth, and its own depth */
interface Above {
  readonly depth: number
  readonly observed: readonly (readonly [Element, number])[]
}

/** The observers made here that observe something */
const observers = new Set<SizeObserver>()
/** The observers of each observed element */
const observing = new Map<Element, Set<SizeObserver>>()
/** The delivery: the depth of the shallowest element of the latest report of ours; null between deliveries */
let delivery: { depth: number } | null = null
/** A callback of ours runs, or the microtasks right after it: a change now is one of the rendering update */
let delivering = false
/** The end of the delivery is asked for */
let ending = false
/** The elements let go in the delivery, observed anew as it ends */
const released = new Map<Element, Release>()
/** The frames of the page, counted by `requestAnimationFrame` while a delivery or a cycle is open */
let frame = 0
let frameRequested = false
/** The elements let go in the delivery of the frames up to `frame`, and how many in a row */
const cycles = new Map<Element, { frame: number; frames: number; logged: boolean }>()

const mutations = typeof MutationObserver === "undefined" ? null : new MutationObserver(records => settle(records))

/** The parent of `node` in the flat tree: the host of a shadow root */
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

/** The observed elements at or above `node`, memoized across the records of one commit */
function observedAbove(node: Node, memo: Map<Node, Above>): Above {
  const chain: Node[] = []
  let current: Node | null = node
  let known: Above | undefined
  while (current !== null && (known = memo.get(current)) === undefined) {
    chain.push(current)
    current = parentOf(current)
  }
  let above: Above = known ?? { depth: -1, observed: [] }
  for (let index = chain.length - 1; index >= 0; index--) {
    const at = chain[index]!
    const depth = above.depth + 1
    above = observing.has(at as Element) ? { depth, observed: [...above.observed, [at as Element, depth]] } : { depth, observed: above.observed }
    memo.set(at, above)
  }
  return above
}

/** The border box of `element` as laid out now, which no transform changes; null without a box */
function laidOut(element: Element): BorderBox | null {
  if (!element.isConnected || typeof getComputedStyle === "undefined") {
    return null
  }
  const style = getComputedStyle(element)
  if (style.display === "none") {
    return { inline: 0, block: 0 }
  }
  const width = Number.parseFloat(style.width)
  const height = Number.parseFloat(style.height)
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    return null
  }
  if (style.boxSizing === "border-box") {
    return { inline: width, block: height }
  }
  const sum = (...values: string[]) => values.reduce((total, value) => total + (Number.parseFloat(value) || 0), 0)
  return {
    inline: width + sum(style.paddingLeft, style.paddingRight, style.borderLeftWidth, style.borderRightWidth),
    block: height + sum(style.paddingTop, style.paddingBottom, style.borderTopWidth, style.borderBottomWidth),
  }
}

function sameSize(one: BorderBox, other: BorderBox): boolean {
  return Math.abs(one.inline - other.inline) <= SAME_SIZE && Math.abs(one.block - other.block) <= SAME_SIZE
}

/** An element in a log line: its test id, its slot, else its tag */
function nameOf(element: Element): string {
  return element.getAttribute("data-testid") ?? element.getAttribute("data-slot") ?? element.tagName.toLowerCase()
}

/** A report of ours with `entries`: the delivery is open, at the depth of its shallowest element */
function delivered(observer: SizeObserver, entries: readonly ResizeObserverEntry[]): void {
  let depth = Number.POSITIVE_INFINITY
  for (const entry of entries) {
    const box = entry.borderBoxSize?.[0]
    observer.reported(entry.target, box === undefined ? null : { inline: box.inlineSize, block: box.blockSize })
    depth = Math.min(depth, depthOf(entry.target))
  }
  if (delivery === null) {
    delivery = { depth }
    endDeliveryLater()
  } else {
    delivery.depth = depth
  }
  // the update takes in this iteration whatever is deeper than the elements it reports now: one let go deeper than
  // them is delivered at its depth, observed anew now
  for (const [element, release] of released) {
    if (release.depth > depth) {
      released.delete(element)
      observeAgain(element, release)
    }
  }
}

/** The changes of a callback, then those of the microtasks it left (a commit of React that waited), are of its delivery */
function afterCallback(): void {
  settle(mutations?.takeRecords() ?? [])
  queueMicrotask(() =>
    queueMicrotask(() => {
      settle(mutations?.takeRecords() ?? [])
      delivering = false
    }),
  )
}

/** The changes `records` tell: in a delivery, an observed element they touch no deeper than its report leaves its observers */
function settle(records: readonly MutationRecord[]): void {
  if (delivery === null || records.length === 0) {
    return
  }
  const memo = new Map<Node, Above>()
  const touched = new Map<Element, number>()
  for (const record of records) {
    for (const [element, depth] of observedAbove(record.target, memo).observed) {
      touched.set(element, depth)
    }
  }
  for (const [element, depth] of touched) {
    if (depth <= delivery.depth && !released.has(element)) {
      release(element, depth)
    }
  }
}

/** `element` leaves its observers until the delivery ends */
function release(element: Element, depth: number): void {
  const by = new Set<SizeObserver>()
  for (const observer of observing.get(element) ?? []) {
    if (observer.letGo(element)) {
      by.add(observer)
    }
  }
  if (by.size === 0) {
    return
  }
  const chain = cycles.get(element)
  const frames = chain !== undefined && chain.frame === frame - 1 ? chain.frames : 0
  // the frame that makes a cycle is measured (a layout now): resized by a change of its own, not by a drag between frames
  const resized = delivering && frames + 1 >= CYCLE_FRAMES && resizedSinceReport(element, by)
  released.set(element, { depth, by, frame, inDelivery: delivering, resized })
}

/** Whether `element` is laid out at another size than one of `by` reported it with */
function resizedSinceReport(element: Element, by: ReadonlySet<SizeObserver>): boolean {
  const box = laidOut(element)
  return box !== null && [...by].some(observer => observer.wasReported(element, reported => !sameSize(reported, box)))
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
  for (const [element, release] of released) {
    if (release.inDelivery) {
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

/** `element` was let go in the delivery of `release.frame`: a cycle once so in `CYCLE_FRAMES` frames in a row, resized there */
function countCycle(element: Element, release: Release): void {
  const chain = cycles.get(element)
  const frames = chain !== undefined && chain.frame === release.frame - 1 ? chain.frames + 1 : 1
  if (frames < CYCLE_FRAMES) {
    cycles.set(element, { frame: release.frame, frames, logged: false })
  } else if (release.resized) {
    if (chain?.logged !== true) {
      logError("A size observer of the page cycles", new Error(`${nameOf(element)} resized in its own delivery in ${CYCLE_FRAMES} frames in a row`))
    }
    cycles.set(element, { frame: release.frame, frames, logged: true })
  } else {
    // its size there was the one it was reported with: a change between frames (a drag, a resize of the window)
    cycles.set(element, { frame: release.frame, frames: 0, logged: false })
  }
  requestFrame()
}

/** The next frame is counted and ends the delivery; a cycle that missed a frame is dropped, and frames count while one is open */
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
      if (chain.frame < frame - 1) {
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
            delivering = true
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
    this.resize.observe(element, options)
    observers.add(this)
    let by = observing.get(element)
    if (by === undefined) {
      by = new Set()
      observing.set(element, by)
    }
    by.add(this)
    mutations?.observe(element, { subtree: true, childList: true, characterData: true, attributeFilter: ["class", "style", "hidden"] })
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

  /** Its report of `element` gave `box` */
  reported(element: Element, box: BorderBox | null): void {
    const observation = this.observed.get(element)
    if (observation !== undefined) {
      observation.reported = box
    }
  }

  /** Whether it reported `element` since it observes it, with a box `test` holds for */
  wasReported(element: Element, test: (reported: BorderBox) => boolean): boolean {
    const reported = this.observed.get(element)?.reported
    return reported !== null && reported !== undefined && test(reported)
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
    return [...this.observed].flatMap(([element, observation]) => {
      const box = this.gone.has(element) ? null : laidOut(element)
      if (box === null) {
        return []
      }
      const reported = observation.reported
      return (reported === null ? box.inline > 0 || box.block > 0 : !sameSize(box, reported)) ? [element] : []
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
