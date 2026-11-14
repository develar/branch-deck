import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"

/** The size each fake element is laid out at now */
const sizes = new Map<object, { width: number; height: number }>()

/** A node of a fake document: its parent, its test id */
interface FakeNode {
  readonly parentNode: FakeNode | null
  readonly nodeType: number
  readonly isConnected: boolean
  readonly tagName: string
  getAttribute(name: string): string | null
}

function node(parent: FakeNode | null, testId: string | null = null, nodeType = 1): FakeNode {
  return { parentNode: parent, nodeType, isConnected: true, tagName: "DIV", getAttribute: name => (name === "data-testid" ? testId : null) }
}

/** An element under `parent`, laid out at `width` × `height`, and a text inside it */
function element(parent: FakeNode, testId: string, width = 100, height = 20): { element: Element; text: Node } {
  const it = node(parent, testId)
  sizes.set(it, { width, height })
  return { element: it as unknown as Element, text: node(it, null, 3) as unknown as Node }
}

function resize(target: Element, width: number, height: number): void {
  sizes.set(target, { width, height })
}

/** The one `ResizeObserver` of an observer: what it observes, and its callback to report by hand */
class FakeResizeObserver {
  static all: FakeResizeObserver[] = []
  readonly observed = new Set<Element>()

  constructor(readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.all.push(this)
  }

  observe(target: Element): void {
    this.observed.add(target)
  }

  unobserve(target: Element): void {
    this.observed.delete(target)
  }

  disconnect(): void {
    this.observed.clear()
  }

  /** A report of `targets` at the sizes they are laid out at, as a rendering update gives it */
  report(...targets: Element[]): void {
    this.callback(
      targets.map(target => {
        const size = sizes.get(target)!
        return { target, borderBoxSize: [{ inlineSize: size.width, blockSize: size.height }] } as unknown as ResizeObserverEntry
      }),
      this as unknown as ResizeObserver,
    )
  }
}

/** The observer of the changes of the observed subtrees: a change is a record, delivered in a microtask as the engine does */
class FakeMutationObserver {
  static current: FakeMutationObserver | null = null
  private pending: MutationRecord[] = []

  constructor(private readonly callback: MutationCallback) {
    FakeMutationObserver.current = this
  }

  observe(): void {}

  takeRecords(): MutationRecord[] {
    return this.pending.splice(0)
  }

  /** A commit of React changed `nodes` */
  commit(...nodes: Node[]): void {
    if (this.pending.length === 0) {
      queueMicrotask(() => {
        const records = this.takeRecords()
        if (records.length > 0) {
          this.callback(records, this as unknown as MutationObserver)
        }
      })
    }
    this.pending.push(...nodes.map(target => ({ target }) as unknown as MutationRecord))
  }
}

function commit(...nodes: Node[]): void {
  FakeMutationObserver.current!.commit(...nodes)
}

/** The callbacks of the next frame, as `requestAnimationFrame` runs them at the start of a rendering update */
let frameCallbacks: FrameRequestCallback[] = []

function nextFrame(): void {
  const callbacks = frameCallbacks
  frameCallbacks = []
  for (const callback of callbacks) {
    callback(0)
  }
}

/** The microtasks after a callback: the delivery of the changes, a commit of React that waited */
async function microtasks(): Promise<void> {
  for (let count = 0; count < 6; count++) {
    await Promise.resolve()
  }
}

/** The next task, after which a rendering update is over */
function nextTask(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 5))
}

