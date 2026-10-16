import { describe, expect, it, vi } from "vite-plus/test"
import { getRobotTiming } from "@/lib/robot-eta"
import { collectRobotOverview, countConflicts, formatRobotIndicatorLabel, getDockProgress, getRobotIndicatorParts, isFailingEntry } from "@/lib/robot-overview"
import { testRobot, robotsById } from "@/lib/test-robot"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch } from "@/stores/test-branch"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

/** 2024-01-02 10:00Z */
const NOW = 1704189600_000
const NOW_SECONDS = NOW / 1000

/** A branch at the commit `${name}-tip` */
function branchAtTip(name: string, patch: Partial<Branch> = {}): Branch {
  const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
  return testBranch(name, null, { commits: [tip], ...patch })
}

const order = ["auth", "login", "docs", "api", "cache"]
const branches = Object.fromEntries(order.map(name => [name, branchAtTip(name)]))

describe("collectRobotOverview", () => {
  it("lists a watched robot of no branch by what it tests, after the branches, also once it failed or merged", () => {
    const watched = { "pr-run": "#101 Rework the session store", "pr-failed": "#102 Add OAuth login", "pr-merged": "#103 Backport", "pr-gone": "#104 Gone" }
    const overview = collectRobotOverview(
      order,
      branches,
      robotsById(
        testRobot("login-run", "login"),
        testRobot("pr-run", null),
        testRobot("pr-failed", null, { status: "failure", finishTime: NOW_SECONDS - 60 }),
        testRobot("pr-merged", null, { status: "success", finishTime: NOW_SECONDS - 60 }),
      ),
      {},
      NOW,
      watched,
    )
    expect(overview.running.map(it => [it.branchName, it.hasRow])).toEqual([
      ["login", true],
      ["#101 Rework the session store", false],
    ])
    expect(overview.attention.map(it => [it.branchName, it.chip.kind])).toEqual([["#102 Add OAuth login", "failed"]])
    expect(overview.finished.map(it => [it.branchName, it.chip.kind])).toEqual([["#103 Backport", "merged"]])
  })

  it("lists the running robots of the branches in the table order, and the branches whose robot failed on them as they are", () => {
    const robots = robotsById(
      testRobot("login-run", "login"),
      testRobot("auth-run", "auth", { status: "canceling" }),
      testRobot("docs-failed", "docs", { status: "failure", finishTime: NOW_SECONDS - 60 }),
      testRobot("api-review", "api", { status: "failure", finishTime: NOW_SECONDS - 60 }),
      // failed on a commit the branch no longer is at: fixed since
      testRobot("cache-failed", "cache", { status: "failure", sourceRevision: "old", finishTime: NOW_SECONDS - 60 }),
      // no branch: never polled, so never shown as running
      testRobot("gone", "gone"),
    )
    const overview = collectRobotOverview(order, branches, robots, { "api-review": true }, NOW)
    expect(overview.running.map(it => it.branchName)).toEqual(["auth", "login"])
    expect(overview.attention.map(it => [it.branchName, it.chip.kind])).toEqual([
      ["docs", "failed"],
      ["api", "needsReview"],
    ])
    expect(overview.running[0]!.chip.label).toBe("Canceling")
    // a branch is listed once
    expect(overview.finished.map(it => it.branchName)).toEqual(["cache"])
    expect(collectRobotOverview(order, branches, undefined, {}, NOW).running).toEqual([])
  })

  it("lists the robots that finished in the last day, the newest of each branch, also of a branch archived since", () => {
    const robots = robotsById(
      testRobot("login-old", "login", { status: "failure", startTime: NOW_SECONDS - 9000, finishTime: NOW_SECONDS - 7200 }),
      testRobot("login-new", "login", { status: "success", pushMode: "dryRun", finishTime: NOW_SECONDS - 3600 }),
      testRobot("merged", "billing", { status: "success", finishTime: NOW_SECONDS - 600 }),
      // a Safe Push of the IDE on the tip of docs
      testRobot("ide", null, { status: "canceled", sourceRevision: "docs-tip", finishTime: NOW_SECONDS - 1800 }),
      testRobot("yesterday", "api", { status: "failure", sourceRevision: "old", finishTime: NOW_SECONDS - 25 * 3600 }),
      // the branch's robot runs again: the running one says it
      testRobot("auth-failed", "auth", { status: "failure", finishTime: NOW_SECONDS - 60 }),
      testRobot("auth-run", "auth"),
    )
    const overview = collectRobotOverview(order, branches, robots, {}, NOW)
    expect(overview.finished.map(it => [it.branchName, it.robot.id, it.hasRow])).toEqual([
      ["billing", "merged", false],
      ["docs", "ide", true],
      ["login", "login-new", true],
    ])
    expect(overview.finished[0]!.chip.label).toBe("Merged")
  })
})

