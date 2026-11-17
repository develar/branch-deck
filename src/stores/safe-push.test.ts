// @spec openspec/specs/branch/next-step/spec.md
// Where a start shows (`startPlaceOf`): its popover and its progress, at the slot of a row or at the header of an island;
// every press of a start, the popover of a fix of what it needs too, opens there
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { startPlaceOf } from "@/stores/safe-push"
import { placed, testBranch } from "@/stores/test-branch"

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

/** The built stack branch of `root` with `members`, the root first */
function stackBranch(root: string, members: readonly string[]): StackBranch {
  return {
    root,
    branchName: `user/stack/${root}`,
    members,
    commitCount: members.length,
    state: "Built",
    tip: `${root}-stack-tip`,
    blockedBy: null,
    conflict: null,
    error: null,
    remoteStatus: null,
  }
}

describe("where a start shows", () => {
  beforeEach(() => {
    // `[name, parent]`, newest first: f1 forks into an island of five, auth into a small stack; a chain of five on a1 is
    // an island, w1 and w2 a small chain; docs stands alone
    const specs: [string, string | null][] = [
      ["docs", null],
      ["f5", "f4"],
      ["f4", "f3"],
      ["f3", "f1"],
      ["f2", "f1"],
      ["f1", null],
      ["a5", "a4"],
      ["a4", "a3"],
      ["a3", "a2"],
      ["a2", "a1"],
      ["a1", null],
      ["w2", "w1"],
      ["w1", null],
      ["session", "auth"],
      ["login", "auth"],
      ["auth", null],
    ]
    const branches: Record<string, Branch> = Object.fromEntries(specs.map(([name, parent]) => [name, testBranch(name, parent)]))
    const branchOrder = specs.map(([name]) => name)
    const stackBranches = { f1: stackBranch("f1", ["f1", "f2", "f3", "f4", "f5"]), auth: stackBranch("auth", ["auth", "login", "session"]) }
    branchSyncStore.setState({ ...placed({ branches, branchOrder, stackBranches }), branchOrder, stackBranches })
  })

  it("puts a branch at its row, or where its press asked it", () => {
    expect(startPlaceOf("login")).toEqual({ subject: "login", row: "login", placement: "row" })
    expect(startPlaceOf("a5", "stack-header")).toEqual({ subject: "a5", row: "a5", placement: "stack-header" })
  })

  it("puts the whole stack that forks at the header of its island, of a small stack at the root's row", () => {
    expect(startPlaceOf(stackReadinessKey("f1"))).toEqual({ subject: stackReadinessKey("f1"), row: "f1", placement: "stack-header" })
    expect(startPlaceOf(stackReadinessKey("auth"))).toEqual({ subject: stackReadinessKey("auth"), row: "auth", placement: "row" })
  })

  it("starts the whole stack of a straight chain as the Safe Push of its last branch, at the header of its island", () => {
    expect(startPlaceOf(stackReadinessKey("a1"))).toEqual({ subject: "a5", row: "a5", placement: "stack-header" })
    expect(startPlaceOf(stackReadinessKey("w1"))).toEqual({ subject: "w2", row: "w2", placement: "row" })
    // a root that heads no stack has no whole stack to start
    expect(startPlaceOf(stackReadinessKey("docs"))).toBeNull()
  })
})
