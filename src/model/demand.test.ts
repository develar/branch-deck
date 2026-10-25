import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { SetDemandParams } from "@/lib/bindings"
import { DEMAND_SEND_INTERVAL_MS, needBranch, resetDemand, setDemandFocused, setDemandSubscription, showBranch, showRobotDetails } from "@/model/demand"

const { sent } = vi.hoisted(() => ({ sent: [] as SetDemandParams[] }))
vi.mock("@/lib/bindings", () => ({
  commands: {
    setDemand: async (params: SetDemandParams) => {
      sent.push(params)
    },
  },
}))

beforeEach(() => {
  vi.useFakeTimers()
  sent.length = 0
  resetDemand()
})

afterEach(() => {
  resetDemand()
  vi.useRealTimers()
})

describe("the demand of the window", () => {
  it("goes to the subscription of the window once it has one, each surface counted, the lists sorted", async () => {
    const releaseRow = showBranch("ui")
    const releaseOther = showBranch("auth")
    const releaseSecond = showBranch("ui")
    needBranch("auth")
    await vi.runAllTimersAsync()
    expect(sent).toEqual([])

    setDemandSubscription(7)
    await vi.runAllTimersAsync()
    expect(sent).toHaveLength(1)
    expect(sent[0]).toMatchObject({ subscription: 7, demand: { focused: false, visibleBranches: ["auth", "ui"], needed: ["auth"], robotDetails: [] } })

    // two surfaces of ui: it stays while one holds it
    releaseRow()
    releaseRow()
    await vi.advanceTimersByTimeAsync(DEMAND_SEND_INTERVAL_MS)
    expect(sent).toHaveLength(1)
    releaseSecond()
    releaseOther()
    await vi.advanceTimersByTimeAsync(DEMAND_SEND_INTERVAL_MS)
    expect(sent.at(-1)!.demand.visibleBranches).toEqual([])
  })

  it("goes out at most four times a second with the last change, and only when it differs", async () => {
    setDemandSubscription(1)
    await vi.runAllTimersAsync()
    expect(sent).toHaveLength(1)

    setDemandFocused(true)
    const release = showRobotDetails("robot-1")
    showRobotDetails("robot-2")
    await vi.advanceTimersByTimeAsync(DEMAND_SEND_INTERVAL_MS - 1)
    expect(sent).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(1)
    expect(sent).toHaveLength(2)
    expect(sent[1]!.demand).toMatchObject({ focused: true, robotDetails: ["robot-1", "robot-2"] })

    // a change and its undo within the interval: nothing differs, nothing goes
    release()
    showRobotDetails("robot-1")
    await vi.advanceTimersByTimeAsync(DEMAND_SEND_INTERVAL_MS)
    expect(sent).toHaveLength(2)
  })

  it("goes again to a new subscription, and to none without one", async () => {
    showBranch("ui")
    setDemandSubscription(1)
    await vi.runAllTimersAsync()
    setDemandSubscription(null)
    showBranch("api")
    await vi.runAllTimersAsync()
    expect(sent.map(it => it.subscription)).toEqual([1])

    setDemandSubscription(2)
    await vi.runAllTimersAsync()
    expect(sent.map(it => it.subscription)).toEqual([1, 2])
    expect(sent[1]!.demand.visibleBranches).toEqual(["api", "ui"])
  })
})
