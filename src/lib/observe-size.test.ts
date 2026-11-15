import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"

/**
 * The size each fake element is laid out at now, its border box, its padding on each side, and how far the computed
 * style misses the engine's layout of its width (`missedBy`, as WebKit at a zoom of the app other than 100%)
 */
const sizes = new Map<object, { width: number; height: number; padding?: number; missedBy?: number }>()

/** A node of a fake document: its parent, its test id */
interface FakeNode {
  readonly parentNode: FakeNode | null
  readonly nodeType: number
  /** In the page; a test removes an element by setting it false */
  isConnected: boolean
  readonly tagName: string
  getAttribute(name: string): string | null
  contains(other: FakeNode): boolean
}

function node(parent: FakeNode | null, testId: string | null = null, nodeType = 1): FakeNode {
  const it: FakeNode = {
    parentNode: parent,
    nodeType,
    isConnected: true,
    tagName: "DIV",
    getAttribute: name => (name === "data-testid" ? testId : null),
    contains: other => {
      for (let current: FakeNode | null = other; current !== null; current = current.parentNode) {
        if (current === it) {
          return true
        }
      }
      return false
    },
  }
  return it
}

/** An element under `parent`, laid out at `width` × `height`, and a text inside it */
function element(parent: FakeNode, testId: string, width = 100, height = 20): { element: Element; text: Node } {
  const it = node(parent, testId)
  sizes.set(it, { width, height })
  return { element: it as unknown as Element, text: node(it, null, 3) as unknown as Node }
}

function resize(target: Element, width: number, height: number, padding = 0): void {
  sizes.set(target, { ...sizes.get(target), width, height, padding })
}

/**
 * The one `ResizeObserver` of an observer: what it observes, and its callback to report by hand. The first one a module
 * makes is its probe of the loops, made before every other, kept apart from `all`
 */
class FakeResizeObserver {
  static all: FakeResizeObserver[] = []
  static probe: FakeResizeObserver | null = null
  readonly observed = new Set<Element>()
  /** The box each target is observed by */
  readonly boxes = new Map<Element, ResizeObserverBoxOptions | undefined>()

  constructor(readonly callback: ResizeObserverCallback) {
    if (FakeResizeObserver.probe === null) {
      FakeResizeObserver.probe = this
    } else {
      FakeResizeObserver.all.push(this)
    }
  }

  observe(target: Element, options?: ResizeObserverOptions): void {
    this.observed.add(target)
    this.boxes.set(target, options?.box)
  }

  unobserve(target: Element): void {
    this.observed.delete(target)
  }

  disconnect(): void {
    this.observed.clear()
  }

  /** The first report of what it observes, as the next rendering update gives it: the targets in the page the box observed of is not empty */
  reportObserved(): void {
    this.report(
      ...[...this.observed].filter(target => {
        if (!(target as unknown as FakeNode).isConnected) {
          return false
        }
        const { width = 0, padding = 0 } = sizes.get(target) ?? {}
        return (this.boxes.get(target) === "border-box" ? width : width - 2 * padding) > 0
      }),
    )
  }

  /** A report of `targets` at the sizes they are laid out at, as a rendering update gives it: none of no target */
  report(...targets: Element[]): void {
    if (targets.length === 0) {
      return
    }
    this.callback(
      targets.map(target => {
        const { width, height, padding = 0 } = sizes.get(target)!
        return {
          target,
          borderBoxSize: [{ inlineSize: width, blockSize: height }],
          contentBoxSize: [{ inlineSize: width - 2 * padding, blockSize: height - 2 * padding }],
        } as unknown as ResizeObserverEntry
      }),
      this as unknown as ResizeObserver,
    )
  }
}

/**
 * The observer of the changes of the page, as the engine's: a change is a record only under a node it observes, and none
 * while it observes nothing; the records come in a microtask
 */
class FakeMutationObserver {
  static current: FakeMutationObserver | null = null
  private pending: MutationRecord[] = []
  private targets: { node: FakeNode; subtree: boolean }[] = []

  constructor(private readonly callback: MutationCallback) {
    FakeMutationObserver.current = this
  }

  observe(target: Node, options: MutationObserverInit): void {
    this.targets = [...this.targets.filter(it => it.node !== (target as unknown as FakeNode)), { node: target as unknown as FakeNode, subtree: options.subtree === true }]
  }

  disconnect(): void {
    this.targets = []
    this.pending = []
  }

  takeRecords(): MutationRecord[] {
    return this.pending.splice(0)
  }

  /** Whether it observes a node: it records nothing otherwise */
  get observing(): boolean {
    return this.targets.length > 0
  }

  /** A commit of React changed `nodes`: a record of each one under a node observed */
  commit(...nodes: Node[]): void {
    const watched = nodes.filter(changed =>
      this.targets.some(({ node, subtree }) => node === (changed as unknown as FakeNode) || (subtree && node.contains(changed as unknown as FakeNode))),
    )
    if (watched.length === 0) {
      return
    }
    if (this.pending.length === 0) {
      queueMicrotask(() => {
        const records = this.takeRecords()
        if (records.length > 0) {
          this.callback(records, this as unknown as MutationObserver)
        }
      })
    }
    this.pending.push(...watched.map(target => ({ target }) as unknown as MutationRecord))
  }
}

