import { describe, expect, it } from "vite-plus/test"
import { describePushUpTo, pushUpToPlan } from "@/lib/push-up-to"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

/** `count` commits of `name`, newest first, as a branch lists them: `name-1` the oldest, its virtual id `v-name-1` */
function commits(name: string, count: number): SyncedCommit[] {
  return Array.from({ length: count }, (_, index) => {
    const id = `${name}-${count - index}`
    return { originalHash: id, hash: `v-${id}`, strippedSubject: id, message: id, author: "Ann", authorTime: 0, committerTime: 0 }
  })
}

/** A branch of `count` commits, none on the remote unless `pushed` (oldest first) are */
function branch(name: string, parent: string | null, count: number, overrides: Partial<Branch> = {}, pushed = 0): Branch {
  const list = commits(name, count)
  const unpushed = list
    .toReversed()
    .slice(pushed)
    .map(commit => commit.hash!)
  const remoteStatus =
    pushed === 0
      ? testRemoteStatus({ exists: false, unpushedCommits: unpushed, commitsAhead: unpushed.length, myCommitsAhead: unpushed.length })
      : testRemoteStatus({ unpushedCommits: unpushed, commitsAhead: unpushed.length, myCommitsAhead: unpushed.length, remoteTip: `v-${name}-${pushed}` })
  return testBranch(name, parent, { commits: list, commitCount: count, remoteStatus, ...overrides })
}

function sources(...branches: Branch[]) {
  const byName = new Map(branches.map(it => [it.name, it]))
  return { branchOf: (name: string) => byName.get(name), landingOf: () => null }
}

describe("pushUpToPlan", () => {
  it("pushes the branches the commit is built on whole, root first, then its branch up to it", () => {
    const all = sources(branch("auth", null, 1), branch("login", "auth", 3))
    const plan = pushUpToPlan("login", "login-2", all)
    expect(plan).toEqual({
      ok: true,
      steps: [
        { name: "auth", sent: 1, total: 1 },
        { name: "login", tip: "v-login-2", sent: 2, total: 3 },
      ],
    })
    expect(describePushUpTo(plan!.ok ? plan!.steps : [], all.branchOf)).toBe(
      "Pushes 2 branches, parents first:\nauth · 1 commit · creates the remote branch\nlogin · 2 of 3 commits · creates the remote branch",
    )
    // the last commit the remote lacks is the push of the whole branch
    expect(pushUpToPlan("login", "login-3", all)).toEqual({
      ok: true,
      steps: [
        { name: "auth", sent: 1, total: 1 },
        { name: "login", sent: 3, total: 3 },
      ],
    })
  })

  it("cuts only the commits the remote lacks of a branch the remote has an older state of", () => {
    // on the remote up to login-1: a cut at login-3 sends 2 of 3
    const pushed = sources(branch("login", null, 4, {}, 1))
    expect(pushUpToPlan("login", "login-3", pushed)).toEqual({ ok: true, steps: [{ name: "login", tip: "v-login-3", sent: 2, total: 3 }] })
    // the remote has it: nothing to offer
    expect(pushUpToPlan("login", "login-1", pushed)).toBeNull()
    // rewritten since its push: a part would rewind the pull request
    const rewritten = branch("login", null, 3, {}, 1)
    const plan = pushUpToPlan("login", "login-2", sources({ ...rewritten, remoteStatus: { ...rewritten.remoteStatus!, fastForward: false } }))
    expect(plan).toEqual({ ok: false, reason: "login was rewritten since its push: it pushes whole." })
  })

  it("stops at an ancestor that can't be pushed or asks first, and at a branch a running Safe Push holds", () => {
    const auth = branch("auth", null, 1)
    const login = branch("login", "auth", 2)
    expect(pushUpToPlan("login", "login-1", sources({ ...auth, status: "Blocked", statusText: "blocked by core" }, login))).toEqual({
      ok: false,
      reason: "auth can't be pushed: blocked by core.",
      show: "auth",
    })
    const dropping = branch("auth", null, 2, {}, 1)
    const behind = { ...dropping, remoteStatus: { ...dropping.remoteStatus!, commitsBehind: 2, fastForward: false } }
    expect(pushUpToPlan("login", "login-1", sources(behind, login))).toEqual({ ok: false, reason: "auth's push drops 2 remote commits: Force Push auth first.", show: "auth" })
    const landing = { branchOf: sources(auth, login).branchOf, landingOf: (name: string) => (name === "auth" ? "oauth" : null) }
    expect(pushUpToPlan("login", "login-1", landing)).toEqual({ ok: false, reason: "A Safe Push of oauth lands all of auth.", show: "oauth" })
    // its own Safe Push holds the branch: Push and Restart is the push
    const own = { branchOf: sources(auth, login).branchOf, landingOf: (name: string) => (name === "login" ? "login" : null) }
    expect(pushUpToPlan("login", "login-1", own)).toEqual({ ok: false, reason: "A Safe Push of login is running: wait until it merges, or push and restart it.", show: "login" })
  })
})
