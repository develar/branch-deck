import { describe, expect, it } from "vite-plus/test"
import { formatCheckCounts, formatFailedChecks } from "@/lib/check-counts"

describe("formatCheckCounts", () => {
  it("lists the failing checks first, then the running, the canceled and the passed ones", () => {
    expect(formatCheckCounts({ failed: 4, failedNames: ["a", "b", "c"], running: 1, canceled: 2, passed: 9 })).toBe("4 failing (a, b, c, …), 1 running, 2 canceled, 9 passed")
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 4 })).toBe("All 4 passed")
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 1 })).toBe("Passed")
  })
})

describe("formatFailedChecks", () => {
  it("names the checks that failed, counts the ones beyond the first three, nothing while none failed", () => {
    expect(formatFailedChecks({ failed: 1, failedNames: ["Bazel Community Build Aggregator"] })).toBe("Failed: Bazel Community Build Aggregator")
    expect(formatFailedChecks({ failed: 2, failedNames: ["a", "b"] })).toBe("Failed: a and b")
    expect(formatFailedChecks({ failed: 5, failedNames: ["a", "b", "c"] })).toBe("Failed: a, b, c and 2 more")
    expect(formatFailedChecks({ failed: 2, failedNames: [] })).toBe("Failed: 2 checks")
    expect(formatFailedChecks({ failed: 0, failedNames: [] })).toBeNull()
  })
})
