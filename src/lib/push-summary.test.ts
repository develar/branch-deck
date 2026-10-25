import { describe, expect, it } from "vite-plus/test"
import { describePush, describeStackPush, formatPushCount, getPushColor, getPushHue, summarizePushState } from "@/lib/push-summary"
import type { RemoteStatus } from "@/stores/branch-sync"
import { CREATE, forcePush, PUSH, testRemoteStatus } from "@/stores/test-branch"

/** A force push that only replaces the rebased copies, one that replaces older versions, one that drops remote commits */
const REBASED = forcePush()
const REPLACES = forcePush({ replaces: true })
const DROPS = forcePush({ drops: true })

describe("describePush", () => {
  it("says what the push did from the status it was made against", () => {
    expect(describePush("auth", null)).toEqual({ title: "Pushed auth" })
    expect(describePush("auth", testRemoteStatus({ exists: false, commitsAhead: 3, action: CREATE }))).toEqual({
      title: "Pushed auth",
      description: "3 commits · created the remote branch",
    })
    expect(describePush("auth", testRemoteStatus({ commitsAhead: 1, action: PUSH }))).toEqual({ title: "Pushed auth", description: "1 commit" })
    expect(describePush("auth", testRemoteStatus({ commitsAhead: 2, fastForward: false, action: REBASED }))).toEqual({
      title: "Force pushed auth",
      description: "2 commits · replaced the rebased copies",
    })
    expect(describePush("auth", testRemoteStatus({ commitsAhead: 1, commitsBehind: 3, fastForward: false, action: DROPS }))).toEqual({
      title: "Force pushed auth",
      description: "1 commit · dropped 3 remote commits",
      warning: true,
    })
  })

  it("says that a push of an amended or restacked branch replaced the older versions, not that it dropped them", () => {
    expect(describePush("auth", testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed auth",
      description: "1 commit · replaced 1 amended commit",
    })
    expect(describePush("login", testRemoteStatus({ commitsRestacked: 2, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed login",
      description: "replaced the parent's older commits",
    })
    expect(describePush("login", testRemoteStatus({ ancestorCommitsAhead: 1, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed login",
      description: "1 new commit of the parent",
    })
    expect(describePush("login", testRemoteStatus({ commitsAhead: 2, commitsAmended: 2, commitsRestacked: 1, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed login",
      description: "2 commits · replaced 2 amended commits and the parent's older commits",
    })
    expect(
      describePush("auth", testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true, replaces: true }) })),
    ).toEqual({
      title: "Force pushed auth",
      description: "1 commit · replaced 1 amended commit · dropped 1 remote commit",
      warning: true,
    })
  })
})

describe("describePush of older versions", () => {
  it("says that a push replaced older messages or commits that landed, without the warning", () => {
    // landed: the remote has older versions of commits the baseline has, nothing of them is lost
    expect(describePush("a", testRemoteStatus({ commitsAhead: 1, commitsLanded: 7, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed a",
      description: "1 commit · replaced 7 landed commits",
    })
    // reworded: the remote has the changes with the old message, a robot would land it
    expect(describePush("a", testRemoteStatus({ commitsReworded: 1, fastForward: false, action: REPLACES }))).toEqual({
      title: "Force pushed a",
      description: "replaced 1 reworded commit",
    })
    expect(getPushHue(REPLACES)).toBe("primary")
    expect(getPushHue(DROPS)).toBe("warning")
  })
})

describe("describeStackPush", () => {
  it("says what the pushes of a stack did together", () => {
    expect(
      describeStackPush([
        { name: "auth", before: testRemoteStatus({ commitsAhead: 2, fastForward: false, action: REBASED }) },
        { name: "login", before: testRemoteStatus({ exists: false, commitsAhead: 3, action: CREATE }) },
        { name: "oauth", before: testRemoteStatus({ exists: false, commitsAhead: 1, action: CREATE }) },
      ]),
    ).toEqual({ title: "Pushed auth, login and oauth", description: "6 commits · created 2 remote branches · force pushed auth" })
    expect(
      describeStackPush([
        { name: "auth", before: testRemoteStatus({ commitsAhead: 1, commitsBehind: 1, fastForward: false, action: DROPS }) },
        { name: "login", before: null },
        { name: "oauth", before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) },
        { name: "session", before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) },
      ]),
    ).toEqual({ title: "Pushed auth, login, oauth and session", description: "3 commits · force pushed auth · dropped 1 remote commit", warning: true })
    // from five on the first three and the count of the rest; the stack branch of the whole stack that went along
    const pushed = ["auth", "login", "oauth", "session", "device"].map(name => ({ name, before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) }))
    expect(describeStackPush(pushed, true)).toEqual({ title: "Pushed auth, login, oauth and 2 more", description: "5 commits · pushed the whole stack branch" })
  })
})

interface BranchOptions {
  remote?: Partial<RemoteStatus> | null
  hasError?: boolean
  commitCount?: number
}

function branch(name: string, { remote = {}, hasError = false, commitCount = 3 }: BranchOptions = {}) {
  const remoteStatus: RemoteStatus | null = remote === null ? null : testRemoteStatus(remote)
  return { name, remoteStatus, hasError, commitCount }
}

describe("getPushColor", () => {
  it("takes the hue from the risk: warning only for a push that drops remote commits", () => {
    expect(getPushColor(branch("a", { remote: null }))).toBeNull()
    expect(getPushColor(branch("a"))).toBe("neutral")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1, action: PUSH } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { exists: false, commitsAhead: 2, action: CREATE } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsBehind: 1, fastForward: false, action: DROPS } }))).toBe("warning")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 1, commitsBehind: 1, fastForward: false, action: DROPS } }))).toBe("warning")
    // a force push of a rebased branch only replaces the remote commits with their copies
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, myCommitsAhead: 1, fastForward: false, action: REBASED } }))).toBe("primary")
    // one of an amended or restacked branch their older versions
    expect(getPushColor(branch("a", { remote: { commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false, action: REPLACES } }))).toBe("primary")
    expect(
      getPushColor(
        branch("a", {
          remote: { commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true, replaces: true }) },
        }),
      ),
    ).toBe("warning")
  })

  it("keeps the push hue for a restacked branch with no commit of the user to push", () => {
    expect(getPushColor(branch("a", { remote: { commitsRestacked: 1, fastForward: false, action: REPLACES } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, commitsRestacked: 1, fastForward: false, action: REPLACES } }))).toBe("primary")
    expect(summarizePushState([branch("a", { remote: { commitsRestacked: 1, fastForward: false, action: REPLACES } })]).byHue).toEqual({ primary: 1, warning: 0 })
  })

  it("is neutral for a safe push of no commit of the user", () => {
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, action: PUSH } }))).toBe("neutral")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, fastForward: false, action: REBASED } }))).toBe("neutral")
    // a first push keeps its hue whoever authored the commits, a push that drops remote commits too
    expect(getPushColor(branch("a", { remote: { exists: false, commitsAhead: 2, action: CREATE } }))).toBe("primary")
    expect(getPushColor(branch("a", { remote: { commitsAhead: 2, commitsBehind: 1, fastForward: false, action: DROPS } }))).toBe("warning")
  })
})

