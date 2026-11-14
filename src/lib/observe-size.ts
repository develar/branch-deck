/**
 * Every `ResizeObserver` of the page is made here (`pnpm lint:arch`, `resize-observer`), so no change in the rendering
 * update of a report resizes an observed element after its report (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver"): WebKit renders there a commit of React that waited in a microtask after the callback,
 * and a flushSync of any callback renders every update pending then. An observation resized in its own delivery is one
 * the rendering update skips, a ResizeObserver loop. Until the next task after a report of any of these observers, the
 * elements of theirs that a change of their subtree touches leave their observer at once (no observation of them is
 * left to skip) and are observed anew in the next task, whose first report gives their size as it is then: the owner
 * reads it a frame later, as any report. An element let go so in every frame is a cycle (its own callback resizes it
 * again): no loop tells it any more, so the page logs it as an error (`CYCLE_RELEASES`). The observers of the libraries
 * (Base UI, the panels) are theirs, timed as the specification says; a loop of theirs is a warning of the page
 * (`src/lib/page-errors.ts`)
 */
import { logError } from "@/lib/errors"

/** The mark of an observer made here, which the test wrapper of the observers reads (tests/e2e/setup/resize-observers.ts) */
export const SIZE_OBSERVER_MARK = Symbol.for("branch-deck.observeSize")

/** An element let go this many times within a second is a cycle: once a frame, never a change that came in a report */
const CYCLE_RELEASES = 30
/** When each element was let go within the last second */
const releases = new WeakMap<Element, number[]>()

/**
 * `element` was let go in a commit: a cycle once it is let go in every frame of a second, logged once per cycle. Its
 * observers let it go together, counted once (a diff view has three)
 */
function countRelease(element: Element): void {
  const now = performance.now()
  const times = (releases.get(element) ?? []).filter(time => time > now - 1000)
  times.push(now)
  releases.set(element, times)
  if (times.length === CYCLE_RELEASES) {
    const name = element.getAttribute("data-testid") ?? element.getAttribute("data-slot") ?? element.tagName.toLowerCase()
    logError("A size observer of the page cycles", new Error(`${name} resized in every frame of a second`))
  }
}

/** The observers made here that observe something */
const observers = new Set<SizeObserver>()
/** A report of one of them was taken, and the task of its rendering update still runs */
let reporting = false
const mutations =
  typeof MutationObserver === "undefined"
    ? null
    : new MutationObserver(records => {
        if (!reporting) {
          return
        }
        const touched = records.map(record => record.target)
        const released = new Set<Element>()
        for (const observer of observers) {
          observer.release(touched, released)
        }
        for (const element of released) {
          countRelease(element)
        }
      })

/** A report was taken: until the next task, a change of an observed subtree lets its element go (`release`) */
function reported(): void {
  if (reporting) {
    return
  }
  reporting = true
  nextTask(() => {
    reporting = false
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

/** An observer of the sizes of elements, as a `ResizeObserver` (`observeSize`) */
export class SizeObserver {
  private readonly resize: ResizeObserver | null
  private readonly observed = new Map<Element, ResizeObserverOptions | undefined>()
  private readonly released = new Set<Element>()

  constructor(callback: ResizeObserverCallback) {
    this.resize =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver((entries, observer) => {
            try {
              callback(entries, observer)
            } finally {
              reported()
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
    this.observed.set(element, options)
    this.released.delete(element)
    this.resize.observe(element, options)
    observers.add(this)
    mutations?.observe(element, { subtree: true, childList: true, characterData: true, attributeFilter: ["class", "style", "hidden"] })
  }

  unobserve(element: Element): void {
    this.observed.delete(element)
    this.released.delete(element)
    this.resize?.unobserve(element)
  }

  disconnect(): void {
    this.observed.clear()
    this.released.clear()
    this.resize?.disconnect()
    observers.delete(this)
  }

  /**
   * The elements of this observer at or above `touched` leave it at once and are observed anew in the next task; each
   * joins `released`
   */
  release(touched: readonly Node[], released: Set<Element>): void {
    const resize = this.resize
    if (resize === null) {
      return
    }
    let any = false
    for (const node of touched) {
      for (let current: Node | null = node; current !== null; current = current.parentNode) {
        // the observed are elements: a text or a comment is none of them
        const element = current as Element
        if (this.observed.has(element)) {
          if (!this.released.has(element)) {
            resize.unobserve(element)
            this.released.add(element)
            released.add(element)
            any = true
          }
          break
        }
      }
    }
    if (any) {
      nextTask(() => {
        for (const element of this.released) {
          if (this.observed.has(element)) {
            resize.observe(element, this.observed.get(element))
          }
        }
        this.released.clear()
      })
    }
  }
}

/** A `ResizeObserver` of the page with `callback` (module docs) */
export function observeSize(callback: ResizeObserverCallback): SizeObserver {
  return new SizeObserver(callback)
}
