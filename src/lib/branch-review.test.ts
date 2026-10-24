import { describe, expect, it } from "vite-plus/test"
import type { BranchReviewSegment } from "@/lib/bindings"
import { branchReviewBlocker, branchReviewWindowKey, keepSegmentRange, moveView, planReviewOpen, segmentRangeSubject, selectSegment, stackBarItems } from "@/lib/branch-review"
import type { BranchReviewFocus, ShownBranchReview } from "@/lib/branch-review"
import { testBranch } from "@/stores/test-branch"
import type { BranchReviewWindowData } from "@/types/sub-window"

function segment(name: string, base: string, head: string): BranchReviewSegment {
  return { name, base, head, commits: [{ id: head, subject: name, message: name, author: "Test User", authorTime: 1, committerTime: 1 }] }
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
    const shape = (items: ReturnType<typeof stackBarItems>) => items.map(it => (it.kind === "segment" ? `${it.index}` : `+${it.count}`)).join(" ")
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

describe("the review window of a branch", () => {
  const plain: BranchReviewWindowData = {
    repositoryPath: "/repo",
    baselineBranch: "origin/master",
    chain: ["auth", "login"],
    groups: [],
    focus: { range: "own", filter: { kind: "all" }, commitId: null, path: null },
    openedAt: 1,
    startedAt: 1,
    move: false,
    state: { repositoryPath: "/repo", selected: true, tips: {}, items: {}, menus: {}, createPullRequest: null, wholeStack: null, stackCommits: {}, commitMenus: {}, groups: null },
  }
  const shown: ShownBranchReview = { repositoryPath: "/repo", chain: ["auth", "login"], wholeStack: false, startedAt: 1 }
  const open = (branchName: string, chain: string[], focus: BranchReviewFocus) => planReviewOpen(shown, { repositoryPath: "/repo", branchName, chain, focus }, 9)

  it("keeps its review for a plain open of the branch it shows, and starts over for another branch or a place", () => {
    expect(open("login", ["auth", "login"], {})).toEqual({ chain: ["auth", "login"], startedAt: 1, move: false })
    expect(open("auth", ["auth"], {})).toEqual({ chain: ["auth"], startedAt: 9, move: false })
    // the "Review Changes" of the Safe Push popover: the whole stack, the files of a group, a file
    expect(open("login", ["auth", "login"], { range: "all", filter: { kind: "group", name: "Air" }, path: "a.kt" }).startedAt).toBe(9)
    expect(planReviewOpen(null, { repositoryPath: "/repo", branchName: "login", chain: ["auth", "login"], focus: {} }, 9).startedAt).toBe(9)
    expect(planReviewOpen({ ...shown, wholeStack: true }, { repositoryPath: "/repo", branchName: "login", chain: ["auth", "login"], focus: {} }, 9).startedAt).toBe(9)
    // the key follows the start: the window starts over only when it changes
    expect(branchReviewWindowKey({ ...plain, openedAt: 2 })).toBe(branchReviewWindowKey(plain))
    expect(branchReviewWindowKey({ ...plain, startedAt: 9 })).not.toBe(branchReviewWindowKey(plain))
  })

  it("moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch", () => {
    // a commit of the parent: the chain on screen stays
    expect(open("auth", ["auth"], { commitId: "a1" })).toEqual({ chain: ["auth", "login"], startedAt: 1, move: true })
    expect(open("login", ["auth", "login"], { commitId: "l1", path: "a.kt" })).toEqual({ chain: ["auth", "login"], startedAt: 1, move: true })
    expect(open("ui", ["auth", "login", "ui"], { commitId: "u1" })).toEqual({ chain: ["auth", "login", "ui"], startedAt: 9, move: false })
    expect(planReviewOpen(shown, { repositoryPath: "/other", branchName: "auth", chain: ["auth"], focus: { commitId: "a1" } }, 9).move).toBe(false)
  })
})

describe("moveView", () => {
  const view = { segments, range: { from: 1, to: 2 }, commitId: null }

  it("keeps the part that has the commit, else shows the commit's segment", () => {
    expect(moveView(view, { commitId: "l1", range: { from: "login", to: "login" } })).toEqual({ ...view, commitId: "l1" })
    expect(moveView(view, { commitId: "a1", range: { from: "auth", to: "auth" } })).toEqual({ ...view, range: { from: 0, to: 0 }, commitId: "a1" })
  })

  it("shows all changes of the commit's branch when the segments don't have the commit", () => {
    expect(moveView(view, { commitId: "gone", range: { from: "auth", to: "auth" } })).toEqual({ ...view, range: { from: 0, to: 0 }, commitId: null })
  })
})
