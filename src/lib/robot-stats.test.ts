// @spec spec/repository/main-views.spec.md
import { describe, expect, it } from "vite-plus/test"
import { STATS_PERIODS, formatStatsPeriod, minuteTicks } from "@/lib/robot-stats"

describe("formatStatsPeriod", () => {
  it("names every period by its days, the shortest first", () => {
    expect(STATS_PERIODS.map(formatStatsPeriod)).toEqual(["14 days", "30 days", "10 weeks", "6 months"])
  })
})

describe("minuteTicks", () => {
  it("steps by whole quarters, halves or hours to at most about five ticks over the largest value", () => {
    expect(minuteTicks(50)).toEqual([0, 15, 30, 45, 60])
    expect(minuteTicks(140)).toEqual([0, 30, 60, 90, 120, 150])
    expect(minuteTicks(160)).toEqual([0, 60, 120, 180])
    expect(minuteTicks(0)).toEqual([0])
  })
})
