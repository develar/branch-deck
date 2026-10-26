import { describe, expect, it } from "vite-plus/test"
import type { Landed, Reason } from "@/lib/bindings"
import { testRobot } from "@/lib/test-robot"
import { formatLanded, formatLandedWhole, formatLandingReason, formatPushHoldReason, formatReason, formatStackHold, startCheck } from "@/presenters/reasons"

/** Every code Rust gives, with an example (`reason_catalog_test.rs` writes it) */
const CATALOG = Object.values(import.meta.glob<Reason[]>("/crates/domain-policy/reasons.json", { eager: true, import: "default" }))[0]!

describe("formatReason", () => {
  it("words every reason of the catalog Rust writes as the page says it", () => {
    expect(CATALOG.map(reason => formatReason(reason))).toEqual([
      "Blocked by auth: fix it first, then sync",
      "Blocked: fix the parent branch first",
      "A Dry Run of the last commit is running",
      "A Safe Push of oauth is running: it lands 2 commits of auth with it",
      "A Safe Push of the whole air stack is running: it lands auth with it",
      "A Safe Push of auth is running: wait until it merges",
      "Landed in master with oauth",
      "Merged by Patronus",
      "A Safe Push of session is running: it lands 1 commit of login with it",
      "A Safe Push of auth is running: wait until it merges, or restart it on the current branch",
      "The auth stack has no stack branch: only a stack that forks builds one",
      "The stack branch conflicts at 01234567 in a.kt: the stack lands in pieces",
      "Blocked by login: fix it first, then sync",
      "Holds parked login: land the stack in pieces",
      "Landed in master with deploy and the whole auth stack",
      "A Safe Push of the whole auth stack is running: wait until it merges, or restart it on the current stack",
      "auth can't be pushed: it failed to sync",
      "The remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them",
      "Built on parked auth: Unpark auth to push",
      "Parked: Unpark it to push",
      "Patronus does not push into master",
      "auth has commits the remote lacks: the review request pushes them first",
      "Patronus names no code owner with a GitHub login of IJPL Code Platform: choose the reviewers",
      "#7 is a draft: Patronus merges it once it is ready for review",
      "Approvals in #7 don't count: its base is release/253",
      "Patronus checks the approvals when the robot starts and fails without them",
      "The push dismisses the approvals of #7: push first, then ask for the review again",
      "“Use parseFlow” of flow uses parseFlow (flow/Flow.kt:3), which “Add parseFlow” of util adds: git merges it, but the build of flow fails. Stack flow on util to land it too.",
      "“Remove the env” of fake-agent removes CONTROL_URL_ENV, which acp/Host.kt:7 still uses, and “Read the url” of acp changes that use: the build of fake-agent fails. Move that commit into fake-agent, or stack fake-agent on its branch.",
      "01234567 conflicts in a.kt with the newest target: Patronus would fail the robot at once",
      "Landed in origin/master",
      "Pull request #7 is merged",
    ])
    // every code of the bindings has an example: the formatter's table is typed by the codes
    expect(new Set(CATALOG.map(reason => reason.code)).size).toBe(25)
  })

  it("names why the sync did not build a branch by its status text, as its row says it", () => {
    const cantPush = CATALOG.find(reason => reason.code === "cantPush")!
    expect(formatReason(cantPush, () => "merge conflict")).toBe("auth can't be pushed: merge conflict")
  })

  it("says that nothing is left to push of a stack branch merged Safe Pushes landed, and nothing for no hold", () => {
    const landed = CATALOG.find(reason => reason.code === "stackLanded")!
    expect(formatStackHold(landed)).toBe("Landed in master with deploy and the whole auth stack: nothing of it is left to push")
    expect(formatStackHold(CATALOG.find(reason => reason.code === "stackParked"))).toBe("Holds parked login: land the stack in pieces")
    expect(formatStackHold(null)).toBeNull()
  })

  it("starts without a reason and refuses with its words", () => {
    expect(startCheck(null)).toEqual({ ok: true })
    expect(startCheck(CATALOG[2])).toEqual({ ok: false, reason: "A Dry Run of the last commit is running" })
  })
})

describe("formatPushHoldReason", () => {
  it("says why a held branch offers no push: its own Safe Push, one that lands it with the commits it counts", () => {
    const running = testRobot("r1", "IJPL-1")
    expect(formatPushHoldReason("IJPL-1", { kind: "own", branch: "IJPL-1", robot: running })).toBe(
      "A Safe Push of IJPL-1 is running: wait until it merges, or restart it on the current branch",
    )
    const oauth = testRobot("oauth", "oauth")
    expect(formatPushHoldReason("auth", { kind: "span", branch: "oauth", robot: oauth })).toBe("A Safe Push of oauth is running: it lands auth with it")
    // once the number of its commits the robot lands is told, the reason names it, as `bd` does
    expect(formatPushHoldReason("auth", { kind: "span", branch: "oauth", robot: oauth, commits: 2 })).toBe("A Safe Push of oauth is running: it lands 2 commits of auth with it")
    expect(formatLandingReason("auth", { branch: "oauth", robot: oauth, commits: 1 })).toBe("A Safe Push of oauth is running: it lands 1 commit of auth with it")
  })

  it("says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge", () => {
    const oauth = testRobot("oauth", "oauth", { status: "success", finishTime: 200 })
    expect(formatPushHoldReason("auth", { kind: "landed", branch: "oauth", robot: oauth })).toBe("Landed in master with oauth: nothing of it is left to push")
    const whole = testRobot("whole", null, { stackRoot: "air", status: "success" })
    expect(formatPushHoldReason("bazel", { kind: "landed", branch: "air", robot: whole, wholeStack: true })).toBe(
      "Landed in master with the whole air stack: nothing of it is left to push",
    )
    expect(formatPushHoldReason("oauth", { kind: "landed", branch: "oauth", robot: oauth })).toBe("Merged by Patronus: nothing of it is left to push")
  })
})

describe("formatLanded", () => {
  it("words what landed a subject: the baseline, merged Safe Pushes, its own robot, its pull request", () => {
    const of = (branch: string, wholeStack = false) => ({ branch, robotId: `${branch}-robot`, wholeStack })
    const robots = (by: ReturnType<typeof of>[], own = false): Landed => ({ by: { kind: "robots", by, own, target: "master" }, after: [] })
    expect(formatLanded({ by: { kind: "baseline", upstream: "origin/master" }, after: [] })).toBe("Landed in origin/master")
    expect(formatLanded(robots([of("oauth")]))).toBe("Landed in master with oauth")
    expect(formatLanded(robots([of("login"), of("air", true), of("login")]))).toBe("Landed in master with login and the whole air stack")
    expect(formatLanded(robots([of("auth")], true))).toBe("Merged by Patronus")
    expect(formatLanded({ by: { kind: "pullRequest", number: 7 }, after: [] })).toBe("Pull request #7 is merged")
    // a part of it landed: the commits after it are left to land
    expect(formatLandedWhole({ ...robots([of("oauth")]), after: ["o2"] })).toBeNull()
    expect(formatLandedWhole(undefined)).toBeNull()
  })
})
