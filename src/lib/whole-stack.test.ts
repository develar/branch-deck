import { describe, expect, it } from "vite-plus/test"
import type { PullRequest } from "@/lib/bindings"
import { describeOrphanedStackPullRequest, formatWholeStack, formatWholeStackStart, getStackBranchBlocker } from "@/lib/whole-stack"
import type { StackBranch } from "@/stores/branch-sync"

function stack(patch: Partial<StackBranch> = {}): StackBranch {
  return {
    root: "air",
    branchName: "user/stack/air",
    members: ["air", "auth", "side"],
    commitCount: 4,
    state: "Built",
    tip: "stack-tip",
    blockedBy: null,
    conflict: null,
    error: null,
    remoteStatus: null,
    ...patch,
  }
}

const pullRequest = { number: 12, state: "open" } as PullRequest

describe("the whole stack", () => {
  it("is named as bd names it", () => {
    expect([formatWholeStack("air"), formatWholeStackStart("air")]).toEqual(["the whole air stack", "Whole air stack"])
  })

  it("is pushed and landed only while the sync built its stack branch, else says why", () => {
    expect(getStackBranchBlocker("air", stack())).toBeNull()
    expect(getStackBranchBlocker("air", undefined)).toBe("The air stack has no stack branch: only a stack that forks builds one")
    expect(getStackBranchBlocker("air", stack({ state: "Blocked", blockedBy: { member: "auth", reason: "Conflict" } }))).toBe("Blocked by auth: fix it first, then sync")
    expect(getStackBranchBlocker("air", stack({ state: "Conflict", conflict: { commit: "0123456789abcdef", files: ["a.kt", "b.kt"] } }))).toBe(
      "The stack branch conflicts at 01234567 in a.kt and b.kt: the stack lands in pieces",
    )
    expect(getStackBranchBlocker("air", stack({ state: "Stale", error: "Not rebuilt: another process moved the ref" }))).toBe("Not rebuilt: another process moved the ref")
  })

  it("has a pull request that no longer matches it once a piece landed the root or the stack no longer forks, an open one only", () => {
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack(), null)).toBeNull()
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack(), "auth")).toBe("#12 no longer matches the air stack: auth landed")
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack({ members: [], state: "Stale" }), null)).toBe("#12 no longer matches the air stack: it no longer forks")
    expect(describeOrphanedStackPullRequest("air", { ...pullRequest, state: "closed" }, stack({ members: [] }), "auth")).toBeNull()
    expect(describeOrphanedStackPullRequest("air", undefined, undefined, "auth")).toBeNull()
  })
})
