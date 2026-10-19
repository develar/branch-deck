import { describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot } from "@/lib/bindings"
import { firstBranchOfEachState, getStackAttention, summarizeStackState } from "@/lib/stack-state"
import type { StackState } from "@/lib/stack-state"
import { robotsById, testRobot } from "@/lib/test-robot"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

/** A branch at the commit `${name}-tip`, pushed and up to date unless `patch` says otherwise */
function branch(name: string, patch: Partial<Branch> = {}): Branch {
  const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
  return testBranch(name, "root", { commits: [tip], remoteStatus: testRemoteStatus({ remoteTip: `${name}-tip` }), ...patch })
}

/** A robot that failed on the tip of `branchName` */
function failed(id: string, branchName: string, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return testRobot(id, branchName, { status: "failure", finishTime: 1704189300, ...patch })
}

const NONE: StackState = { conflicts: 0, errors: 0, blocked: 0, failed: 0, needsReview: 0, failing: 0, running: 0, toPush: 0 }

describe("summarizeStackState", () => {
  it("counts a failed robot as the header does: on the branch as it is, apart from one that needs review", () => {
    const branches = [branch("session"), branch("login"), branch("oauth"), branch("docs")]
    const robots = robotsById(
      failed("session-failed", "session"),
      // failed on a commit the branch no longer is at: fixed since, its row offers a new Safe Push
      failed("login-failed", "login", { sourceRevision: "old" }),
      failed("oauth-review", "oauth"),
      testRobot("docs-run", "docs"),
    )
    const state = summarizeStackState(branches, { robots, needsReview: { "oauth-review": true }, failing: {}, isLanded: () => false })
    expect(state).toEqual({ ...NONE, failed: 1, needsReview: 1, running: 1 })
    // a running robot whose required check failed is failing, not running
    expect(summarizeStackState(branches, { robots, needsReview: { "oauth-review": true }, failing: { "docs-run": true }, isLanded: () => false })).toEqual({
      ...NONE,
      failed: 1,
      needsReview: 1,
      failing: 1,
    })
    expect(summarizeStackState(branches, { robots: undefined, needsReview: {}, failing: {}, isLanded: () => false })).toEqual(NONE)
  })

  it("counts no failure of a branch another robot holds or landed, or the baseline has: the Safe Push of a branch built on it lands it", () => {
    const branches = [branch("session"), branch("oauth")]
    const robots = robotsById(failed("session-failed", "session"), failed("oauth-review", "oauth"))
    const sources = { robots, needsReview: { "oauth-review": true }, failing: {}, isLanded: () => false }
    expect(summarizeStackState(branches, sources)).toEqual({ ...NONE, failed: 1, needsReview: 1 })
    expect(summarizeStackState(branches, { ...sources, isHeld: name => name === "session" })).toEqual({ ...NONE, needsReview: 1 })
    expect(summarizeStackState(branches, { ...sources, isLanded: name => name === "oauth" })).toEqual({ ...NONE, failed: 1 })
    expect(firstBranchOfEachState(branches, { ...sources, isHeld: () => true })).toEqual({})
  })

  it("counts the push by the rule of the outgoing counts: not a landed branch, nor one without commits", () => {
    const ahead = testRemoteStatus({ commitsAhead: 2, myCommitsAhead: 2 })
    const branches = [
      branch("session", { remoteStatus: ahead }),
      branch("login", { remoteStatus: ahead }),
      branch("oauth", { remoteStatus: ahead, commitCount: 0 }),
      branch("docs", { remoteStatus: ahead, status: "Blocked" }),
    ]
    const state = summarizeStackState(branches, { robots: undefined, needsReview: {}, failing: {}, isLanded: name => name === "login" })
    expect(state).toEqual({ ...NONE, blocked: 1, toPush: 1 })
  })

  it("counts the robot of the whole stack, which runs on its stack branch, as the root's: running, failing or failed on it as it is", () => {
    const branches = [branch("air"), branch("bazel"), branch("side-fix")]
    const stack = { root: "air", state: "Built" as const, tip: "stack-tip", remoteStatus: testRemoteStatus({ remoteTip: "stack-tip" }) }
    const sources = (...robots: PatronusRobot[]) => ({ robots: robotsById(...robots), needsReview: {}, failing: {}, isLanded: () => false })
    const whole = testRobot("whole", null, { stackRoot: "air", sourceRevision: "stack-tip" })
    expect(summarizeStackState(branches, sources(whole), stack)).toEqual({ ...NONE, running: 1 })
    expect(firstBranchOfEachState(branches, sources(whole), stack)).toEqual({ running: "air" })
    // no branch has it: without its stack branch the stack holds nothing
    expect(summarizeStackState(branches, sources(whole))).toEqual(NONE)
    expect(summarizeStackState(branches, { ...sources(whole), failing: { whole: true } }, stack)).toEqual({ ...NONE, failing: 1 })
    // failed on the stack branch as it is; one of an older stack branch is fixed since
    const failedOn = (sourceRevision: string) => failed("whole", "air", { branchName: null, stackRoot: "air", sourceRevision })
    expect(summarizeStackState(branches, sources(failedOn("stack-tip")), stack)).toEqual({ ...NONE, failed: 1 })
    expect(summarizeStackState(branches, sources(failedOn("old-stack")), stack)).toEqual(NONE)
  })

  it("counts no failure of the robot of the whole stack on a stack branch the sync did not build: its restart is refused, the stack lands in pieces", () => {
    const branches = [branch("air"), branch("bazel")]
    const sources = (...robots: PatronusRobot[]) => ({ robots: robotsById(...robots), needsReview: {}, failing: {}, isLanded: () => false })
    const failedRobot = failed("whole", "air", { branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })
    const running = testRobot("whole", null, { stackRoot: "air", sourceRevision: "stack-tip" })
    for (const state of ["Stale", "Blocked", "Conflict"] as const) {
      // kept for its remote branch once the stack no longer forks, or not rebuilt: its tip is not the stack's commits
      const stack = { root: "air", state, tip: "stack-tip", remoteStatus: testRemoteStatus({ remoteTip: "stack-tip" }) }
      expect(summarizeStackState(branches, sources(failedRobot), stack)).toEqual(NONE)
      expect(summarizeStackState(branches, { ...sources(failedRobot), needsReview: { whole: true } }, stack)).toEqual(NONE)
      expect(firstBranchOfEachState(branches, sources(failedRobot), stack)).toEqual({})
      // a robot that still runs on it lands the stack: counted
      expect(summarizeStackState(branches, sources(running), stack)).toEqual({ ...NONE, running: 1 })
    }
  })

  it("tells a conflict from another sync error", () => {
    const conflict = branch("session", { hasError: true, errorDetails: { MergeConflict: {} } as Branch["errorDetails"] })
    const error = branch("login", { hasError: true })
    expect(summarizeStackState([conflict, error], { robots: undefined, needsReview: {}, failing: {}, isLanded: () => false })).toEqual({ ...NONE, conflicts: 1, errors: 1 })
  })
})

