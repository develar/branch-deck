import { describe, expect, it } from "vite-plus/test"
import { formatCheckCounts, formatFailedChecks } from "@/lib/check-counts"

describe("formatCheckCounts", () => {
  it("lists the failing checks first, then the running, the canceled and the passed ones", () => {
    expect(formatCheckCounts({ failed: 5, failedNames: ["a", "b", "c"], running: 1, canceled: 2, passed: 9 })).toBe(
      "5 failing (a, b, c and 2 more), 1 running, 2 canceled, 9 passed",
    )
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 4 })).toBe("All 4 passed")
    expect(formatCheckCounts({ failed: 0, failedNames: [], running: 0, passed: 1 })).toBe("Passed")
  })
})

describe("formatFailedChecks", () => {
  it("names the checks that failed, counts the ones beyond the first three, nothing while none failed", () => {
    expect(formatFailedChecks({ failed: 1, failedNames: ["Bazel Community Build Aggregator"] })).toBe("Failed: Bazel Community Build Aggregator")
    expect(formatFailedChecks({ failed: 2, failedNames: ["a", "b"] })).toBe("Failed: a and b")
    expect(formatFailedChecks({ failed: 5, failedNames: ["a", "b", "c"] })).toBe("Failed: a, b, c and 2 more")
    // never "and 1 more": the backend names four of four
    expect(formatFailedChecks({ failed: 4, failedNames: ["a", "b", "c", "d"] })).toBe("Failed: a, b, c and d")
    expect(formatFailedChecks({ failed: 5, failedNames: ["a", "b", "c", "d"] })).toBe("Failed: a, b, c and 2 more")
    expect(formatFailedChecks({ failed: 2, failedNames: [] })).toBe("Failed: 2 checks")
    expect(formatFailedChecks({ failed: 0, failedNames: [] })).toBeNull()
  })

  it("never counts one failed check left out: one without a name is said as such, in the counts of the checks too", () => {
    // a GitHub check without a name: the only one the backend leaves out alone
    expect(formatFailedChecks({ failed: 2, failedNames: ["tests"] })).toBe("Failed: tests and a check without a name")
    expect(formatFailedChecks({ failed: 4, failedNames: ["a", "b", "c"] })).toBe("Failed: a, b, c and a check without a name")
    expect(formatCheckCounts({ failed: 2, failedNames: ["tests"], running: 0, passed: 3 })).toBe("2 failing (tests and a check without a name), 3 passed")
    // two or more left out are counted, named or not
    expect(formatFailedChecks({ failed: 3, failedNames: ["tests"] })).toBe("Failed: tests and 2 more")
    expect(formatCheckCounts({ failed: 4, failedNames: ["a", "b", "c", "d"], running: 0, passed: 0 })).toBe("4 failing (a, b, c and d)")
  })
})
