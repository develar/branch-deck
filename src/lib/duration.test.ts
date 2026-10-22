import { describe, expect, it } from "vite-plus/test"
import { formatTotalDuration } from "@/lib/duration"

describe("formatTotalDuration", () => {
  it("names minutes, hours below two days, then days and hours", () => {
    expect(formatTotalDuration(0)).toBe("0 min")
    expect(formatTotalDuration(20)).toBe("1 min")
    expect(formatTotalDuration(45 * 60)).toBe("45 min")
    expect(formatTotalDuration(36 * 3600)).toBe("36 h")
    expect(formatTotalDuration(3 * 86_400 + 4 * 3600)).toBe("3 d 4 h")
    expect(formatTotalDuration(3 * 86_400)).toBe("3 d")
  })
})
