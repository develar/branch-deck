import { describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, RobotAttention } from "@/lib/bindings"
import { getRobotTiming } from "@/lib/robot-eta"
import type { RobotOverviewEntry, RowRobots } from "@/lib/robot-overview"
import { NO_ROW_ROBOTS, collectRobotOverview, countConflicts, getDockProgress, orderLandingRobots } from "@/lib/robot-overview"
import { testRobot, robotsById } from "@/lib/test-robot"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { testBranch, testTip } from "@/stores/test-branch"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

/** 2024-01-02 10:00Z */
const NOW = 1704189600_000
const NOW_SECONDS = NOW / 1000

/** A branch at the commit `${name}-tip` */
function branchAtTip(name: string, patch: Partial<Branch> = {}): Branch {
  const tip = testTip(name)
  return testBranch(name, null, { commits: [tip], ...patch })
}

const order = ["auth", "login", "docs", "api", "cache"]

/** The robot of each row as the session matched it, by branch and by the root of a whole stack */
function rows(branches: Readonly<Record<string, string>>, stacks: Readonly<Record<string, string>> = {}): RowRobots {
  return { branches, stacks }
}
const branches = Object.fromEntries(order.map(name => [name, branchAtTip(name)]))

describe("collectRobotOverview", () => {
  it("lists the robot of the whole stack as the whole stack after the branches, a click showing the root, its failure as the move the view names, any other as finished", () => {
    const stack = (root: string, tip: string, state: StackBranch["state"] = "Built"): StackBranch => ({
      root,
      branchName: `user/stack/${root}`,
      members: state === "Stale" ? [] : [root, "login"],
      commitCount: 2,
      state,
      tip,
      blockedBy: null,
      conflict: null,
      error: null,
      remoteStatus: null,
    })
    const robots = robotsById(
      testRobot("login-run", "login"),
      testRobot("whole", null, { stackRoot: "auth", sourceRevision: "stack-tip" }),
      testRobot("whole-failed", null, { stackRoot: "docs", sourceRevision: "docs-stack-tip", status: "failure", finishTime: NOW_SECONDS - 60 }),
      // failed on a stack branch rebuilt since: nothing to act on
      testRobot("whole-old", null, { stackRoot: "api", sourceRevision: "old", status: "failure", finishTime: NOW_SECONDS - 60 }),
      // failed on a stack branch kept for its remote branch once the stack no longer forks: the stack lands in pieces
      testRobot("whole-stale", null, { stackRoot: "cache", sourceRevision: "cache-stack-tip", status: "failure", finishTime: NOW_SECONDS - 120 }),
    )
    const matched = rows({ login: "login-run" }, { auth: "whole", docs: "whole-failed", api: "whole-old", cache: "whole-stale" })
    const stacks = {
      auth: stack("auth", "stack-tip"),
      docs: stack("docs", "docs-stack-tip"),
      api: stack("api", "api-stack-tip"),
      cache: stack("cache", "cache-stack-tip", "Stale"),
    }
    // Rust names the failure of the docs stack on its built stack branch as it is (`RepositoryView.attention`)
    const overview = collectRobotOverview(order, branches, robots, matched, {}, NOW, {}, {}, stacks, { "stack:docs": { robotId: "whole-failed" } })
    expect(overview.running.map(it => [it.branchName, it.stackRoot, it.hasRow])).toEqual([
      ["login", undefined, true],
      ["Whole auth stack", "auth", true],
    ])
    expect(overview.attention.map(it => [it.branchName, it.chip.kind])).toEqual([["Whole docs stack", "failed"]])
    expect(overview.finished.map(it => it.branchName)).toEqual(["Whole api stack", "Whole cache stack"])
    // a Safe Push of a branch of the docs stack lands a part of it: the view names no failure, it is in the last day
    const carried = collectRobotOverview(order, branches, robots, matched, {}, NOW, {}, {}, stacks)
    expect(carried.attention).toEqual([])
    expect(carried.finished.map(it => it.branchName)).toEqual(["Whole docs stack", "Whole api stack", "Whole cache stack"])
  })

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
      rows({ login: "login-run" }),
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

  it("asks nothing for the failure of a watched robot once its pull request was started again: the newest robot is the one to act on", () => {
    const label = "#102 Add OAuth login"
    const failed = testRobot("pr-failed", null, { status: "failure", startTime: NOW_SECONDS - 3600, finishTime: NOW_SECONDS - 600 })
    const watched = { "pr-failed": label, "pr-again": label }
    const again = (patch: Partial<PatronusRobot>) =>
      collectRobotOverview(order, branches, robotsById(failed, testRobot("pr-again", null, { startTime: NOW_SECONDS - 300, ...patch })), NO_ROW_ROBOTS, {}, NOW, watched)
    expect(again({}).running.map(it => it.robot.id)).toEqual(["pr-again"])
    expect(again({}).attention).toEqual([])
    expect(again({}).finished.map(it => it.robot.id)).toEqual(["pr-failed"])
    // the new one failed too: one failure to act on, its own
    expect(again({ status: "failure", finishTime: NOW_SECONDS - 60 }).attention.map(it => it.robot.id)).toEqual(["pr-again"])
  })

  it("lists the running robots of the branches in the table order, and the branches whose robot needs the user as the view names them", () => {
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
    // the robot the session matched to each row: the robot of a branch no row has is no row's
    const matched = rows({ login: "login-run", auth: "auth-run", docs: "docs-failed", api: "api-review", cache: "cache-failed" })
    const attention = { docs: { robotId: "docs-failed" }, api: { robotId: "api-review" } }
    const overview = collectRobotOverview(order, branches, robots, matched, { "api-review": true }, NOW, {}, {}, {}, attention)
    expect(overview.running.map(it => it.branchName)).toEqual(["auth", "login"])
    expect(overview.attention.map(it => [it.branchName, it.chip.kind])).toEqual([
      ["docs", "failed"],
      ["api", "needsReview"],
    ])
    expect(overview.running[0]!.chip.label).toBe("Canceling")
    // a branch is listed once
    expect(overview.finished.map(it => it.branchName)).toEqual(["cache"])
    expect(collectRobotOverview(order, branches, undefined, matched, {}, NOW).running).toEqual([])
  })

  it("asks nothing for a failure the view does not name, nor for one it names of another robot than the row's: it is finished history", () => {
    // docs failed on itself; then a Safe Push of a branch built on it took it along, or merged it, or master has it
    const robots = robotsById(testRobot("docs-failed", "docs", { status: "failure", finishTime: NOW_SECONDS - 60 }))
    const matched = rows({ docs: "docs-failed" })
    const needs = { docs: { robotId: "docs-failed" } }
    expect(collectRobotOverview(order, branches, robots, matched, {}, NOW, {}, {}, {}, needs).attention.map(it => it.branchName)).toEqual(["docs"])
    const none: Readonly<Record<string, RobotAttention>> = {}
    for (const attention of [none, { docs: { robotId: "older" } }]) {
      const overview = collectRobotOverview(order, branches, robots, matched, {}, NOW, {}, {}, {}, attention)
      expect(overview.attention).toEqual([])
      expect(overview.finished.map(it => [it.branchName, it.chip.kind])).toEqual([["docs", "failed"]])
    }
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
    const overview = collectRobotOverview(order, branches, robots, rows({ login: "login-new", docs: "ide", api: "yesterday", auth: "auth-run" }), {}, NOW)
    expect(overview.finished.map(it => [it.branchName, it.robot.id, it.hasRow])).toEqual([
      ["billing", "merged", false],
      ["docs", "ide", true],
      ["login", "login-new", true],
    ])
    expect(overview.finished[0]!.chip.label).toBe("Merged")
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

  it("queues the Safe Pushes nearest to finishing first, those without an estimate last, then the Dry Runs", () => {
    const start = { startTime: NOW_SECONDS - 600 }
    const usual = (seconds: number) => ({ seconds, basis: { kind: "mode" } as const })
    const running = (name: string, pushMode: PatronusRobot["pushMode"] = "rebase"): RobotOverviewEntry => ({
      branchName: name,
      hasRow: true,
      robot: testRobot(`${name}-robot`, name, { pushMode, ...start }),
      chip: { kind: "running", color: "neutral", label: "Running", commit: "current", outdated: false },
    })
    // 30, 10 minutes left; no history; a Dry Run 1 minute left
    const estimates: Record<string, number | null> = { auth: 2400, login: 1200, docs: null, dry: 660, api: 2400 }
    const entries = [running("auth"), running("docs"), running("dry", "dryRun"), running("login"), running("api")]
    const order = orderLandingRobots(entries, entry => {
      const seconds = estimates[entry.branchName] ?? null
      return getRobotTiming(entry.robot, seconds === null ? null : usual(seconds), NOW)
    })
    // auth and api tie: the table order
    expect(order.map(it => it.branchName)).toEqual(["login", "auth", "api", "docs", "dry"])
  })

  it("counts the conflicts of the last sync for the badge", () => {
    expect(countConflicts({ ...branches, auth: branchAtTip("auth", { status: "MergeConflict" }), login: branchAtTip("login", { status: "AnalyzingConflict" }) })).toBe(2)
    expect(countConflicts(branches)).toBe(0)
  })
})
