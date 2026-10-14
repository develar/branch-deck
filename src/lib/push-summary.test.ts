import { describe, expect, it } from "vitest"
import type { OutgoingBranch } from "@/lib/push-summary"
import { formatOutgoingCount, formatOutgoingTitle, formatPushSummaryTitle, getOutgoingBarSegments, getOutgoingLegend, getPushAction, getPushColor, MAX_BAR_SEGMENTS, summarizePushState } from "@/lib/push-summary"
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

describe("getPushColor", () => {
  it("takes the hue from the push action: primary for a safe push, warning for a force push", () => {
    expect(getPushColor(branch("a", { remote: null }))).toBeNull()
    expect(getPushColor(branch("a"))).toBe("neutral")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1 } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { exists: false, commitsAhead: 2 } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsBehind: 1 } }))).toBe("warning")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 1, commitsBehind: 1 } }))).toBe("warning")
    // the status text says "1 (2)↑", but the rewritten branch needs a force push
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1 }, status: "Updated" }))).toBe("warning")
  })

  it("is neutral for a plain push of no commit of the user", () => {
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2 } }))).toBe("neutral")
    // a force push still overwrites remote commits
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2 }, status: "Updated" }))).toBe("warning")
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
    // the indicator stays for an open pull request
    expect(formatOutgoingTitle(summary, 2)).toBe("Nothing to push, 2 pull requests open")
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
      branch("a", { remote: { commitsAhead: 4, myCommitsAhead: 1 } }),
      branch("b", { remote: { commitsBehind: 1 } }),
    ])
    expect(getOutgoingBarSegments(summary)).toEqual([
      { key: "a", state: "push", weight: 4, mine: 1 },
      // solid: there are no commits of others to show light
      { key: "b", state: "force", weight: 1, mine: 1 },
    ])
  })

  it("has a segment per state for many branches", () => {
    const branches = Array.from({ length: MAX_BAR_SEGMENTS }, (_, index) => branch(`push${index}`, { remote: { commitsAhead: 2, myCommitsAhead: 1 } }))
    branches.push(branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 3 } }))
    expect(getOutgoingBarSegments(summarizePushState(branches))).toEqual([
      { key: "push", state: "push", weight: MAX_BAR_SEGMENTS * 2, mine: MAX_BAR_SEGMENTS },
      { key: "create", state: "create", weight: 3, mine: 3 },
    ])
  })
})

describe("getOutgoingLegend", () => {
  it("has a swatch per colour present", () => {
    const create = branch("new", { remote: { exists: false, commitsAhead: 3 } })
    const push = branch("ahead", { remote: { commitsAhead: 1 } })
    const force = branch("diverged", { remote: { commitsBehind: 1 } })
    expect(getOutgoingLegend(summarizePushState([create]))).toEqual([{ state: "create", label: "push" }])
    expect(getOutgoingLegend(summarizePushState([create, push, force]))).toEqual([
      { state: "push", label: "push" },
      { state: "force", label: "force push" },
    ])
    expect(getOutgoingLegend(summarizePushState([force]))).toEqual([{ state: "force", label: "force push" }])
  })
})
