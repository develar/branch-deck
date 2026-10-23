// @spec spec/repository/main-views.spec.md
import { describe, expect, it } from "vite-plus/test"
import { STATS_PERIODS, countTicks, formatStatsPeriod, minuteTicks } from "@/lib/robot-stats"

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

describe("countTicks", () => {
  it("steps by 1, 2 or 5 times a power of ten to at most about five ticks over the largest value", () => {
    expect(countTicks(4)).toEqual([0, 1, 2, 3, 4])
    expect(countTicks(12)).toEqual([0, 5, 10, 15])
    expect(countTicks(180)).toEqual([0, 50, 100, 150, 200])
    expect(countTicks(1000)).toEqual([0, 500, 1000])
    expect(countTicks(0)).toEqual([0])
  })
})
