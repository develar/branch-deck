import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { ColumnRegistry } from "@/lib/branch-columns"

/**
 * The one observer of a registry: the test reports the sizes the browser would, by hand (`report`) or as a rendering
 * update does (`renderingUpdate`): a target whose size differs from the one it reported last, 0 once observed. An
 * observe of a target observed already changes nothing, as in Chromium and WebKit
 */
class FakeResizeObserver {
  static current: FakeResizeObserver | null = null
  readonly reported = new Map<Element, number>()

  constructor(private readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.current = this
  }

  observe(target: Element): void {
    if (!this.reported.has(target)) {
      this.reported.set(target, 0)
    }
  }

  unobserve(target: Element): void {
    this.reported.delete(target)
  }

  disconnect(): void {
    this.reported.clear()
  }
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

/** The observer's report of the elements as they are laid out now */
function deliver(elements: Element[]): void {
  const observer = FakeResizeObserver.current!
  const entries = elements.map(element => {
    const width = element.getBoundingClientRect().width
    observer.reported.set(element, width)
    const size = [{ inlineSize: width }]
    return { target: element, borderBoxSize: size, contentBoxSize: size, contentRect: { width } }
  })
  ;(observer as unknown as { callback: ResizeObserverCallback }).callback(entries as unknown as ResizeObserverEntry[], observer as unknown as ResizeObserver)
}

/** The observer's report of the items as they are laid out now */
function report(...items: Item[]): void {
  deliver(items.map(it => it.element))
}

/** A rendering update: the observer reports every observed element whose size is not the one it reported last */
function renderingUpdate(): void {
  const changed = [...FakeResizeObserver.current!.reported].filter(([element, width]) => element.getBoundingClientRect().width !== width).map(([element]) => element)
  if (changed.length > 0) {
    deliver(changed)
  }
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
  // as wide as a window of 768px at its least
  return { element: { style, toggleAttribute: () => true, getBoundingClientRect: () => ({ width: 752 }) } as unknown as HTMLElement, variables, writes }
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

  it("follows an item read wider as the hold ends and back at its reported width before the next rendering update", () => {
    // the root's report compares the view with 56rem
    vi.stubGlobal("document", { documentElement: {} })
    vi.stubGlobal("getComputedStyle", () => ({ fontSize: "16px" }))
    const registry = new ColumnRegistry()
    const view = root()
    registry.setRoot(view.element)
    const state = item(98.36)
    const step = item(0)
    registry.observe(state.element, "state")
    registry.observe(step.element, "step")
    registry.setLayout(null, true)
    renderingUpdate()
    expect(view.variables.get("--bd-track-step")).toBe("0px")

    // the commit that ends the sync shows "Checking…" in the step slot of every row: read as laid out, the track
    // follows it at once
    step.width = 100.03
    state.width = 82.97
    registry.setLayout(null, false)
    expect(view.variables.get("--bd-track-step")).toBe("100.03px")
    // the fact comes before the next rendering update and the slot is empty again, the size the observer reported
    // last: no row shows a step, the track takes no room
    step.width = 0
    renderingUpdate()
    expect(view.variables.get("--bd-track-step")).toBe("0px")
    expect(view.variables.get("--bd-gap-step")).toBe("0px")
    expect(view.variables.get("--bd-track-state")).toBe("82.97px")

    // a read the observer reports at its next rendering update stands
    step.width = 100.03
    registry.setLayout({ step: 110.63 }, false)
    registry.setLayout(null, false)
    renderingUpdate()
    expect(view.variables.get("--bd-track-step")).toBe("100.03px")
  })
})
