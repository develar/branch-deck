import { describe, expect, it } from "vitest"
import type { BranchReviewSegment } from "@/lib/bindings"
import { branchReviewBlocker, keepSegmentRange, segmentRangeSubject, selectSegment } from "@/lib/branch-review"
import { testBranch } from "@/stores/test-branch"

function segment(name: string, base: string, head: string): BranchReviewSegment {
  return { name, base, head, commits: [{ id: head, subject: name, authorTime: 1 }] }
}

const segments = [segment("auth", "b0", "a1"), segment("login", "a1", "l1"), segment("ui", "l1", "u1")]

describe("segmentRangeSubject", () => {
  it("diffs the base of the first segment to the head of the last one", () => {
    expect(segmentRangeSubject(segments, { from: 2, to: 2 })).toEqual({ kind: "range", base: "l1", head: "u1" })
    expect(segmentRangeSubject(segments, { from: 0, to: 2 })).toEqual({ kind: "range", base: "b0", head: "u1" })
  })
})

describe("selectSegment", () => {
  it("selects one segment, or extends the range to the clicked one with Shift", () => {
    expect(selectSegment({ from: 2, to: 2 }, 1, false)).toEqual({ from: 1, to: 1 })
    expect(selectSegment({ from: 2, to: 2 }, 0, true)).toEqual({ from: 0, to: 2 })
    expect(selectSegment({ from: 0, to: 1 }, 2, true)).toEqual({ from: 0, to: 2 })
  })
})

describe("keepSegmentRange", () => {
  it("keeps the range by the names of its ends, or falls back to the reviewed branch", () => {
    const moved = [segment("auth", "b0", "a2"), segment("login", "a2", "l2"), segment("ui", "l2", "u2")]
    expect(keepSegmentRange(segments, moved, { from: 1, to: 2 })).toEqual({ from: 1, to: 2 })
    // the parent landed: the chain lost it
    expect(keepSegmentRange(segments, moved.slice(1), { from: 0, to: 1 })).toEqual({ from: 1, to: 1 })
  })
})

describe("branchReviewBlocker", () => {
  it("allows a synced branch with commits", () => {
    expect(branchReviewBlocker(testBranch("ui", null))).toBeNull()
    expect(branchReviewBlocker(testBranch("ui", null, { hasError: true }))).toBe("The branch failed to sync")
  })
})
