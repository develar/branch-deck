import { describe, expect, it } from "vitest"
import type { OutgoingBranch } from "@/lib/push-summary"
import { formatOutgoingCount, formatPushSummaryTitle, getOutgoingBarSegments, getPushAction, MAX_BAR_SEGMENTS, summarizePushState } from "@/lib/push-summary"
import type { BranchStatus, RemoteStatus } from "@/stores/branch-sync"

interface BranchOptions {
  remote?: Partial<RemoteStatus> | null
  status?: BranchStatus
  hasError?: boolean
  commitCount?: number
}

function branch(name: string, { remote = {}, status = "Unchanged", hasError = false, commitCount = 3 }: BranchOptions = {}) {
  const remoteStatus: RemoteStatus | null = remote === null
    ? null
    : { exists: true, unpushedCommits: [], commitsAhead: 0, commitsBehind: 0, myCommitsAhead: 0, lastPushTime: 0, ...remote }
  return { name, remoteStatus, status, hasError, commitCount }
}

describe("getPushAction", () => {
  it("follows the rules of the push button label", () => {
    expect(getPushAction(branch("a", { remote: null }))).toBeNull()
    expect(getPushAction(branch("a", { remote: { exists: false, commitsAhead: 2 } }))).toBe("create")
    expect(getPushAction(branch("a"))).toBe("upToDate")
    // a rewritten branch whose remote matches is up to date: nothing to force
    expect(getPushAction(branch("a", { status: "Updated" }))).toBe("upToDate")
    expect(getPushAction(branch("a", { remote: { commitsAhead: 2 } }))).toBe("push")
    expect(getPushAction(branch("a", { remote: { commitsAhead: 2 }, status: "Updated" }))).toBe("force")
    expect(getPushAction(branch("a", { remote: { commitsBehind: 1 } }))).toBe("force")
  })
})

describe("summarizePushState", () => {
  it("sums the outgoing branches in order and counts them by state", () => {
    const summary = summarizePushState([
      branch("pushed"),
      branch("new", { remote: { exists: false, myCommitsAhead: 3 } }),
      branch("ahead", { remote: { commitsAhead: 5, myCommitsAhead: 2 } }),
      branch("diverged", { remote: { commitsAhead: 1, commitsBehind: 2, myCommitsAhead: 1 } }),
    ])
    expect(summary.total).toBe(9)
    expect(summary.mine).toBe(6)
    expect(summary.branches).toEqual([
      { name: "new", count: 3, mine: 3, state: "create", behind: 0 },
      { name: "ahead", count: 5, mine: 2, state: "push", behind: 0 },
      { name: "diverged", count: 1, mine: 1, state: "force", behind: 2 },
    ])
    expect(summary.byState).toEqual({ push: 1, create: 1, force: 1 })
    expect(summary.notLoaded).toBe(0)
    expect(formatPushSummaryTitle(summary)).toBe("9 commits to push in 3 branches")
  })

  it("counts every commit of a branch not on the remote", () => {
    // the sync reports no commits ahead for a branch not on the remote, and all its commits as the user's
    const summary = summarizePushState([branch("new", { remote: { exists: false, myCommitsAhead: 3 }, commitCount: 3 })])
    expect(summary.branches).toEqual([{ name: "new", count: 3, mine: 3, state: "create", behind: 0 }])
    expect(summary.total).toBe(3)
  })

  it("skips the branches whose row offers no push and counts the ones not loaded", () => {
    const summary = summarizePushState([
      branch("conflict", { remote: { commitsAhead: 2 }, hasError: true }),
      branch("empty", { remote: { exists: false }, commitCount: 0 }),
      branch("loading", { remote: null }),
      branch("ahead", { remote: { commitsAhead: 1 } }),
    ])
    expect(summary.branches.map(it => it.name)).toEqual(["ahead"])
    expect(summary.notLoaded).toBe(1)
    expect(formatPushSummaryTitle(summary)).toBe("1 commit to push in 1 branch")
  })

  it("is empty when everything is pushed", () => {
    const summary = summarizePushState([branch("a"), branch("b", { remote: null, hasError: true })])
    expect(summary.branches).toEqual([])
    expect(summary.total).toBe(0)
    expect(summary.notLoaded).toBe(0)
  })
})

describe("formatOutgoingCount", () => {
  it("reads as the status text of a branch row", () => {
    const outgoing: OutgoingBranch = { name: "a", count: 5, mine: 3, state: "push", behind: 0 }
    expect(formatOutgoingCount(outgoing)).toBe("3 (5)↑")
    expect(formatOutgoingCount({ ...outgoing, state: "force", behind: 2 })).toBe("3 (5)↑ · 2↓")
  })
})

describe("getOutgoingBarSegments", () => {
  it("has a segment per branch, a force push without commits still visible", () => {
    const summary = summarizePushState([
      branch("a", { remote: { commitsAhead: 4 } }),
      branch("b", { remote: { commitsBehind: 1 } }),
    ])
    expect(getOutgoingBarSegments(summary)).toEqual([
      { key: "a", state: "push", weight: 4 },
      { key: "b", state: "force", weight: 1 },
    ])
  })

  it("has a segment per state for many branches", () => {
    const branches = Array.from({ length: MAX_BAR_SEGMENTS }, (_, index) => branch(`push${index}`, { remote: { commitsAhead: 2 } }))
    branches.push(branch("new", { remote: { exists: false, commitsAhead: 3 } }))
    expect(getOutgoingBarSegments(summarizePushState(branches))).toEqual([
      { key: "push", state: "push", weight: MAX_BAR_SEGMENTS * 2 },
      { key: "create", state: "create", weight: 3 },
    ])
  })
})
