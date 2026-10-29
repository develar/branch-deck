import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { ColumnRegistry } from "@/lib/branch-columns"

/** The one observer of a registry: the test reports the sizes the browser would */
class FakeResizeObserver {
  static current: FakeResizeObserver | null = null

  constructor(private readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.current = this
  }

  observe(): void {}

  unobserve(): void {}

  disconnect(): void {}
}

/** An item as laid out: its width now, which the observer reports later (`report`) */
interface Item {
  width: number
  element: Element
}

function item(width: number): Item {
  const it: Item = { width, element: {} as Element }
  it.element = { getBoundingClientRect: () => ({ width: it.width }) } as unknown as Element
  return it
}

/** The observer's report of the items as they are laid out now */
function report(...items: Item[]): void {
  const observer = FakeResizeObserver.current!
  const entries = items.map(it => ({ target: it.element, borderBoxSize: [{ inlineSize: it.width }], contentRect: { width: it.width } }))
  ;(observer as unknown as { callback: ResizeObserverCallback }).callback(entries as unknown as ResizeObserverEntry[], observer as unknown as ResizeObserver)
}

/** The root of a view: the variables written on it, each write counted (each restyles every row of the view) */
function root(): { element: HTMLElement; variables: Map<string, string>; writes: string[] } {
  const variables = new Map<string, string>()
  const writes: string[] = []
  const style = {
    setProperty: (name: string, value: string) => {
      variables.set(name, value)
      writes.push(`${name}=${value}`)
    },
  }
  return { element: { style, toggleAttribute: () => true } as unknown as HTMLElement, variables, writes }
}

describe("ColumnRegistry", () => {
  beforeEach(() => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("writes no track before the observer measured an item, then every track at once", () => {
    const registry = new ColumnRegistry()
    const view = root()
    registry.setRoot(view.element)
    const status = item(98.36)
    registry.observe(status.element, "state")
    registry.setLayout(null, true)
    // the first commit: nothing measured yet, a write of the zeros would restyle every row for none
    expect(view.writes).toEqual([])

    report(status)
    expect(view.variables.get("--bd-track-state")).toBe("98.36px")
    expect(view.variables.get("--bd-track-commits")).toBe("0px")
  })

  it("only widens a track while held, and follows the items as laid out once the hold ends", () => {
    const registry = new ColumnRegistry()
    const view = root()
    registry.setRoot(view.element)
    const syncing = item(98.36)
    registry.observe(syncing.element, "state")
    registry.setLayout(null, true)
    report(syncing)

    // the run ends for the branch ("syncing 0/1…" to "created"): the track keeps its width
    syncing.width = 82.97
    report(syncing)
    expect(view.variables.get("--bd-track-state")).toBe("98.36px")
    // a robot comes, wider: the track widens
    syncing.width = 106.61
    report(syncing)
    expect(view.variables.get("--bd-track-state")).toBe("106.61px")

    // the robot ends in the commit that ends the hold: the track follows it at once, before the observer reports it
    syncing.width = 82.97
    registry.setLayout(null, false)
    expect(view.variables.get("--bd-track-state")).toBe("82.97px")
    report(syncing)
    expect(view.writes.filter(write => write.startsWith("--bd-track-state="))).toEqual(["--bd-track-state=98.36px", "--bd-track-state=106.61px", "--bd-track-state=82.97px"])

    // not held: the track follows its content
    syncing.width = 60
    report(syncing)
    expect(view.variables.get("--bd-track-state")).toBe("60px")
  })

  it("keeps a track at its floor until the floors go, then at what its items show", () => {
    const registry = new ColumnRegistry()
    const view = root()
    registry.setRoot(view.element)
    const step = item(0)
    registry.observe(step.element, "step")
    registry.setLayout({ step: 110.63 }, true)
    // the first commit of a start stands where the settled view of the last time stood
    expect(view.variables.get("--bd-track-step")).toBe("110.63px")
    expect(view.variables.get("--bd-gap-step")).toBe("0.5rem")

    step.width = 100.03
    report(step)
    expect(view.variables.get("--bd-track-step")).toBe("110.63px")

    registry.setLayout(null, false)
    expect(view.variables.get("--bd-track-step")).toBe("100.03px")
  })
})
