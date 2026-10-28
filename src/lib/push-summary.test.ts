import { describe, expect, it } from "vite-plus/test"
import { describePush, describeStackPush, formatPushCount, formatPushedBranch, formatPushFailure, getPushColor, getPushHue } from "@/lib/push-summary"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { RemoteStatus } from "@/stores/branch-sync"
import { CREATE, forcePush, PUSH, testRemoteStatus } from "@/stores/test-branch"

/** A force push that only replaces the rebased copies, one that replaces older versions, one that drops remote commits */
const REBASED = forcePush()
const REPLACES = forcePush({ replaces: true })
const DROPS = forcePush({ drops: true })

describe("formatPushedBranch", () => {
  it("names the remote branch a push sends to, beside a push into the target", () => {
    expect(formatPushedBranch("auth")).toBe("branch auth")
    expect(formatPushedBranch(stackReadinessKey("air"))).toBe("the whole stack branch of air")
    expect(describePush(formatPushedBranch("auth"), null)).toEqual({ title: "Pushed branch auth" })
    expect(formatPushFailure("auth")).toBe("Cannot push branch auth")
    expect(formatPushFailure(stackReadinessKey("air"))).toBe("Cannot push the whole air stack")
  })
})

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
    ).toEqual({ title: "Pushed branches auth, login and oauth", description: "6 commits · created 2 remote branches · force pushed auth" })
    expect(
      describeStackPush([
        { name: "auth", before: testRemoteStatus({ commitsAhead: 1, commitsBehind: 1, fastForward: false, action: DROPS }) },
        { name: "login", before: null },
        { name: "oauth", before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) },
        { name: "session", before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) },
      ]),
    ).toEqual({ title: "Pushed branches auth, login, oauth and session", description: "3 commits · force pushed auth · dropped 1 remote commit", warning: true })
    // from five on the first three and the count of the rest; the stack branch of the whole stack that went along
    const pushed = ["auth", "login", "oauth", "session", "device"].map(name => ({ name, before: testRemoteStatus({ commitsAhead: 1, action: PUSH }) }))
    expect(describeStackPush(pushed, true)).toEqual({ title: "Pushed branches auth, login, oauth and 2 more", description: "5 commits · pushed the whole stack branch" })
    expect(describeStackPush(pushed.slice(0, 1), true)).toMatchObject({ title: "Pushed branch auth" })
  })
})

/** A branch with the remote status `remote`, not loaded for null */
function branch(remote: Partial<RemoteStatus> | null = {}) {
  return { remoteStatus: remote === null ? null : testRemoteStatus(remote) }
}

describe("getPushColor", () => {
  it("takes the hue from the risk: warning only for a push that drops remote commits", () => {
    expect(getPushColor(branch(null))).toBeNull()
    expect(getPushColor(branch())).toBe("neutral")
    expect(getPushColor(branch({ commitsAhead: 2, myCommitsAhead: 1, action: PUSH }))).toBe("primary")
    expect(getPushColor(branch({ exists: false, commitsAhead: 2, action: CREATE }))).toBe("primary")
    expect(getPushColor(branch({ commitsBehind: 1, fastForward: false, action: DROPS }))).toBe("warning")
    expect(getPushColor(branch({ commitsAhead: 1, commitsBehind: 1, fastForward: false, action: DROPS }))).toBe("warning")
    // a force push of a rebased branch only replaces the remote commits with their copies
    expect(getPushColor(branch({ commitsAhead: 2, myCommitsAhead: 1, fastForward: false, action: REBASED }))).toBe("primary")
    // one of an amended or restacked branch their older versions
    expect(getPushColor(branch({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false, action: REPLACES }))).toBe("primary")
    expect(
      getPushColor(branch({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true, replaces: true }) })),
    ).toBe("warning")
  })

  it("keeps the push hue for a restacked branch with no commit of the user to push", () => {
    expect(getPushColor(branch({ commitsRestacked: 1, fastForward: false, action: REPLACES }))).toBe("primary")
    expect(getPushColor(branch({ commitsAhead: 2, commitsRestacked: 1, fastForward: false, action: REPLACES }))).toBe("primary")
  })

  it("is neutral for a safe push of no commit of the user", () => {
    expect(getPushColor(branch({ commitsAhead: 2, action: PUSH }))).toBe("neutral")
    expect(getPushColor(branch({ commitsAhead: 2, fastForward: false, action: REBASED }))).toBe("neutral")
    // a first push keeps its hue whoever authored the commits, a push that drops remote commits too
    expect(getPushColor(branch({ exists: false, commitsAhead: 2, action: CREATE }))).toBe("primary")
    expect(getPushColor(branch({ commitsAhead: 2, commitsBehind: 1, fastForward: false, action: DROPS }))).toBe("warning")
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
