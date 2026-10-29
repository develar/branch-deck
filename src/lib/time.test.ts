import { describe, expect, it } from "vite-plus/test"
import { formatCompactTimestamp, formatRelativeTime, readsLater } from "@/lib/time"

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

describe("readsLater", () => {
  const synced = Date.parse("2026-10-29T12:00:00Z")
  const minute = 60_000

  it('says a check in the minute of the sync nothing "just now" does not: the check of the start, 100 ms after its sync', () => {
    expect(readsLater(synced + 100, synced)).toBe(false)
    expect(readsLater(synced + minute - 1, synced)).toBe(false)
    expect(readsLater(synced + minute, synced)).toBe(true)
    expect(readsLater(synced + 18 * minute, synced)).toBe(true)
  })

  it("keeps its answer while both age, where their texts read the same now and apart a bucket later", () => {
    const checked = synced + 30 * minute
    // an hour and a half after the sync both are "1 hour ago", half an hour later "2 hours ago" and "1 hour ago"
    expect(formatRelativeTime(checked, synced + 90 * minute)).toBe(formatRelativeTime(synced, synced + 90 * minute))
    expect(formatRelativeTime(checked, synced + 120 * minute)).not.toBe(formatRelativeTime(synced, synced + 120 * minute))
    expect(readsLater(checked, synced)).toBe(true)
  })
})
