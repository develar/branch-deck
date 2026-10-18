import { describe, expect, it } from "vite-plus/test"
import { formatCompactTimestamp } from "@/lib/time"

/** Unix seconds of a local date and time */
function at(year: number, month: number, day: number, hours = 12): number {
  return new Date(year, month - 1, day, hours).getTime() / 1000
}

describe("formatCompactTimestamp", () => {
  const now = new Date(2026, 9, 18, 15)

  it("says the time of today, only the day of yesterday", () => {
    expect(formatCompactTimestamp(at(2026, 10, 18, 9), now)).toMatch(/^Today \d/)
    expect(formatCompactTimestamp(at(2026, 10, 17, 23), now)).toBe("Yesterday")
  })

  it("leaves the year out within the year of now", () => {
    expect(formatCompactTimestamp(at(2026, 10, 3), now)).not.toContain("2026")
    expect(formatCompactTimestamp(at(2025, 10, 3), now)).toContain("2025")
  })
})
