import { describe, expect, it } from "vite-plus/test"
import type { PullRequest } from "@/lib/bindings"
import {
  describeOrphanedStackPullRequest,
  describeStackBranchAlong,
  describeStackBranchPush,
  describeWholeStackApprovalsForPiece,
  describeWholeStackLanding,
  formatReadinessSubject,
  formatStackBranchPushLine,
  formatWholeStack,
  formatWholeStackApprovals,
  formatWholeStackBranch,
  formatWholeStackStart,
  stackReadinessKey,
  stackRootOfReadinessKey,
} from "@/lib/whole-stack"
import type { StackBranch } from "@/stores/branch-sync"
import { CREATE, PUSH, testRemoteStatus } from "@/stores/test-branch"

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

  it("lands its branches in one robot, the side branches named: they are what only it lands", () => {
    const members = ["air", "bazel", "bridge", "runtime", "deploy", "side-fix", "side-test"]
    expect(describeWholeStackLanding(members, 14, "master", "rebaseAutosquash")).toBe(
      "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test",
    )
    expect(describeWholeStackLanding(members, 14, "master", "dryRun")).toBe("Checks 7 branches · 14 commits on master in one Dry Run: air, bazel, … side-fix and side-test")
    expect(describeWholeStackLanding(["auth", "login", "session"], 3, "master", "rebaseAutosquash")).toBe(
      "Lands 3 branches · 3 commits into master in one Safe Push: auth, login and session",
    )
  })

  it("has the readiness of its stack branch under a key no branch has, and its approvals only in its own pull request", () => {
    expect(stackReadinessKey("air")).toBe("stack:air")
    expect([stackRootOfReadinessKey("stack:air"), stackRootOfReadinessKey("air")]).toEqual(["air", null])
    expect([formatReadinessSubject("stack:air"), formatReadinessSubject("auth")]).toEqual(["the whole air stack", "auth"])
    expect(formatWholeStackApprovals(12)).toBe("Approvals count in #12, the pull request of the whole stack")
    expect(describeWholeStackApprovalsForPiece("air", 12, "deploy")).toBe(
      "#12, the pull request of the whole air stack, is open: its approvals count only for the Safe Push of the whole stack, not for this Safe Push of deploy",
    )
  })

  it("names the stack branch a push of its root takes along", () => {
    const ahead = testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["c1", "c2"], action: PUSH })
    const absent = testRemoteStatus({ exists: false, remoteTip: null, commitsAhead: 4, action: CREATE })
    expect(formatStackBranchPushLine(stack({ remoteStatus: ahead }))).toBe("user/stack/air · the whole stack branch · 2 commits")
    expect(formatStackBranchPushLine(stack({ remoteStatus: absent }))).toBe("user/stack/air · the whole stack branch · 4 commits · creates the remote branch")
    expect(describeStackBranchAlong(stack(), pullRequest)).toBe("Also pushes the whole stack branch user/stack/air, the head of #12")
    expect(describeStackBranchAlong(stack(), undefined)).toBe("Also pushes the whole stack branch user/stack/air")
    expect(formatWholeStackBranch("air")).toBe("the whole stack branch of air")
  })

  it("pushes only its stack branch for its Safe Push, and says that its branches stay only of those on the remote", () => {
    const members = ["air", "auth", "side"]
    expect(describeStackBranchPush("user/stack/air", members, [])).toBe("Pushes only its stack branch user/stack/air.")
    expect(describeStackBranchPush("user/stack/air", members, members)).toBe(
      "Pushes only its stack branch user/stack/air: the branches of the stack stay as they are on the remote.",
    )
    expect(describeStackBranchPush("user/stack/air", members, ["air"])).toBe("Pushes only its stack branch user/stack/air: air stays as it is on the remote.")
    expect(describeStackBranchPush("user/stack/air", members, ["air", "auth"])).toBe("Pushes only its stack branch user/stack/air: air and auth stay as they are on the remote.")
  })

  it("has a pull request that no longer matches it once a piece landed the root or the stack no longer forks, an open one only", () => {
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack(), null)).toBeNull()
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack(), "auth")).toBe("#12 no longer matches the air stack: auth landed")
    expect(describeOrphanedStackPullRequest("air", pullRequest, stack({ members: [], state: "Stale" }), null)).toBe("#12 no longer matches the air stack: it no longer forks")
    expect(describeOrphanedStackPullRequest("air", { ...pullRequest, state: "closed" }, stack({ members: [] }), "auth")).toBeNull()
    expect(describeOrphanedStackPullRequest("air", undefined, undefined, "auth")).toBeNull()
  })
})
