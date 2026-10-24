import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import { batchTimer } from "@/lib/batch-timer"

/** One turn of the task queue: a message posted before it has run */
function nextTask(): Promise<void> {
  return new Promise(resolve => {
    const channel = new MessageChannel()
    channel.port1.onmessage = () => {
      channel.port1.close()
      resolve()
    }
    channel.port2.postMessage(null)
  })
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe("a batch timer", () => {
  it("runs a batch once after its delay in a shown window, and not after a cancel", () => {
    vi.useFakeTimers()
    vi.stubGlobal("document", { visibilityState: "visible" })
    const run = vi.fn()
    const timer = batchTimer(run, 16)
    timer.schedule()
    timer.schedule()
    vi.advanceTimersByTime(15)
    expect(run).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(run).toHaveBeenCalledTimes(1)
    timer.schedule()
    timer.cancel()
    vi.advanceTimersByTime(16)
    expect(run).toHaveBeenCalledTimes(1)
  })

  it("runs a batch on the next task, no timer, while the window never showed, then on its timer once it showed", async () => {
    // the timers of a hidden page wait for a second
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] })
    const page = { visibilityState: "hidden" }
    vi.stubGlobal("document", page)
    const run = vi.fn()
    const timer = batchTimer(run, 16)
    timer.schedule()
    timer.schedule()
    await nextTask()
    expect(run).toHaveBeenCalledTimes(1)
    // a canceled batch of the hidden window does not run
    timer.schedule()
    timer.cancel()
    await nextTask()
    expect(run).toHaveBeenCalledTimes(1)

    page.visibilityState = "visible"
    timer.schedule()
    await nextTask()
    expect(run).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(16)
    expect(run).toHaveBeenCalledTimes(2)
    // hidden again (minimized): a shown window keeps its timer
    page.visibilityState = "hidden"
    timer.schedule()
    await nextTask()
    expect(run).toHaveBeenCalledTimes(2)
    vi.advanceTimersByTime(16)
    expect(run).toHaveBeenCalledTimes(3)
  })
})
