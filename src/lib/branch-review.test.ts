import { describe, expect, it } from "vite-plus/test"
import type { BranchReviewSegment } from "@/lib/bindings"
import { branchReviewBlocker, keepSegmentRange, segmentRangeSubject, selectSegment, stackBarItems } from "@/lib/branch-review"
import { testBranch } from "@/stores/test-branch"

function segment(name: string, base: string, head: string): BranchReviewSegment {
  return { name, base, head, commits: [{ id: head, subject: name, message: name, author: "Test User", authorTime: 1 }] }
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

describe("stackBarItems", () => {
  it("folds the middle of a long stack, keeping the root, the ends of the range and the last two segments", () => {
    const shape = (items: ReturnType<typeof stackBarItems>) => items.map(it => it.kind === "segment" ? `${it.index}` : `+${it.count}`).join(" ")
    expect(shape(stackBarItems(18, { from: 17, to: 17 }, false))).toBe("0 +15 16 17")
    expect(shape(stackBarItems(18, { from: 5, to: 17 }, false))).toBe("0 +4 5 +10 16 17")
    // a run of one isn't folded
    expect(shape(stackBarItems(7, { from: 2, to: 6 }, false))).toBe("0 1 2 +2 5 6")
  })

  it("shows every segment of a short stack, or once unfolded", () => {
    expect(stackBarItems(6, { from: 5, to: 5 }, false)).toHaveLength(6)
    expect(stackBarItems(18, { from: 17, to: 17 }, true)).toHaveLength(18)
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