describe("summarizePushState", () => {
  it("sums the outgoing branches in order and counts them by state", () => {
    const summary = summarizePushState([
      branch("pushed"),
      branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 3, action: CREATE } }),
      branch("ahead", { remote: { commitsAhead: 5, myCommitsAhead: 2, action: PUSH } }),
      branch("diverged", { remote: { commitsAhead: 1, commitsBehind: 2, myCommitsAhead: 1, fastForward: false, action: DROPS } }),
    ])
    expect(summary.total).toBe(9)
    expect(summary.mine).toBe(6)
    expect(summary.branches).toEqual([
      { name: "new", count: 3, mine: 3, state: "create", behind: 0, amended: 0, restacked: 0 },
      { name: "ahead", count: 5, mine: 2, state: "push", behind: 0, amended: 0, restacked: 0 },
      { name: "diverged", count: 1, mine: 1, state: "force", behind: 2, amended: 0, restacked: 0 },
    ])
    expect(summary.byState).toEqual({ push: 1, create: 1, force: 1 })
    expect(summary.byHue).toEqual({ primary: 2, warning: 1 })
    expect(summary.notLoaded).toBe(0)
    expect([summary.total, summary.branches.length]).toEqual([9, 3])
  })

  it("counts the commits whose content the remote lacks", () => {
    // a branch not on the remote sends all its commits, by any author
    const created = branch("new", { remote: { exists: false, commitsAhead: 3, myCommitsAhead: 1, action: CREATE }, commitCount: 3 })
    // the sync reports neither the baseline commits nor the rebased copies of pushed commits as ahead
    const rebased = branch("rebased", { remote: { commitsAhead: 1, myCommitsAhead: 1, fastForward: false, action: REBASED }, commitCount: 4 })
    const summary = summarizePushState([created, rebased])
    expect(summary.branches).toEqual([
      { name: "new", count: 3, mine: 1, state: "create", behind: 0, amended: 0, restacked: 0 },
      { name: "rebased", count: 1, mine: 1, state: "force", behind: 0, amended: 0, restacked: 0 },
    ])
    expect(summary.total).toBe(4)
    expect(summary.byHue).toEqual({ primary: 2, warning: 0 })
  })

  it("skips the branches whose row offers no push and counts the ones not loaded", () => {
    const summary = summarizePushState([
      branch("conflict", { remote: { commitsAhead: 2, action: PUSH }, hasError: true }),
      branch("empty", { remote: { exists: false, action: CREATE }, commitCount: 0 }),
      branch("loading", { remote: null }),
      branch("ahead", { remote: { commitsAhead: 1, action: PUSH } }),
    ])
    expect(summary.branches.map(it => it.name)).toEqual(["ahead"])
    expect(summary.notLoaded).toBe(1)
    expect([summary.total, summary.branches.length]).toEqual([1, 1])
  })

  /** Two branches not on the remote yet: one commit and two */
  function twoNew() {
    return [
      branch("feature-auth", { remote: { exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE } }),
      branch("search", { remote: { exists: false, commitsAhead: 2, myCommitsAhead: 2, action: CREATE } }),
    ]
  }

  it("skips a branch the fetched baseline has all of: the update archives it", () => {
    const branches = twoNew()
    const summary = summarizePushState(branches, name => name === "feature-auth")
    expect(summary.branches.map(it => it.name)).toEqual(["search"])
    expect([summary.total, summary.branches.length]).toEqual([2, 1])
    expect(summarizePushState(branches).branches).toHaveLength(2)
  })

  it("skips a branch a running Safe Push holds: its row offers no push", () => {
    expect(
      summarizePushState(
        twoNew(),
        () => false,
        name => name === "search",
      ).branches.map(it => it.name),
    ).toEqual(["feature-auth"])
  })

  it("says why each branch with commits is not outgoing, and counts the work for the remote it leaves out", () => {
    const summary = summarizePushState(
      [
        branch("conflict", { remote: { commitsAhead: 2, action: PUSH }, hasError: true }),
        { ...branch("blocked", { remote: { commitsAhead: 1, action: PUSH } }), status: "Blocked" as const },
        { ...branch("blockedPushed"), status: "Blocked" as const },
        branch("held", { remote: { commitsAhead: 1, action: PUSH } }),
        branch("landed", { remote: { commitsAhead: 1, action: PUSH } }),
        branch("loading", { remote: null }),
        branch("pushed"),
        branch("empty", { commitCount: 0 }),
        branch("ahead", { remote: { commitsAhead: 1, myCommitsAhead: 1, action: PUSH } }),
      ],
      name => name === "landed",
      name => name === "held",
    )
    expect(summary.branches.map(it => it.name)).toEqual(["ahead"])
    expect(summary.skipped).toEqual(
      new Map([
        ["conflict", "failed"],
        ["blocked", "blocked"],
        ["blockedPushed", "blocked"],
        ["held", "held"],
        ["landed", "landed"],
        ["loading", "notLoaded"],
        ["pushed", "upToDate"],
      ]),
    )
    // a blocked branch the remote has as it is withholds nothing; a landed one has nothing of the user's left to push
    expect(summary.withheld).toEqual({ failed: 1, blocked: 1, held: 1 })
  })

  it("is empty when everything is pushed", () => {
    const summary = summarizePushState([branch("a"), branch("b", { remote: null, hasError: true })])
    expect(summary.branches).toEqual([])
    expect(summary.total).toBe(0)
    expect(summary.notLoaded).toBe(0)
  })
})

describe("formatPushCount", () => {
  it("names the older versions a push replaces after the commits it sends, before the ones it drops", () => {
    const count = { mine: 1, count: 1, amended: 0, restacked: 0, landed: 0, reworded: 0, behind: 0 }
    expect(formatPushCount({ ...count, amended: 1 })).toBe("1↑ · 1 amended")
    expect(formatPushCount({ ...count, mine: 4, count: 4, restacked: 2 })).toBe("4↑ · restacked")
    expect(formatPushCount({ ...count, mine: 0, count: 0, restacked: 2 })).toBe("restacked")
    expect(formatPushCount({ ...count, mine: 0, count: 0, amended: 1 })).toBe("1 amended")
    expect(formatPushCount({ ...count, mine: 2, count: 2, behind: 2 })).toBe("2↑ · 2↓")
    expect(formatPushCount({ ...count, mine: 0, count: 0, reworded: 1 })).toBe("1 reworded")
    expect(formatPushCount({ ...count, landed: 7 })).toBe("1↑ · 7 landed")
    expect(formatPushCount({ mine: 1, count: 3, amended: 1, restacked: 1, landed: 0, reworded: 0, behind: 2 })).toBe("1 (3)↑ · 1 amended · restacked · 2↓")
  })
})
