import { describe, expect, it } from "vite-plus/test"
import type { ParkedState } from "@/lib/parked"
import type { PushHold } from "@/lib/patronus"
import { describePushUpTo, pushUpToPlan } from "@/lib/push-up-to"
import { testRobot } from "@/lib/test-robot"
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
  return { branchOf: (name: string) => byName.get(name), holdOf: () => null, landedOf: () => null, parkedOf: () => null }
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

  it("offers nothing for a commit a merged Safe Push landed, only for one added after the landed ones, and leaves out a branch it landed whole", () => {
    const auth = branch("auth", null, 1)
    const login = branch("login", "auth", 3)
    const merged = { ...testRobot("login", "login"), status: "success" as const }
    // its Safe Push landed login-1 and login-2 with auth; login-3 came after
    const landedLogin = { branch: "login", robot: merged, own: true, after: ["login-3"] }
    const landedAuth = { branch: "login", robot: merged, after: [] }
    const landed = {
      ...sources(auth, login),
      holdOf: (name: string): PushHold | null => (name === "auth" ? { kind: "landed", branch: "login", robot: merged } : null),
      landedOf: (name: string) => (name === "login" ? landedLogin : name === "auth" ? landedAuth : null),
    }
    expect(pushUpToPlan("login", "login-1", landed)).toBeNull()
    expect(pushUpToPlan("login", "login-2", landed)).toBeNull()
    expect(pushUpToPlan("auth", "auth-1", landed)).toBeNull()
    // the one commit left is the push of the whole branch, without auth: all of it is in the target
    expect(pushUpToPlan("login", "login-3", landed)).toEqual({ ok: true, steps: [{ name: "login", sent: 3, total: 3 }] })
  })

  it("keeps a parked branch and a branch built on it local, in the words of their push button", () => {
    const auth = branch("auth", null, 1)
    const login = branch("login", "auth", 2)
    const parked = (states: Record<string, ParkedState>) => ({ ...sources(auth, login), parkedOf: (name: string) => states[name] ?? null })
    const own = { note: "", with: null }
    expect(pushUpToPlan("auth", "auth-1", parked({ auth: own }))).toEqual({ ok: false, reason: "Parked: Unpark it to push" })
    expect(pushUpToPlan("login", "login-1", parked({ auth: own, login: { note: "", with: "auth" } }))).toEqual({
      ok: false,
      reason: "Built on parked auth: Unpark auth to push",
      show: "auth",
    })
    // a commit the remote has offers nothing, parked or not
    expect(pushUpToPlan("login", "login-1", { ...sources(branch("login", null, 2, {}, 1)), parkedOf: () => own })).toBeNull()
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
    const holding = (name: string, hold: PushHold) => ({ ...sources(auth, login), holdOf: (it: string) => (it === name ? hold : null) })
    const oauth = testRobot("oauth", "oauth")
    expect(pushUpToPlan("login", "login-1", holding("auth", { kind: "span", branch: "oauth", robot: oauth }))).toEqual({
      ok: false,
      reason: "A Safe Push of oauth lands all of auth.",
      show: "oauth",
    })
    const whole = testRobot("whole", null, { stackRoot: "auth" })
    expect(pushUpToPlan("login", "login-1", holding("auth", { kind: "span", branch: "auth", robot: whole, wholeStack: true }))).toEqual({
      ok: false,
      reason: "A Safe Push of the whole auth stack lands all of auth.",
      show: "auth",
    })
    // its own Safe Push holds the branch: its restart is the push
    const own = holding("login", { kind: "own", branch: "login", robot: testRobot("login", "login") })
    expect(pushUpToPlan("login", "login-1", own)).toEqual({
      ok: false,
      reason: "A Safe Push of login is running: wait until it merges, or restart it on the current branch.",
      show: "login",
    })
  })
})