describe("the indicator", () => {
  it("says what runs and what needs the user, and nothing more", () => {
    const overview = collectRobotOverview(
      order,
      branches,
      robotsById(testRobot("a", "auth"), testRobot("b", "login"), testRobot("c", "docs", { status: "failure", finishTime: NOW_SECONDS - 60 })),
      {},
      NOW,
    )
    const parts = getRobotIndicatorParts(overview)
    expect(parts).toEqual([
      { kind: "running", text: "2 running" },
      { kind: "failed", text: "1 failed" },
    ])
    expect(formatRobotIndicatorLabel(parts)).toBe("Patronus robots: 2 running, 1 failed")
    const reviews = collectRobotOverview(
      order,
      branches,
      robotsById(testRobot("a", "auth", { status: "failure", finishTime: NOW_SECONDS - 60 }), testRobot("b", "login", { status: "failure", finishTime: NOW_SECONDS - 60 })),
      { a: true, b: true },
      NOW,
    )
    expect(getRobotIndicatorParts(reviews)).toEqual([{ kind: "needsReview", text: "2 need review" }])
    expect(getRobotIndicatorParts(collectRobotOverview(order, branches, {}, {}, NOW))).toEqual([])
  })

  it("counts a running robot that will fail as failing, not as running, and leaves a canceling one running", () => {
    const overview = collectRobotOverview(
      order,
      branches,
      robotsById(testRobot("a", "auth"), testRobot("b", "login"), testRobot("c", "docs", { status: "canceling" })),
      {},
      NOW,
      {},
      { b: true, c: true },
    )
    expect(overview.running.map(it => [it.branchName, it.chip.label, isFailingEntry(it)])).toEqual([
      ["auth", "Running", false],
      ["login", "Failing", true],
      ["docs", "Canceling", false],
    ])
    const parts = getRobotIndicatorParts(overview)
    expect(parts).toEqual([
      { kind: "running", text: "2 running" },
      { kind: "failing", text: "1 failing" },
    ])
    expect(formatRobotIndicatorLabel(parts)).toBe("Patronus robots: 2 running, 1 failing")
  })
})

describe("getDockProgress", () => {
  it("shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs", () => {
    const start = { startTime: NOW_SECONDS - 600 }
    expect(getDockProgress([])).toEqual({ status: "none" })
    const usual = (seconds: number) => ({ seconds, basis: { kind: "mode" } as const })
    expect(getDockProgress([getRobotTiming(start, null, NOW)])).toEqual({ status: "indeterminate" })
    // past its usual time it has no estimate left either
    expect(getDockProgress([getRobotTiming(start, usual(300), NOW)])).toEqual({ status: "indeterminate" })
    // 10 of 40 minutes, 30 left; 10 of 20 minutes, 10 left: the second is nearer
    expect(getDockProgress([getRobotTiming(start, usual(2400), NOW), getRobotTiming(start, usual(1200), NOW), getRobotTiming(start, null, NOW)])).toEqual({
      status: "normal",
      progress: 50,
    })
  })

  it("counts the conflicts of the last sync for the badge", () => {
    expect(countConflicts({ ...branches, auth: branchAtTip("auth", { status: "MergeConflict" }), login: branchAtTip("login", { status: "AnalyzingConflict" }) })).toBe(2)
    expect(countConflicts(branches)).toBe(0)
  })
})
