// @spec spec/repository/main-views.spec.md
import { describe, expect, it } from "vite-plus/test"
import { STATS_PERIODS, formatStatsPeriod } from "@/lib/robot-stats"

describe("formatStatsPeriod", () => {
  it("names every period by its days, the shortest first", () => {
    expect(STATS_PERIODS.map(formatStatsPeriod)).toEqual(["14 days", "30 days", "10 weeks", "6 months"])
  })
})
