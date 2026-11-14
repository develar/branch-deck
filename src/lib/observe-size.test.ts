import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"

/** The one `ResizeObserver` of an observer: what it observes, and its callback to report by hand */
class FakeResizeObserver {
  static current: FakeResizeObserver | null = null
  readonly observed = new Set<Element>()

  constructor(readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.current = this
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

  /** A report of `targets`, as a rendering update gives it */
  report(...targets: Element[]): void {
    this.callback(
      targets.map(target => ({ target }) as unknown as ResizeObserverEntry),
      this as unknown as ResizeObserver,
    )
  }
}

/** The observer of the changes of the observed subtrees: the test tells the records of a commit by hand */
class FakeMutationObserver {
  static current: FakeMutationObserver | null = null

  constructor(private readonly callback: MutationCallback) {
    FakeMutationObserver.current = this
  }

  observe(): void {}

  /** A commit of React changed `nodes` */
  commit(...nodes: Node[]): void {
    this.callback(
      nodes.map(target => ({ target }) as unknown as MutationRecord),
      this as unknown as MutationObserver,
    )
  }
}

/** The next task, after which a rendering update is over */
function nextTask(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 0))
}

/** An element, and a text inside it */
function element(): { element: Element; text: Node } {
  const it = { parentNode: null } as unknown as Element
  return { element: it, text: { parentNode: it } as unknown as Node }
}

describe("observeSize", () => {
  beforeEach(() => {
    vi.resetModules()
    vi.stubGlobal("ResizeObserver", FakeResizeObserver)
    vi.stubGlobal("MutationObserver", FakeMutationObserver)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("lets an element a commit changes in the rendering update of a report go at once, and observes it anew in the next task", async () => {
    // WebKit renders a commit of React that waited in a microtask after the callback, in the rendering update of the
    // report: an element it resized after its report would be an observation that update skips, a ResizeObserver loop
    const { observeSize } = await import("@/lib/observe-size")
    const reports: Element[][] = []
    const observer = observeSize(entries => reports.push(entries.map(entry => entry.target)))
    const prologue = element()
    observer.observe(prologue.element)
    const resize = FakeResizeObserver.current!
    resize.report(prologue.element)
    expect(reports).toEqual([[prologue.element]])

    FakeMutationObserver.current!.commit(prologue.text)
    expect(resize.observed.has(prologue.element)).toBe(false)
    await nextTask()
    // observed anew: its first report gives its size as it is now
    expect(resize.observed.has(prologue.element)).toBe(true)

    // after the rendering update a change is the observer's to report, as any change
    resize.observed.delete(prologue.element)
    resize.observed.add(prologue.element)
    FakeMutationObserver.current!.commit(prologue.text)
    expect(resize.observed.has(prologue.element)).toBe(true)
  })

  it("counts an element its observers let go in one commit once, and logs a cycle only when it is let go in every frame of a second", async () => {
    // a diff view has three observers: a commit of it after a report lets it go from each, which is one frame
    const logError = vi.fn()
    vi.doMock("@/lib/errors", () => ({ logError }))
    const { observeSize } = await import("@/lib/observe-size")
    const view = element()
    Reflect.set(view.element, "getAttribute", (name: string) => (name === "data-testid" ? "diff-view" : null))
    const resizes = [0, 1, 2].map(() => {
      observeSize(() => {}).observe(view.element)
      return FakeResizeObserver.current!
    })
    const frame = async () => {
      resizes[0]!.report(view.element)
      FakeMutationObserver.current!.commit(view.text)
      expect(resizes.map(resize => resize.observed.has(view.element))).toEqual([false, false, false])
      await nextTask()
    }
    for (let count = 0; count < 29; count++) {
      await frame()
    }
    expect(logError).not.toHaveBeenCalled()
    await frame()
    expect(logError).toHaveBeenCalledTimes(1)
    expect(logError.mock.calls[0]![0]).toBe("A size observer of the page cycles")
    expect(String(logError.mock.calls[0]![1])).toBe("Error: diff-view resized in every frame of a second")
  })

  it("leaves an element it no longer observes alone", async () => {
    const { observeSize } = await import("@/lib/observe-size")
    const observer = observeSize(() => {})
    const gone = element()
    observer.observe(gone.element)
    const resize = FakeResizeObserver.current!
    resize.report(gone.element)
    observer.unobserve(gone.element)
    FakeMutationObserver.current!.commit(gone.text)
    await nextTask()
    expect(resize.observed.has(gone.element)).toBe(false)
  })
})