function commit(...nodes: Node[]): void {
  FakeMutationObserver.current!.commit(...nodes)
}

/** A commit of React grows `target` by a pixel, a change of its text */
function grow(target: { element: Element; text: Node }): void {
  const size = sizes.get(target.element)!
  resize(target.element, size.width, size.height + 1)
  commit(target.text)
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

/**
 * The ends of the deliveries waiting: the messages the module posts for them, held while a test runs a task of the page
 * before them (a timer due, a message of the backend), as the engine may run one first; null when each comes in the next
 * task
 */
let heldEnds: (() => void)[] | null = null

/** The channel the module ends a delivery with: its message comes in the next task, unless held (`holdTheEnds`) */
class FakeMessageChannel {
  readonly port1: { onmessage: (() => void) | null; close(): void } = { onmessage: null, close: () => {} }
  readonly port2 = {
    postMessage: () => {
      const deliver = () => this.port1.onmessage?.()
      if (heldEnds === null) {
        setTimeout(deliver, 0)
      } else {
        heldEnds.push(deliver)
      }
    },
  }
}

/** The deliveries end only at `end()`: a task of the page (`aTask`) runs before their end */
function holdTheEnds(): { end(): void } {
  heldEnds = []
  return {
    end: () => {
      const ends = heldEnds ?? []
      heldEnds = null
      for (const deliver of ends) {
        deliver()
      }
    },
  }
}

/** A task of the page: after the rendering update and every microtask of its checkpoint */
function aTask(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 0))
}

/** `count` microtasks after now, as the awaits of an async function */
async function hops(count: number): Promise<void> {
  for (let hop = 0; hop < count; hop++) {
    await Promise.resolve()
  }
}

/** `then` runs `promises` promises after now, as the awaits of an async function before it */
function promisesLater(promises: number, then: () => void): void {
  let chain = Promise.resolve()
  for (let count = 1; count < promises; count++) {
    chain = chain.then(() => undefined)
  }
  void chain.then(then)
}

