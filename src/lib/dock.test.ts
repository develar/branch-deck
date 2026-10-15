import { beforeEach, describe, expect, it, vi } from "vitest"
import { resetDock, setBadgeSource } from "@/lib/dock"

const { dock } = vi.hoisted(() => ({ dock: { badges: [] as (number | undefined)[] } }))

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setBadgeCount: async (count: number | undefined) => {
      dock.badges.push(count)
    },
  }),
  ProgressBarStatus: {},
}))

vi.mock("@/lib/platform", () => ({ isWindows: false }))

beforeEach(() => {
  resetDock()
  dock.badges = []
})

describe("setBadgeSource", () => {
  it("shows the sum of its sources, set only on a change, none at 0", async () => {
    await setBadgeSource("attention", 2)
    await setBadgeSource("reviewRequests", 3)
    // the same sum again: no call
    await setBadgeSource("attention", 2)
    await setBadgeSource("reviewRequests", 0)
    await setBadgeSource("attention", 0)
    expect(dock.badges).toEqual([2, 5, 2, undefined])
  })
})
