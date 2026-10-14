import { describe, expect, it } from "vitest"
import type { OutgoingBranch } from "@/lib/push-summary"
import { formatOutgoingCount, formatOutgoingTitle, formatPushSummaryTitle, getOutgoingBarSegments, getOutgoingLegend, getPushAction, getPushColor, MAX_BAR_SEGMENTS, summarizePushState } from "@/lib/push-summary"
import type { RemoteStatus } from "@/stores/branch-sync"

interface BranchOptions {
  remote?: Partial<RemoteStatus> | null
  hasError?: boolean
  commitCount?: number
}

function branch(name: string, { remote = {}, hasError = false, commitCount = 3 }: BranchOptions = {}) {
  const remoteStatus: RemoteStatus | null = remote === null
    ? null
    : { exists: true, unpushedCommits: [], commitsAhead: 0, commitsBehind: 0, myCommitsAhead: 0, fastForward: true, lastPushTime: 0, remoteTip: null, ...remote }
  return { name, remoteStatus, hasError, commitCount }
}

describe("getPushAction", () => {
  it("follows the rules of the push button label", () => {
    expect(getPushAction(branch("a", { remote: null }))).toBeNull()
    expect(getPushAction(branch("a", { remote: { exists: false, commitsAhead: 2 } }))).toBe("create")
    expect(getPushAction(branch("a"))).toBe("upToDate")
    expect(getPushAction(branch("a", { remote: { commitsAhead: 2 } }))).toBe("push")
    // a rebased branch with new commits: not a fast-forward of its remote
    expect(getPushAction(branch("a", { remote: { commitsAhead: 2, fastForward: false } }))).toBe("force")
    expect(getPushAction(branch("a", { remote: { commitsBehind: 1, fastForward: false } }))).toBe("force")
  })

  it("is up to date for a branch only rebased onto a newer baseline: the remote has its work", () => {
    expect(getPushAction(branch("a", { remote: { fastForward: false } }))).toBe("upToDate")
    expect(summarizePushState([branch("a", { remote: { fastForward: false } })]).branches).toEqual([])
  })
})

describe("getPushColor", () => {
  it("takes the hue from the risk: warning only for a push that drops remote commits", () => {
    expect(getPushColor(branch("a", { remote: null }))).toBeNull()
    expect(getPushColor(branch("a"))).toBe("neutral")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1 } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { exists: false, commitsAhead: 2 } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsBehind: 1, fastForward: false } }))).toBe("warning")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 1, commitsBehind: 1, fastForward: false } }))).toBe("warning")
    // a force push of a rebased branch only replaces the remote commits with their copies
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1, fastForward: false } }))).toBe("primary")
  })

  it("is neutral for a safe push of no commit of the user", () => {
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2 } }))).toBe("neutral")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, fastForward: false } }))).toBe("neutral")
    // a first push keeps its hue whoever authored the commits, a push that drops remote commits too
    expect(getPushColor(branch("a", { remote: { exists: false, commitsAhead: 2 } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, commitsBehind: 1, fastForward: false } }))).toBe("warning")
  })
})

describe("summarizePushState", () => {
  it("sums the outgoing branches in order and counts them by state", () => {
    const summary = summarizePushState([
      branch("pushed"),
      branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 3 } }),
      branch("ahead", { remote: { commitsAhead: 5, myCommitsAhead: 2 } }),
      branch("diverged", { remote: { commitsAhead: 1, commitsBehind: 2, myCommitsAhead: 1, fastForward: false } }),
    ])
    expect(summary.total).toBe(9)
    expect(summary.mine).toBe(6)
    expect(summary.branches).toEqual([
      { name: "new", count: 3, mine: 3, state: "create", behind: 0 },
      { name: "ahead", count: 5, mine: 2, state: "push", behind: 0 },
      { name: "diverged", count: 1, mine: 1, state: "force", behind: 2 },
    ])
    expect(summary.byState).toEqual({ push: 1, create: 1, force: 1 })
    expect(summary.byHue).toEqual({ primary: 2, warning: 1 })
    expect(summary.notLoaded).toBe(0)
    expect(formatPushSummaryTitle(summary)).toBe("9 commits to push in 3 branches")
  })

  it("counts the commits whose content the remote lacks", () => {
    // a branch not on the remote sends all its commits, by any author
    const created = branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 1 }, commitCount: 3 })
    // the sync reports neither the baseline commits nor the rebased copies of pushed commits as ahead
    const rebased = branch("rebased", { remote: { commitsAhead: 1, myCommitsAhead: 1, fastForward: false }, commitCount: 4 })
    const summary = summarizePushState([created, rebased])
    expect(summary.branches).toEqual([
      { name: "new", count: 3, mine: 1, state: "create", behind: 0 },
      { name: "rebased", count: 1, mine: 1, state: "force", behind: 0 },
    ])
    expect(summary.total).toBe(4)
    expect(summary.byHue).toEqual({ primary: 2, warning: 0 })
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
      branch("b", { remote: { commitsBehind: 1, fastForward: false } }),
    ])
    expect(getOutgoingBarSegments(summary)).toEqual([
      { key: "a", hue: "primary", weight: 4, mine: 1 },
      // solid: there are no commits of others to show light
      { key: "b", hue: "warning", weight: 1, mine: 1 },
    ])
  })

  it("has a segment per hue for many branches", () => {
    const branches = Array.from({ length: MAX_BAR_SEGMENTS }, (_, index) => branch(`push${index}`, { remote: { commitsAhead: 2, myCommitsAhead: 1 } }))
    branches.push(branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 3 } }))
    branches.push(branch("diverged", { remote: { commitsAhead: 1, commitsBehind: 1, myCommitsAhead: 1, fastForward: false } }))
    expect(getOutgoingBarSegments(summarizePushState(branches))).toEqual([
      { key: "primary", hue: "primary", weight: MAX_BAR_SEGMENTS * 2 + 3, mine: MAX_BAR_SEGMENTS + 3 },
      { key: "warning", hue: "warning", weight: 1, mine: 1 },
    ])
  })
})

describe("getOutgoingLegend", () => {
  it("has a swatch per colour present", () => {
    const create = branch("new", { remote: { exists: false, commitsAhead: 3 } })
    const push = branch("ahead", { remote: { commitsAhead: 1 } })
    const rebased = branch("rebased", { remote: { commitsAhead: 1, fastForward: false } })
    const diverged = branch("diverged", { remote: { commitsBehind: 1, fastForward: false } })
    expect(getOutgoingLegend(summarizePushState([create]))).toEqual([{ hue: "primary", label: "push" }])
    // a force push of a rebased branch drops nothing: it has the colour of a push
    expect(getOutgoingLegend(summarizePushState([rebased]))).toEqual([{ hue: "primary", label: "push" }])
    expect(getOutgoingLegend(summarizePushState([create, push, diverged]))).toEqual([
      { hue: "primary", label: "push" },
      { hue: "warning", label: "drops remote commits" },
    ])
    expect(getOutgoingLegend(summarizePushState([diverged]))).toEqual([{ hue: "warning", label: "drops remote commits" }])
  })
})
