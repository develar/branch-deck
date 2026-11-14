import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"

const RESIZE_LOOP = "ResizeObserver loop completed with undelivered notifications."

const log = vi.hoisted(() => ({ error: vi.fn((_message: string, _options?: unknown) => Promise.resolve()), warn: vi.fn((_message: string) => Promise.resolve()) }))
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

  /** A loop the engine reports at `at` ms: whether the page took it as its own (no uncaught error of the console) */
  function loopAt(at: number): boolean {
    now = at
    let prevented = false
    onError({ error: null, message: RESIZE_LOOP, preventDefault: () => (prevented = true) })
    return prevented
  }

  it("logs a loop of the observers of a library as a warning, and three within five seconds as an error, however slowly they repeat", () => {
    // the page has the loop as its own: the engine tells no console of it as an uncaught error
    expect(loopAt(0)).toBe(true)
    loopAt(2400)
    expect(log.warn).toHaveBeenCalledTimes(1)
    expect(log.error).not.toHaveBeenCalled()
    // the third within five seconds: a loop that repeats every 2.4 s renders the window again and again
    loopAt(4800)
    expect(log.error).toHaveBeenCalledWith("A ResizeObserver loop repeats in the page", { keyValues: expect.objectContaining({ error: "3 loops within 5 s" }) })
  })

  it("logs loops five seconds apart as warnings, one in ten seconds at most with the count of those it left out", () => {
    for (const at of [0, 5000, 10_000, 15_000]) {
      loopAt(at)
    }
    expect(log.error).not.toHaveBeenCalled()
    expect(log.warn.mock.calls.map(call => String(call[0]))).toEqual([
      `A ResizeObserver loop of the page: ${RESIZE_LOOP}`,
      `A ResizeObserver loop of the page: ${RESIZE_LOOP}; 1 more since the last line`,
    ])
  })

  it("logs a loop that left an element of the observers of the page pending as an error at once, then once in ten seconds with the count", async () => {
    const { attributeLoopsWith } = await import("@/lib/resize-loops")
    attributeLoopsWith(() => ["diff-view-prologue"])
    // a loop in every frame of a second
    for (let frame = 0; frame < 60; frame++) {
      loopAt(frame * 16)
    }
    expect(log.warn).not.toHaveBeenCalled()
    expect(log.error).toHaveBeenCalledTimes(1)
    expect(log.error).toHaveBeenCalledWith("A ResizeObserver loop of an observer of the page", {
      keyValues: expect.objectContaining({ error: "left diff-view-prologue pending" }),
    })
    loopAt(10_000)
    expect(log.error).toHaveBeenCalledTimes(2)
    expect(log.error).toHaveBeenLastCalledWith("A ResizeObserver loop of an observer of the page", {
      keyValues: expect.objectContaining({ error: "left diff-view-prologue pending; 59 more since the last line" }),
    })
  })
})
