import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000)
})

afterEach(() => {
  vi.useRealTimers()
})

const gates = createRefreshGates({
  isDue: (reason, sinceStart, sinceAnswer) => reason === "manual" || (reason === "sync" && sinceStart >= 1_000) || (reason === "focus" && sinceAnswer >= 5_000),
  followUpReasons: new Set<RefreshReason>(["manual"]),
})

describe("createRefreshGates", () => {
  it("asks when the policy says so, per key", async () => {
    gates.reset()
    const reasons: string[] = []
    const task = async (reason: RefreshReason) => {
      reasons.push(reason)
    }
    await gates.refresh("a", "sync", 0, task)
    await gates.refresh("a", "sync", 0, task)
    await gates.refresh("b", "sync", 0, task)
    vi.advanceTimersByTime(1_000)
    await gates.refresh("a", "sync", 0, task)
    // the age of the answer, not of the request
    await gates.refresh("a", "focus", Date.now() - 4_999, task)
    expect(reasons).toEqual(["sync", "sync", "sync"])
  })

  it("runs one task per key; a follow-up reason asks once more after it, with its reason", async () => {
    gates.reset()
    const reasons: string[] = []
    let release: () => void = () => {}
    const task = async (reason: RefreshReason) => {
      reasons.push(reason)
      if (reasons.length === 1) {
        await new Promise<void>(resolve => {
          release = resolve
        })
      }
    }
    const first = gates.refresh("a", "sync", 0, task)
    void gates.refresh("a", "sync", 0, task)
    void gates.refresh("a", "manual", 0, task)
    void gates.refresh("a", "manual", 0, task)
    expect(reasons).toEqual(["sync"])
    release()
    await first
    expect(reasons).toEqual(["sync", "manual"])
  })
})
