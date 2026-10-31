// @spec spec/repository/safe-push-statistics.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { GetRobotStatsParams, RobotStats, RobotStatsEvent, StatsPeriod } from "@/lib/bindings"
import { readRobotStats, robotStatsStore } from "@/stores/robot-stats"

const getRobotStats = vi.hoisted(() => vi.fn())

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", () => ({ commands: { getRobotStats } }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

type Events = { onmessage: (event: RobotStatsEvent) => void }

/** Holds the next `get_robot_stats`: the test sends its events through `events()` and ends it with `end(period)` */
function holdNextRead(): { events: () => Events; end: (period: StatsPeriod) => void } {
  let channel: Events | undefined
  let resolveCall = (_period: StatsPeriod) => {}
  getRobotStats.mockImplementationOnce((_params: GetRobotStatsParams, events: Events) => {
    channel = events
    return new Promise(resolve => {
      resolveCall = period => resolve({ status: "ok", data: { availability: { status: "available" }, stale: false, fetched_at: 1, stats: statsOf(period) } })
    })
  })
  return { events: () => channel!, end: period => resolveCall(period) }
}

function statsOf(period: StatsPeriod): RobotStats {
  return { window: { period } } as RobotStats
}

const entry = () => robotStatsStore.getState().byRepository["/repo"]

beforeEach(() => {
  robotStatsStore.setState({ byRepository: {} })
  getRobotStats.mockReset()
})

describe("robot stats", () => {
  it("keeps the numbers of the lists of a read while the read of another period starts, until it has its own", async () => {
    const first = holdNextRead()
    readRobotStats({ repositoryPath: "/repo", baselineBranch: "master", period: "30d" }, false)
    first.events().onmessage({ type: "partial", data: statsOf("30d") })
    expect(entry()?.partial?.window.period).toBe("30d")

    const second = holdNextRead()
    readRobotStats({ repositoryPath: "/repo", baselineBranch: "master", period: "14d" }, false)
    expect(entry()?.partial?.window.period, "the numbers stay while the next read runs").toBe("30d")
    expect(entry()?.loading).toBe(true)

    second.events().onmessage({ type: "partial", data: statsOf("14d") })
    expect(entry()?.partial?.window.period).toBe("14d")
    second.end("14d")
    await vi.waitFor(() => expect(entry()?.loading).toBe(false))
    expect(entry()?.partial).toBeNull()
    expect(entry()?.result?.stats?.window.period).toBe("14d")
  })
})