describe("observeSize", () => {
  let body: FakeNode

  /** A scroller of the body, and a block deeper inside it */
  const scrollerWithBlock = () => {
    const scroller = element(body, "scroller")
    return { scroller, block: element(node(scroller.element as unknown as FakeNode), "block") }
  }

  beforeEach(() => {
    vi.resetModules()
    FakeResizeObserver.all = []
    frameCallbacks = []
    sizes.clear()
    vi.stubGlobal("ResizeObserver", FakeResizeObserver)
    vi.stubGlobal("MutationObserver", FakeMutationObserver)
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frameCallbacks.push(callback))
    vi.stubGlobal("getComputedStyle", (target: object) => {
      const size = sizes.get(target) ?? { width: 0, height: 0 }
      return { display: "block", boxSizing: "border-box", width: `${size.width}px`, height: `${size.height}px` }
    })
    body = node(node(null, null, 9))
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.doUnmock("@/lib/errors")
  })

  it("lets an element a change of the delivery touches go at once, and observes it anew as the delivery ends", async () => {
    // WebKit renders a commit of React that waited in a microtask after the callback, in the rendering update of the
    // report: an element it resized after its report would be an observation that update skips, a ResizeObserver loop
    const { observeSize } = await import("@/lib/observe-size")
    const reports: Element[][] = []
    const prologue = element(body, "prologue")
    const observer = observeSize(entries => {
      reports.push(entries.map(entry => entry.target))
      commit(prologue.text)
    })
    observer.observe(prologue.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(prologue.element)
    expect(reports).toEqual([[prologue.element]])
    expect(resizeObserver.observed.has(prologue.element)).toBe(false)
    await nextTask()
    // observed anew: its first report gives its size as it is now
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)

    // between deliveries a change is the observer's to report, as any change
    commit(prologue.text)
    await microtasks()
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)
  })

  it("observes an element a task after the update let go anew in the next frame, when that frame comes before the next task", async () => {
    // a render of React in a task that ran before the end of the delivery: the next frame observes the element first, so
    // that frame's report has the change, and no frame paints the block without its height
    const { observeSize } = await import("@/lib/observe-size")
    const prologue = element(body, "prologue")
    observeSize(() => {}).observe(prologue.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(prologue.element)
    await microtasks()
    commit(prologue.text)
    await microtasks()
    expect(resizeObserver.observed.has(prologue.element)).toBe(false)
    nextFrame()
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)
  })

  it("leaves an element deeper than the elements of the latest report observed: the update delivers it at its depth", async () => {
    // the scroller reported, and a render in its delivery grew a block inside it: the update takes the block in at its
    // depth in that frame, so the block stays observed, and the layout has its height before that frame paints
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    observeSize(() => commit(block.text)).observe(scroller.element)
    observeSize(() => {}).observe(block.element)
    const [ofScroller, ofBlock] = FakeResizeObserver.all
    ofScroller!.report(scroller.element)
    await microtasks()
    expect(ofBlock!.observed.has(block.element)).toBe(true)
    // the scroller itself, touched at the depth of the report, leaves its observer
    expect(ofScroller!.observed.has(scroller.element)).toBe(false)
  })

  it("observes an element its report let go anew as a report of shallower elements shows it deeper than they are", async () => {
    // the blocks report first and their render grows one; the scroller reports next in the same iteration, which then
    // takes in only what is deeper than the scroller: the block is delivered in that frame, observed anew at once
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    observeSize(() => commit(block.text)).observe(block.element)
    observeSize(() => {}).observe(scroller.element)
    const [ofBlock, ofScroller] = FakeResizeObserver.all
    ofBlock!.report(block.element)
    await microtasks()
    expect(ofBlock!.observed.has(block.element)).toBe(false)
    ofScroller!.report(scroller.element)
    expect(ofBlock!.observed.has(block.element)).toBe(true)
  })

  it("logs a cycle once when its own callback resizes an element in its delivery in ten frames in a row", async () => {
    // frames, not time: a window that renders a frame a second tells the cycle in ten seconds
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const cell = element(body, "cell")
    let width = 100
    // a registry that widens the cell it measures by a pixel, again at every report
    observeSize(() => {
      width++
      resize(cell.element, width, 20)
      commit(cell.text)
    }).observe(cell.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    // the delivery of a frame, ended by the next frame
    const frame = async () => {
      resizeObserver.report(cell.element)
      await microtasks()
      nextFrame()
    }
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    expect(logError).not.toHaveBeenCalled()
    await frame()
    expect(logError).toHaveBeenCalledTimes(1)
    expect(logError.mock.calls[0]![0]).toBe("A size observer of the page cycles")
    expect(String(logError.mock.calls[0]![1])).toBe("Error: cell resized in its own delivery in 10 frames in a row")
    // the cycle goes on: logged once
    for (let count = 0; count < 12; count++) {
      await frame()
    }
    expect(logError).toHaveBeenCalledTimes(1)
  })

  it("counts an element its observers let go in one delivery once, and a frame without it ends the count", async () => {
    // a diff view has three observers: a change of it in a delivery lets it go from each, which is one frame
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "diff-view")
    let height = 20
    observeSize(() => {
      height++
      resize(view.element, 100, height)
      commit(view.text)
    }).observe(view.element)
    observeSize(() => {}).observe(view.element)
    observeSize(() => {}).observe(view.element)
    const all = FakeResizeObserver.all
    const frame = async () => {
      all[0]!.report(view.element)
      expect(all.map(resizeObserver => resizeObserver.observed.has(view.element))).toEqual([false, false, false])
      await microtasks()
      nextFrame()
    }
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    // a frame that renders nothing of it
    nextFrame()
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    expect(logError).not.toHaveBeenCalled()
    await frame()
    expect(logError).toHaveBeenCalledTimes(1)
  })

  it("logs no cycle of an element resized between frames, nor of one let go by a change after the update", async () => {
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    // a drag of the divider resizes the view before each frame, and its callback renders its rows into it
    const view = element(body, "diff-view")
    observeSize(() => commit(view.text)).observe(view.element)
    // a text that arrives in a task after each update resizes a block
    const block = element(body, "block")
    observeSize(() => {}).observe(block.element)
    const [ofView, ofBlock] = FakeResizeObserver.all
    for (let count = 0; count < 30; count++) {
      nextFrame()
      resize(view.element, 100 + count, 400)
      ofView!.report(view.element)
      ofBlock!.report(block.element)
      await microtasks()
      resize(block.element, 100, 20 + count)
      commit(block.text)
      await microtasks()
    }
    nextFrame()
    expect(logError).not.toHaveBeenCalled()
  })

  it("names at a loop the elements of ours resized since their report, or observed with a size and never reported", async () => {
    const { observeSize } = await import("@/lib/observe-size")
    const { loopOfOurs } = await import("@/lib/resize-loops")
    const resized = element(body, "resized")
    const same = element(body, "same")
    const fresh = element(body, "fresh")
    const empty = element(body, "empty", 0, 0)
    const observer = observeSize(() => {})
    for (const it of [resized, same, fresh, empty]) {
      observer.observe(it.element)
    }
    FakeResizeObserver.all[0]!.report(resized.element, same.element)
    resize(resized.element, 120, 20)
    expect(loopOfOurs()).toEqual(["resized", "fresh"])
  })

  it("leaves an element it no longer observes alone", async () => {
    const { observeSize } = await import("@/lib/observe-size")
    const observer = observeSize(() => {})
    const gone = element(body, "gone")
    observer.observe(gone.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(gone.element)
    observer.unobserve(gone.element)
    commit(gone.text)
    await nextTask()
    expect(resizeObserver.observed.has(gone.element)).toBe(false)
  })
})
