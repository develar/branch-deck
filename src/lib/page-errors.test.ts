import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"

const RESIZE_LOOP = "ResizeObserver loop completed with undelivered notifications."

const log = vi.hoisted(() => ({ error: vi.fn(() => Promise.resolve()), warn: vi.fn(() => Promise.resolve()) }))
vi.mock("@tauri-apps/plugin-log", () => log)

describe("the errors of the page", () => {
  let onError: (event: Partial<ErrorEvent>) => void
  let now = 0

  beforeEach(async () => {
    vi.resetModules()
    log.error.mockClear()
    log.warn.mockClear()
    now = 0
    vi.spyOn(performance, "now").mockImplementation(() => now)
    vi.spyOn(console, "warn").mockImplementation(() => {})
    vi.spyOn(console, "error").mockImplementation(() => {})
    vi.stubGlobal("window", {
      addEventListener: (type: string, listener: (event: Partial<ErrorEvent>) => void) => {
        if (type === "error") {
          onError = listener
        }
      },
    })
    await import("@/lib/page-errors")
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  /** A loop the engine reports at `at` ms */
  function loopAt(at: number): void {
    now = at
    onError({ error: null, message: RESIZE_LOOP })
  }

  it("logs a loop of the observers of a library as a warning, and three within five seconds as an error, however slowly they repeat", () => {
    loopAt(0)
    loopAt(2400)
    expect(log.warn).toHaveBeenCalledTimes(2)
    expect(log.error).not.toHaveBeenCalled()
    // the third within five seconds: a loop that repeats every 2.4 s renders the window again and again
    loopAt(4800)
    expect(log.error).toHaveBeenCalledWith("A ResizeObserver loop repeats in the page", { keyValues: expect.objectContaining({ error: "3 loops within 5 s" }) })
  })

  it("logs loops five seconds apart as warnings", () => {
    for (const at of [0, 5000, 10_000, 15_000]) {
      loopAt(at)
    }
    expect(log.warn).toHaveBeenCalledTimes(4)
    expect(log.error).not.toHaveBeenCalled()
  })

  it("logs a loop that left an element of the observers of the page pending as an error at once", async () => {
    const { attributeLoopsWith } = await import("@/lib/resize-loops")
    attributeLoopsWith(() => ["diff-view-prologue"])
    loopAt(0)
    expect(log.warn).not.toHaveBeenCalled()
    expect(log.error).toHaveBeenCalledWith("A ResizeObserver loop of an observer of the page", {
      keyValues: expect.objectContaining({ error: "left diff-view-prologue pending" }),
    })
  })
})