describe("getStackAttention", () => {
  it("puts what waits for the user before what runs, and the push last", () => {
    const all: StackState = { conflicts: 2, errors: 1, blocked: 1, failed: 2, needsReview: 1, failing: 1, running: 3, toPush: 4 }
    expect(getStackAttention(all)).toEqual({ text: "2 conflicts", color: "error", ring: false })
    expect(getStackAttention({ ...all, conflicts: 0 })?.text).toBe("1 error")
    expect(getStackAttention({ ...all, conflicts: 0, errors: 0 })).toEqual({ text: "1 blocked", color: "warning", ring: false })
    // a failed robot is not hidden behind a running one
    expect(getStackAttention({ ...NONE, failed: 2, running: 3, toPush: 4 })).toEqual({ text: "2 failed", color: "error", ring: false })
    expect(getStackAttention({ ...NONE, needsReview: 2, running: 3 })).toEqual({ text: "2 need review", color: "warning", ring: false })
    expect(getStackAttention({ ...NONE, needsReview: 1 })?.text).toBe("1 needs review")
    expect(getStackAttention({ ...NONE, failing: 1, running: 3, toPush: 4 })).toEqual({ text: "1 failing", color: "error", ring: true })
    expect(getStackAttention({ ...NONE, running: 3, toPush: 4 })).toEqual({ text: "3 running", color: "primary", ring: true })
    expect(getStackAttention({ ...NONE, toPush: 4 })).toEqual({ text: "4 to push", color: "primary", ring: false })
    expect(getStackAttention(NONE)).toBeNull()
  })
})