describe("observeSize", () => {
  let body: FakeNode

  /** The elements of ours a loop the engine reports now left pending, as the probe tells them from the next update */
  const loopNow = async (): Promise<readonly string[]> => {
    const { loopOfOurs } = await import("@/lib/resize-loops")
    let told: readonly string[] | null = null
    loopOfOurs(pending => {
      told = pending
    })
    if (told === null) {
      FakeResizeObserver.probe!.reportObserved()
    }
    return told!
  }

  /** A panel of the body, observed by an observer of ours, which reported it: `missedBy` is how far the arithmetic misses its width */
  const reportedPanel = async (missedBy = 0) => {
    const { observeSize } = await import("@/lib/observe-size")
    const panel = element(body, "panel")
    sizes.set(panel.element, { width: 100, height: 20, missedBy })
    observeSize(() => {}).observe(panel.element)
    FakeResizeObserver.all[0]!.report(panel.element)
    return panel
  }

  /** A scroller of the body, and a block deeper inside it */
  const scrollerWithBlock = () => {
    const scroller = element(body, "scroller")
    return { scroller, block: element(node(scroller.element as unknown as FakeNode), "block") }
  }

  beforeEach(() => {
    vi.resetModules()
    FakeResizeObserver.all = []
    FakeResizeObserver.probe = null
    frameCallbacks = []
    heldEnds = null
    sizes.clear()
    vi.stubGlobal("ResizeObserver", FakeResizeObserver)
    vi.stubGlobal("MessageChannel", FakeMessageChannel)
    vi.stubGlobal("MutationObserver", FakeMutationObserver)
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frameCallbacks.push(callback))
    vi.stubGlobal("getComputedStyle", (target: object) => {
      const { width, height, padding = 0, missedBy = 0 } = sizes.get(target) ?? { width: 0, height: 0 }
      const side = `${padding}px`
      return {
        display: "block",
        boxSizing: "border-box",
        width: `${width + missedBy}px`,
        height: `${height}px`,
        paddingLeft: side,
        paddingRight: side,
        paddingTop: side,
        paddingBottom: side,
      }
    })
    // the page the changes are watched in: its root element, the body inside it
    const html = node(node(null, null, 9))
    sizes.set(html, { width: 1000, height: 800 })
    vi.stubGlobal("document", { documentElement: html })
    body = node(html)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.doUnmock("@/lib/errors")
  })

  it("lets an element go at once on a change of the delivery, and observes it anew as the delivery ends", async () => {
    // WebKit renders a commit of React that waited in a microtask after the callback, in the rendering update of the
    // report: an element it resized after its report would be an observation that update skips, a ResizeObserver loop
    const { observeSize } = await import("@/lib/observe-size")
    const reports: Element[][] = []
    const prologue = element(body, "prologue")
    const observer = observeSize(entries => {
      reports.push(entries.map(entry => entry.target))
      grow(prologue)
    })
    observer.observe(prologue.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(prologue.element)
    expect(reports).toEqual([[prologue.element]])
    expect(resizeObserver.observed.has(prologue.element)).toBe(false)
    await nextTask()
    // observed anew: its first report gives its size as it is now
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)

    // between deliveries a change is the observer's to report, as any change: the page is not watched then
    grow(prologue)
    await microtasks()
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)
  })

  it("observes an element the delivery let go anew at the next frame, when that frame comes before the next task", async () => {
    // a loaded machine renders the next frame before the next task: the element is observed again first, so that
    // frame's report has its size
    const { observeSize } = await import("@/lib/observe-size")
    const prologue = element(body, "prologue")
    observeSize(() => grow(prologue)).observe(prologue.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(prologue.element)
    await microtasks()
    expect(resizeObserver.observed.has(prologue.element)).toBe(false)
    nextFrame()
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)
  })

  it("leaves an element observed that a change after the delivery resizes: the next update delivers it in its own frame", async () => {
    // a render of React in a task after the delivery ended (the next task or frame): no update skips it, so the block keeps
    // its observation, and the frame that follows reports its height
    const { observeSize } = await import("@/lib/observe-size")
    const prologue = element(body, "prologue")
    observeSize(() => {}).observe(prologue.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(prologue.element)
    await microtasks()
    expect(FakeMutationObserver.current!.observing).toBe(true)
    nextFrame()
    // the page is watched no longer
    expect(FakeMutationObserver.current!.observing).toBe(false)
    grow(prologue)
    await microtasks()
    expect(resizeObserver.observed.has(prologue.element)).toBe(true)
  })

  /**
   * A view of ours in a row beside a sibling (a flex row: the sibling grows, the view shrinks), the sibling grown
   * `promises` promises after the callback of an observer of a panel, ours or a library's, its report `inChainOf`
   * microtasks after the report of another observer of ours (none when null); whether the view is observed after the
   * update, the delivery not ended yet, and once it ends
   */
  const siblingGrownLater = async (promises: number, of: "ours" | "a library", inChainOf: number | null = null) => {
    const { observeSize } = await import("@/lib/observe-size")
    const row = node(body, "row")
    const view = element(row, "view")
    const sibling = element(row, "sibling", 0)
    // a popup's, deeper than the view: the next iteration takes only what is deeper than the panel
    const panel = element(node(node(body)), "panel")
    observeSize(() => {}).observe(view.element)
    const grown = () =>
      promisesLater(promises, () => {
        resize(sibling.element, 60, 20)
        resize(view.element, 40, 20)
        commit(sibling.text)
      })
    if (of === "ours") {
      observeSize(grown).observe(panel.element)
    } else {
      new ResizeObserver(grown).observe(panel.element)
    }
    const [ours, ofPanel] = FakeResizeObserver.all
    const delivery = holdTheEnds()
    if (inChainOf !== null) {
      const other = element(node(node(body)), "other")
      observeSize(() => {}).observe(other.element)
      FakeResizeObserver.all.at(-1)!.report(other.element)
      await hops(inChainOf)
    }
    ofPanel!.report(panel.element)
    await aTask()
    const inTheUpdate = ours!.observed.has(view.element)
    delivery.end()
    return { inTheUpdate, atTheEnd: ours!.observed.has(view.element) }
  }

  it("lets the view go that a sibling grown three promises after the callback shrinks, though the change is not in it", async () => {
    // a commit of React that waited some promises after the callback is still in the update the delivery is in, and the
    // layout is the page's: the view, no deeper than the report, would be an observation the next iteration skips
    expect(await siblingGrownLater(3, "ours")).toEqual({ inTheUpdate: false, atTheEnd: true })
  })

  it("lets the view go that a sibling grown a hundred and twenty promises after the callback of a library shrinks, as floating-ui places a popup", async () => {
    // floating-ui places a popup with a flushSync after the awaits of its middleware, 42 to 87 microtasks after the
    // callback of its observer in the E2E suites, more after a reset of flip
    expect(await siblingGrownLater(120, "a library")).toEqual({ inTheUpdate: false, atTheEnd: true })
  })

  it("lets the view go that a sibling grown 255 promises after the callback shrinks, in the last microtask of the chain", async () => {
    // the boundary of the chain (`SPAN_MICROTASKS`): 256 microtasks, the last of them the one after the 255th promise
    expect(await siblingGrownLater(255, "ours")).toEqual({ inTheUpdate: false, atTheEnd: true })
  })

  it("leaves the view observed that a sibling grown 256 promises after the callback shrinks, the first past the chain", async () => {
    // past the chain the page is no longer taken for the update of the callback: a change lets go only the elements it is in
    expect(await siblingGrownLater(256, "ours")).toEqual({ inTheUpdate: true, atTheEnd: true })
  })

  it("makes the chain as long again from a callback that begins in the chain of another", async () => {
    // the engine runs a callback after the checkpoint of the one before: a test that reports in a chain still running
    // keeps the chain of the latest callback whole, not ended by the chain of the one before
    expect(await siblingGrownLater(120, "ours", 200)).toEqual({ inTheUpdate: false, atTheEnd: true })
  })

  it("lets every element of ours go that a change anywhere in the delivery can resize, and leaves one deeper than the report observed", async () => {
    // the layout is the page's: a block above the view that grows in the delivery of a report shrinks the view, a view
    // hidden around a table empties its cells, though neither change is in them
    const { observeSize } = await import("@/lib/observe-size")
    const view = node(body, "view")
    const cell = element(node(view), "cell")
    const deeper = element(node(node(node(view))), "deeper")
    const above = node(body, "above")
    const observer = observeSize(() => {
      resize(cell.element, 100, 0)
      resize(deeper.element, 100, 0)
      commit(above as unknown as Node)
    })
    observer.observe(cell.element)
    observer.observe(deeper.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(cell.element)
    await microtasks()
    expect(resizeObserver.observed.has(cell.element)).toBe(false)
    // the update takes it in at its depth, in that frame
    expect(resizeObserver.observed.has(deeper.element)).toBe(true)
    nextFrame()
    expect(resizeObserver.observed.has(cell.element)).toBe(true)
  })

  it("leaves an element deeper than the elements of the latest report observed: the update delivers it at its depth", async () => {
    // the scroller reported, and a render in its delivery grew a block inside it: the update takes the block in at its
    // depth in that frame, so the block stays observed, and the layout has its height before that frame paints
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    observeSize(() => {
      grow(block)
      grow(scroller)
    }).observe(scroller.element)
    observeSize(() => {}).observe(block.element)
    const [ofScroller, ofBlock] = FakeResizeObserver.all
    ofScroller!.report(scroller.element)
    await microtasks()
    expect(ofBlock!.observed.has(block.element)).toBe(true)
    // the scroller itself, resized at the depth of the report, leaves its observer
    expect(ofScroller!.observed.has(scroller.element)).toBe(false)
  })

  it("observes an element its report let go anew as a report of shallower elements shows it deeper than they are", async () => {
    // the blocks report first and their render grows one; the scroller reports next in the same iteration, which then
    // takes in only what is deeper than the scroller: the block is delivered in that frame, observed anew at once
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    observeSize(() => grow(block)).observe(block.element)
    observeSize(() => {}).observe(scroller.element)
    const [ofBlock, ofScroller] = FakeResizeObserver.all
    ofBlock!.report(block.element)
    await microtasks()
    expect(ofBlock!.observed.has(block.element)).toBe(false)
    ofScroller!.report(scroller.element)
    expect(ofBlock!.observed.has(block.element)).toBe(true)
  })

  it("keeps an element let go above a report of deeper elements let go when a report shallower than they are, but not than it, follows", async () => {
    // the main area above the table, resized in the delivery of the cells: a report of the rows after it is shallower
    // than the cells, not than the area, which the next iteration would skip
    const { observeSize } = await import("@/lib/observe-size")
    const area = element(body, "area")
    const rows = node(node(area.element as unknown as FakeNode))
    const row = element(rows, "row")
    const cell = element(node(node(row.element as unknown as FakeNode)), "cell")
    observeSize(() => grow(area)).observe(cell.element)
    observeSize(() => {}).observe(area.element)
    observeSize(() => {}).observe(row.element)
    const [ofCells, ofArea, ofRows] = FakeResizeObserver.all
    ofCells!.report(cell.element)
    await microtasks()
    expect(ofArea!.observed.has(area.element)).toBe(false)
    ofRows!.report(row.element)
    expect(ofArea!.observed.has(area.element)).toBe(false)
    nextFrame()
    expect(ofArea!.observed.has(area.element)).toBe(true)
  })

  it("observes an element first observed in the delivery no deeper than its report only as it ends, with no change to tell it", async () => {
    // the step row of a group a flushSync in a callback mounted, observed by the ref of its block at once: the update
    // would skip that first observation. One deeper than the report is observed at once: the update delivers it
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    const row = element(body, "step-row")
    const ofRows = observeSize(() => {})
    observeSize(() => {
      ofRows.observe(row.element)
      ofRows.observe(block.element)
    }).observe(scroller.element)
    const [rows, ofScroller] = FakeResizeObserver.all
    ofScroller!.report(scroller.element)
    expect(rows!.observed.has(row.element)).toBe(false)
    expect(rows!.observed.has(block.element)).toBe(true)
    await microtasks()
    nextFrame()
    expect(rows!.observed.has(row.element)).toBe(true)
  })

  it("lets go in a task before the delivery ends only the elements a change is in, and observes an element first observed there as it ends", async () => {
    // a render of React in a task that comes before the end of the delivery (a message of the backend) grows a cell, and
    // an effect observes a header in place: past the chain of the callback, so the cells of the other rows keep their
    // observations, and a sync of many rows lets go no cell of every row
    const { observeSize } = await import("@/lib/observe-size")
    const table = element(body, "table")
    const cells = Array.from({ length: 3 }, (_, index) => element(table.element as unknown as FakeNode, `cell-${index}`))
    const header = element(table.element as unknown as FakeNode, "header")
    const ofCells = observeSize(() => {})
    for (const cell of cells) {
      ofCells.observe(cell.element)
    }
    ofCells.observe(table.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    const delivery = holdTheEnds()
    resizeObserver.report(...cells.map(cell => cell.element))
    await aTask()
    grow(cells[0]!)
    ofCells.observe(header.element)
    await microtasks()
    expect(cells.map(cell => resizeObserver.observed.has(cell.element))).toEqual([false, true, true])
    // the table, above the change
    expect(resizeObserver.observed.has(table.element)).toBe(false)
    expect(resizeObserver.observed.has(header.element)).toBe(false)
    delivery.end()
    expect([...cells, table, header].filter(it => !resizeObserver.observed.has(it.element))).toEqual([])
  })

  /**
   * A view whose callback inserts a row (`mount` runs the insertion: at once, or in a microtask after it), and an observer
   * of the row that observes it as its ref does; after the delivery of the view, whether the row is observed
   */
  const rowMountedBy = async (mount: (insert: () => void) => void) => {
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "view")
    let row: { element: Element } | null = null
    let late: FakeResizeObserver | null = null
    observeSize(() =>
      mount(() => {
        row = element(body, "row")
        commit(row.element as unknown as Node)
        observeSize(() => {}).observe(row.element)
        late = FakeResizeObserver.all.at(-1)!
      }),
    ).observe(view.element)
    FakeResizeObserver.all[0]!.report(view.element)
    await microtasks()
    return () => late!.observed.has(row!.element)
  }

  it("observes a row a callback inserted and first observed only as the delivery ends", async () => {
    const observed = await rowMountedBy(insert => insert())
    expect(observed()).toBe(false)
    await nextTask()
    expect(observed()).toBe(true)
  })

  it("observes a row a commit of React inserted and first observed in a microtask after the callback only as the delivery ends", async () => {
    const observed = await rowMountedBy(insert => queueMicrotask(insert))
    expect(observed()).toBe(false)
    nextFrame()
    expect(observed()).toBe(true)
  })

  it("lets the view go on a change a commit of React makes in a microtask right after the callback", async () => {
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "view")
    const above = node(body, "above")
    observeSize(() => queueMicrotask(() => commit(above as unknown as Node))).observe(view.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(view.element)
    await microtasks()
    expect(resizeObserver.observed.has(view.element)).toBe(false)
    await nextTask()
    expect(resizeObserver.observed.has(view.element)).toBe(true)
  })

  it("lets an element its observer observes again in the delivery leave the engine's observer until the delivery ends", async () => {
    // a ref called again for an element observed already: the engine keeps its observation, which the update would skip
    // once a change resizes it, so it leaves the engine as the new one waits
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "view")
    const cell = element(body, "cell")
    const columns = observeSize(() => {})
    columns.observe(cell.element)
    const cells = FakeResizeObserver.all[0]!
    observeSize(() => {
      columns.observe(cell.element)
      grow(cell)
    }).observe(view.element)
    FakeResizeObserver.all[1]!.report(view.element)
    await microtasks()
    expect(cells.observed.has(cell.element)).toBe(false)
    await nextTask()
    expect(cells.observed.has(cell.element)).toBe(true)
  })

  it("keeps an element observed first in the delivery waiting when a report shallower than the delivery, but not than it, follows", async () => {
    // a row observed in the delivery of a report of deeper cells waits at its own depth: a report of the rows after it,
    // shallower than the cells, is not shallower than the row, which the next iteration would skip
    const { observeSize } = await import("@/lib/observe-size")
    const row = element(body, "row")
    const cell = element(node(node(row.element as unknown as FakeNode)), "cell")
    const between = element(node(row.element as unknown as FakeNode), "between")
    const ofRows = observeSize(() => {})
    observeSize(() => ofRows.observe(row.element)).observe(cell.element)
    observeSize(() => {}).observe(between.element)
    const [rows, ofCells, ofBetween] = FakeResizeObserver.all
    ofCells!.report(cell.element)
    expect(rows!.observed.has(row.element)).toBe(false)
    ofBetween!.report(between.element)
    expect(rows!.observed.has(row.element)).toBe(false)
    nextFrame()
    expect(rows!.observed.has(row.element)).toBe(true)
  })

  it("observes an element first observed microtasks after the callback, with no change, only as the delivery ends", async () => {
    // an effect three promises after the callback (as floating-ui's flushSync after the observer of Base UI) observes an
    // element in place: still in the update, with no change of its own to let it go
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "view")
    const row = element(body, "row")
    const ofRows = observeSize(() => {})
    observeSize(() => {
      void Promise.resolve()
        .then(() => undefined)
        .then(() => undefined)
        .then(() => ofRows.observe(row.element))
    }).observe(view.element)
    const [rows, ofView] = FakeResizeObserver.all
    ofView!.report(view.element)
    await microtasks()
    expect(rows!.observed.has(row.element)).toBe(false)
    nextFrame()
    expect(rows!.observed.has(row.element)).toBe(true)
  })

  it("observes the cells of a table mounted again in the delivery only as it ends", async () => {
    // a view shown again mounts its table in the commit of a callback: each new cell, observed by its ref, at the depth
    // of the cells the report had
    const { observeSize } = await import("@/lib/observe-size")
    const table = node(body, "table")
    const old = element(node(table), "old")
    const cells = Array.from({ length: 3 }, (_, index) => element(node(table), `cell-${index}`))
    const ofCells = observeSize(() => {
      commit(table as unknown as Node)
      for (const cell of cells) {
        ofCells.observe(cell.element)
      }
    })
    ofCells.observe(old.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(old.element)
    expect(cells.filter(cell => resizeObserver.observed.has(cell.element))).toEqual([])
    await nextTask()
    expect(cells.filter(cell => !resizeObserver.observed.has(cell.element))).toEqual([])
  })

  /** A cell its observer widens by a pixel at each report, in its delivery (a cycle), and `others` observers of it more */
  const widening = async (others = 0) => {
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const cell = element(body, "cell")
    let width = 100
    observeSize(() => {
      width++
      resize(cell.element, width, 20)
      commit(cell.text)
    }).observe(cell.element)
    for (let count = 0; count < others; count++) {
      observeSize(() => {}).observe(cell.element)
    }
    const all = FakeResizeObserver.all
    // a frame: its delivery, with a report of the cell or none, ended by the next frame
    const frame = async (reported = true) => {
      if (reported) {
        all[0]!.report(cell.element)
        expect(all.map(resizeObserver => resizeObserver.observed.has(cell.element))).toEqual(all.map(() => false))
        await microtasks()
      }
      nextFrame()
    }
    return { logError, frame }
  }

  it("logs a cycle once when its own callback resizes an element in its delivery in every frame", async () => {
    // frames, not time: a window that renders a frame a second tells the cycle in ten seconds
    const { logError, frame } = await widening()
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    expect(logError).not.toHaveBeenCalled()
    await frame()
    expect(logError).toHaveBeenCalledTimes(1)
    expect(logError.mock.calls[0]![0]).toBe("A size observer of the page cycles")
    expect(String(logError.mock.calls[0]![1])).toBe("Error: cell resized in its own delivery in 10 of 20 frames")
    // the cycle goes on: logged once
    for (let count = 0; count < 12; count++) {
      await frame()
    }
    expect(logError).toHaveBeenCalledTimes(1)
  })

  it("logs a cycle that resizes an element in the delivery of every other frame", async () => {
    // a callback that resizes its element only in the frame after a report of it: let go in every other frame
    const { logError, frame } = await widening()
    for (let count = 0; count < 9; count++) {
      await frame()
      await frame(false)
    }
    expect(logError).not.toHaveBeenCalled()
    await frame()
    expect(logError).toHaveBeenCalledTimes(1)
  })

  it("logs no cycle of an element let go three frames apart, nor of one whose releases a pause of twenty frames parts", async () => {
    const { logError, frame } = await widening()
    for (let count = 0; count < 15; count++) {
      await frame()
      await frame(false)
      await frame(false)
    }
    for (let count = 0; count < 20; count++) {
      await frame(false)
    }
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    for (let count = 0; count < 20; count++) {
      await frame(false)
    }
    for (let count = 0; count < 9; count++) {
      await frame()
    }
    expect(logError).not.toHaveBeenCalled()
  })

  it("counts an element its observers let go in one delivery once", async () => {
    // a diff view has three observers: a change of it in a delivery lets it go from each, which is one frame
    const { logError, frame } = await widening(2)
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
    // a text that arrives in a task after each update resizes a block, deeper than the view
    const block = element(node(body), "block")
    observeSize(() => {}).observe(block.element)
    const [ofView, ofBlock] = FakeResizeObserver.all
    // each frame ends the delivery before the next task, as on a loaded machine: the text comes in a task before
    holdTheEnds()
    for (let count = 0; count < 30; count++) {
      nextFrame()
      resize(view.element, 100 + count, 400)
      ofView!.report(view.element)
      ofBlock!.report(block.element)
      await aTask()
      resize(block.element, 100, 20 + count)
      commit(block.text)
      await microtasks()
    }
    nextFrame()
    expect(logError).not.toHaveBeenCalled()
  })

  it("logs no cycle of an element a drag resizes between frames where the arithmetic lays it out otherwise than the engine", async () => {
    // at a zoom of the app other than 100% the computed style misses the box the engine reports: the release is measured
    // against the layout of the element as reported, one arithmetic for both, so the drag's size reads as no change
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "diff-view")
    sizes.set(view.element, { width: 100, height: 400, missedBy: 0.4 })
    observeSize(() => commit(view.text)).observe(view.element)
    const [ofView] = FakeResizeObserver.all
    for (let count = 0; count < 30; count++) {
      nextFrame()
      resize(view.element, 100 + count, 400)
      ofView!.report(view.element)
      await microtasks()
    }
    nextFrame()
    expect(logError).not.toHaveBeenCalled()
  })

  it("names at a loop an element whose content box changed inside the same border box, as the engine compares it", async () => {
    // an observation compares the content box by default: a padding given to a box of a fixed size resizes its content
    const panel = await reportedPanel()
    expect(await loopNow()).toEqual([])
    resize(panel.element, 100, 20, 4)
    expect(await loopNow()).toEqual(["panel"])
  })

  it("compares the box an observation observes alone, as the engine does", async () => {
    // the columns observe the border box of their items: a padding that resizes the content alone is no change of theirs
    const { observeSize } = await import("@/lib/observe-size")
    const item = element(body, "item")
    const panel = element(body, "panel")
    const ofItems = observeSize(() => {})
    ofItems.observe(item.element, { box: "border-box" })
    observeSize(() => {}).observe(panel.element)
    FakeResizeObserver.all[0]!.report(item.element)
    FakeResizeObserver.all[1]!.report(panel.element)
    resize(item.element, 100, 20, 4)
    resize(panel.element, 100, 20, 4)
    expect(await loopNow()).toEqual(["panel"])
  })

  it("logs no cycle of an element another callback lets go before its own report in every frame, as a drag resized it", async () => {
    // a drag of the divider resizes the view before each frame; the blocks report first and render: the view leaves its
    // observer before its report of that frame, which says the drag's size, no change of a callback of ours
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const view = element(body, "view")
    const block = element(body, "block")
    observeSize(() => commit(block.text)).observe(block.element)
    observeSize(() => {}).observe(view.element)
    const [ofBlocks] = FakeResizeObserver.all
    for (let count = 0; count < 30; count++) {
      resize(view.element, 100 + count, 400)
      ofBlocks!.report(block.element)
      await microtasks()
      nextFrame()
    }
    expect(logError).not.toHaveBeenCalled()
  })

  it("names at a loop the elements of ours resized since their report, or observed with a size and never reported", async () => {
    const { observeSize } = await import("@/lib/observe-size")
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
    expect(await loopNow()).toEqual(["resized", "fresh"])
  })

  it("names no element at a loop that the engine lays out as reported, where the arithmetic lays it out otherwise, as at a zoom of the app", async () => {
    // WebKit at a zoom other than 100% lays out borders and paddings on a grid of its own: the computed style misses the
    // box it reports by a fraction of a pixel, so the engine's own boxes decide, from the probe's report in the next update
    const panel = await reportedPanel(0.4)
    expect(await loopNow()).toEqual([])
    expect(FakeResizeObserver.probe!.observed.size).toBe(0)
    resize(panel.element, 101, 20)
    expect(await loopNow()).toEqual(["panel"])
  })

  it("names no element at a loop whose content box is empty, its border box laid out by the engine as reported", async () => {
    // the probe observes the border box, which an element of a padding alone has: it reports what a content box would not
    const { observeSize } = await import("@/lib/observe-size")
    const spacer = element(body, "spacer")
    sizes.set(spacer.element, { width: 8, height: 8, padding: 4, missedBy: 0.4 })
    observeSize(() => {}).observe(spacer.element, { box: "border-box" })
    FakeResizeObserver.all[0]!.report(spacer.element)
    expect(await loopNow()).toEqual([])
  })

  it("names no element at a loop whose border box, which its observation observes, the engine lays out as reported", async () => {
    // the columns observe the border box of their items: a padding that resizes the content alone is no change of theirs
    const { observeSize } = await import("@/lib/observe-size")
    const item = element(body, "item")
    sizes.set(item.element, { width: 100, height: 20, missedBy: 0.4 })
    observeSize(() => {}).observe(item.element, { box: "border-box" })
    FakeResizeObserver.all[0]!.report(item.element)
    resize(item.element, 100, 20, 4)
    expect(await loopNow()).toEqual([])
  })

  it("tells nothing at a loop of an element a task resized before the probe reported, pending or not", async () => {
    // at a zoom of the app the arithmetic proposes elements laid out as reported; a render of a task before the next
    // update resizes one: the engine's boxes then say nothing of the loop, and an error of the page would be false
    const { loopOfOurs } = await import("@/lib/resize-loops")
    const panel = await reportedPanel(0.4)
    const told: (readonly string[])[] = []
    loopOfOurs(pending => told.push(pending))
    resize(panel.element, 90, 20)
    FakeResizeObserver.probe!.reportObserved()
    // one pending at the loop, resized again before the update: no error either, the rarer and the safer way
    loopOfOurs(pending => told.push(pending))
    resize(panel.element, 80, 20)
    FakeResizeObserver.probe!.reportObserved()
    expect(told).toEqual([[], []])
  })

  it("tells nothing at a loop of an element that left the page before the probe reported, as in a remount", async () => {
    const { loopOfOurs } = await import("@/lib/resize-loops")
    const panel = await reportedPanel(0.4)
    const told: (readonly string[])[] = []
    loopOfOurs(pending => told.push(pending))
    ;(panel.element as unknown as FakeNode).isConnected = false
    FakeResizeObserver.probe!.reportObserved()
    expect(told).toEqual([[]])
  })

  it("names an element of ours at a loop that has no box any more, which the probe does not report", async () => {
    const panel = await reportedPanel()
    resize(panel.element, 0, 0)
    expect(await loopNow()).toEqual(["panel"])
  })

  it("tells the loops in their order: a loop of a library alone after one that waits for the probe waits as well", async () => {
    const { loopOfOurs } = await import("@/lib/resize-loops")
    const panel = await reportedPanel()
    resize(panel.element, 120, 20)
    const told: string[] = []
    loopOfOurs(pending => told.push(`first: ${pending.join(", ")}`))
    // the panel reported again: a loop of a library alone now, its verdict after the first, which keeps the boxes of its loop
    FakeResizeObserver.all[0]!.report(panel.element)
    loopOfOurs(pending => told.push(`second: ${pending.join(", ")}`))
    expect(told).toEqual([])
    FakeResizeObserver.probe!.reportObserved()
    expect(told).toEqual(["first: panel", "second: "])
  })

  it("lets an element of ours go that the callback of a library resizes in its delivery, in a frame without a report of ours", async () => {
    // the observer of a panel of a library renders a change of the view in its callback: the update delivered the panel,
    // and the view, no deeper, would be an observation it skips
    const { observeSize } = await import("@/lib/observe-size")
    const panel = element(body, "panel")
    const view = element(body, "view")
    observeSize(() => {}).observe(view.element)
    const library = new ResizeObserver(() => grow(view))
    library.observe(panel.element)
    const [ours, theirs] = FakeResizeObserver.all
    theirs!.report(panel.element)
    await microtasks()
    expect(ours!.observed.has(view.element)).toBe(false)
    nextFrame()
    expect(ours!.observed.has(view.element)).toBe(true)
  })

  it("takes the depth of a report of a library deeper than ours as the depth of the delivery", async () => {
    // ours reported the scroller; the library reports a list deeper inside it and its callback resizes a block between
    // them: the next iteration takes only what is deeper than the list, so the block leaves our observer
    const { observeSize } = await import("@/lib/observe-size")
    const { scroller, block } = scrollerWithBlock()
    const list = element(node(node(block.element as unknown as FakeNode)), "list")
    observeSize(() => {}).observe(scroller.element)
    observeSize(() => {}).observe(block.element)
    const library = new ResizeObserver(() => grow(block))
    library.observe(list.element)
    const [ofScroller, ofBlock, theirs] = FakeResizeObserver.all
    ofScroller!.report(scroller.element)
    await microtasks()
    theirs!.report(list.element)
    await microtasks()
    expect(ofBlock!.observed.has(block.element)).toBe(false)
  })

  it("gives a library the engine's observer: its class, prototype and methods, and its callback the same entries and observer", async () => {
    await import("@/lib/observe-size")
    expect(globalThis.ResizeObserver).not.toBe(FakeResizeObserver)
    expect(globalThis.ResizeObserver.prototype).toBe(FakeResizeObserver.prototype)
    const reports: [ResizeObserverEntry[], ResizeObserver][] = []
    const library = new ResizeObserver((entries, observer) => reports.push([entries, observer]))
    expect(library).toBeInstanceOf(FakeResizeObserver)
    expect(library).toBeInstanceOf(ResizeObserver)
    expect(Object.getPrototypeOf(library)).toBe(FakeResizeObserver.prototype)
    // a subclass of the global is a subclass still
    class Measuring extends ResizeObserver {
      measured = true
    }
    expect(new Measuring(() => {})).toMatchObject({ measured: true })
    expect(new Measuring(() => {})).toBeInstanceOf(Measuring)
    const panel = element(body, "panel")
    library.observe(panel.element)
    FakeResizeObserver.all[0]!.report(panel.element)
    expect(reports).toHaveLength(1)
    expect(reports[0]![0].map(entry => entry.target)).toEqual([panel.element])
    expect(reports[0]![1]).toBe(library)
    expect(() => (ResizeObserver as unknown as (callback: ResizeObserverCallback) => void)(() => {})).toThrow(TypeError)
  })

  it("leaves an element it no longer observes alone", async () => {
    const { observeSize } = await import("@/lib/observe-size")
    const observer = observeSize(() => {})
    const gone = element(body, "gone")
    observer.observe(gone.element)
    const resizeObserver = FakeResizeObserver.all[0]!
    resizeObserver.report(gone.element)
    observer.unobserve(gone.element)
    grow(gone)
    await nextTask()
    expect(resizeObserver.observed.has(gone.element)).toBe(false)
  })
})
