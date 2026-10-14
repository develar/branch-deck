import { describe, expect, it } from "vitest"
import { formatCheckCounts } from "@/lib/check-counts"

describe("formatCheckCounts", () => {
  it("lists the failing checks first, then the running, the canceled and the passed ones", () => {
    expect(formatCheckCounts({ failed: 4, failedNames: ["a", "b", "c"], running: 1, canceled: 2, passed: 9 })).toBe("4 failing (a, b, c, …), 1 running, 2 canceled, 9 passed")
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 4 })).toBe("All 4 passed")
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 1 })).toBe("Passed")
  })
})
