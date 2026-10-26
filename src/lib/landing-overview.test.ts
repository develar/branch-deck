// @spec spec/repository/landing-overview.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingNodeKind } from "@/lib/bindings"
import type { LandingUnit } from "@/lib/landing-overview"
import { collectLandingOverview, formatFinishedSummary, formatLandingSectionTitle, formatLandingTitle, getLandingBarSegments, isLandingOverviewEmpty } from "@/lib/landing-overview"
import type { RobotOverview, RobotOverviewEntry } from "@/lib/robot-overview"
import { EMPTY_ROBOT_OVERVIEW } from "@/lib/robot-overview"
import { testRobot } from "@/lib/test-robot"
import type { LandingNode } from "@/presenters/landing-node"

function node(kind: LandingNodeKind, pending = false): LandingNode {
  return pending ? { kind, label: kind, pending: true } : { kind, label: kind }
}

function unit(name: string, kind: LandingNodeKind, patch: Partial<LandingUnit> = {}): LandingUnit {
  return { name, branchCount: 1, commitCount: 1, node: node(kind), fixes: [], ...patch }
}

function entry(branchName: string, patch: Partial<RobotOverviewEntry> = {}, robot = testRobot(`${branchName}-robot`, branchName)): RobotOverviewEntry {
  return { branchName, hasRow: true, robot, chip: { kind: "running", color: "neutral", label: "Running", commit: "current", outdated: false }, ...patch }
}

function robots(patch: Partial<RobotOverview>): RobotOverview {
  return { ...EMPTY_ROBOT_OVERVIEW, ...patch }
}

describe("collectLandingOverview", () => {
  it("counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first", () => {
    const units = [
      unit("docs", "ready"),
      unit("air", "ready", { branchCount: 22, commitCount: 40 }),
      unit("auth", "yourStep"),
      unit("session", "needsFix"),
      unit("login", "waiting"),
      unit("old", "inMaster"),
    ]
    const overview = collectLandingOverview(units, EMPTY_ROBOT_OVERVIEW)
    expect(overview.moves).toBe(4)
    expect(overview.yourMove.map(it => it.name)).toEqual(["session", "auth", "docs", "air"])
    expect(overview.waiting.map(it => it.name)).toEqual(["login"])
    expect(overview.inMaster.map(it => it.name)).toEqual(["old"])
    expect(overview.urgent).toBe("needsFix")
    expect(formatLandingTitle(overview)).toBe("On the way to master: 4 your move · 1 waiting · 1 in master")
  })

  it("lists a stack whose branch needs a fix with the moves, the fix counted as a move of its own", () => {
    const air = unit("air", "waiting", { branchCount: 5, fixes: [{ name: "bazel", node: node("needsFix") }] })
    const overview = collectLandingOverview([air], EMPTY_ROBOT_OVERVIEW)
    expect(overview.yourMove).toEqual([air])
    expect(overview.waiting).toEqual([])
    expect(overview.moves).toBe(1)
    expect(overview.urgent).toBe("needsFix")
  })

  it("counts no move of a unit before the first answer of Patronus, so the number doesn't change on hover", () => {
    const overview = collectLandingOverview([unit("auth", "waiting", { node: node("waiting", true) }), unit("docs", "ready")], EMPTY_ROBOT_OVERVIEW)
    expect(overview.moves).toBe(1)
    expect(overview.checking).toBe(1)
  })

  it("counts a failed robot of no row and a running Safe Push that will fail as moves, listed with them; a Dry Run lands nothing", () => {
    const failing = entry("auth", { chip: { kind: "running", color: "error", label: "Failing", commit: "current", outdated: false } })
    const dryRun = entry("docs", {}, testRobot("dry", "docs", { pushMode: "dryRun" }))
    const watched = entry("#101 Rework the session store", { hasRow: false, chip: { kind: "failed", color: "error", label: "Failed", commit: "current", outdated: false } })
    const overview = collectLandingOverview([unit("auth", "landing"), unit("docs", "waiting")], robots({ running: [failing, dryRun], attention: [watched] }))
    expect(overview.moves).toBe(2)
    expect(overview.failedRobots).toEqual([watched])
    // no use waiting for a failing one: it is a move, not landing
    expect(overview.failingRobots).toEqual([failing])
    expect(overview.running).toEqual([dryRun])
    expect(overview.landing).toBe(0)
    expect(overview.safePushRuns).toBe(true)
    expect(overview.urgent).toBe("needsFix")
    expect(formatLandingSectionTitle(overview)).toBe("Landing · 1 dry run")
    expect(formatLandingSectionTitle({ running: [dryRun, dryRun, failing], landing: 1 })).toBe("Landing · 1 · 2 dry runs")
    expect(formatLandingSectionTitle({ running: [failing], landing: 1 })).toBe("Landing · 1")
    // only a Dry Run runs: no ring
    expect(collectLandingOverview([], robots({ running: [dryRun] })).safePushRuns).toBe(false)
  })

  it("leaves out of the last day the robots of what is in master: the In master row says it", () => {
    const merged = entry("old", { chip: { kind: "merged", color: "info", label: "Merged", commit: "current", outdated: false } })
    const other = entry("docs", { chip: { kind: "canceled", color: "neutral", label: "Canceled", commit: "current", outdated: false } })
    const overview = collectLandingOverview([unit("old", "inMaster")], robots({ finished: [merged, other] }))
    expect(overview.finished).toEqual([other])
  })

  it("is empty with nothing on the way, nothing in master and no robot of the last day", () => {
    expect(isLandingOverviewEmpty(collectLandingOverview([], EMPTY_ROBOT_OVERVIEW))).toBe(true)
    expect(formatLandingTitle(collectLandingOverview([], EMPTY_ROBOT_OVERVIEW))).toBe("Nothing on the way to master")
    expect(isLandingOverviewEmpty(collectLandingOverview([unit("old", "inMaster")], EMPTY_ROBOT_OVERVIEW))).toBe(false)
  })
})

describe("formatFinishedSummary", () => {
  const finishedAs = (name: string, kind: RobotOverviewEntry["chip"]["kind"], patch: Partial<RobotOverviewEntry> = {}) =>
    entry(name, { chip: { kind, color: "neutral", label: kind, commit: "current", outdated: false }, ...patch })

  it("names what merged and what failed, counts the canceled and the passed Dry Runs, nothing without a robot", () => {
    const finished = [
      finishedAs("auth", "merged"),
      finishedAs("login", "merged"),
      finishedAs("oauth", "merged"),
      finishedAs("Whole air stack", "merged", { stackRoot: "air" }),
      finishedAs("cache", "merged"),
      finishedAs("ui", "failed"),
      finishedAs("api", "needsReview"),
      finishedAs("docs", "canceled"),
      finishedAs("tools", "passed"),
      finishedAs("misc", "unknown"),
    ]
    expect(formatFinishedSummary(finished)).toBe("auth, login, oauth and 2 more merged · ui and api failed · 1 canceled · 1 dry run passed in the last 24 hours")
    expect(formatFinishedSummary([finishedAs("Whole air stack", "merged", { stackRoot: "air" })])).toBe("the whole air stack merged in the last 24 hours")
    expect(formatFinishedSummary([finishedAs("a", "passed"), finishedAs("b", "passed")])).toBe("2 dry runs passed in the last 24 hours")
    expect(formatFinishedSummary([])).toBeNull()
  })
})

describe("getLandingBarSegments", () => {
  it("has a segment per kind on the way, landing first and the user's moves last, its weight the commits", () => {
    const units = [
      unit("docs", "ready", { commitCount: 2 }),
      unit("auth", "waiting", { commitCount: 3 }),
      unit("air", "landing", { commitCount: 5 }),
      unit("login", "waiting", { commitCount: 0 }),
      unit("old", "inMaster", { commitCount: 9 }),
      unit("side", "waiting", { fixes: [{ name: "side-fix", node: node("needsFix") }] }),
    ]
    expect(getLandingBarSegments(units)).toEqual([
      { kind: "landing", weight: 5 },
      { kind: "waiting", weight: 4 },
      { kind: "ready", weight: 2 },
      { kind: "needsFix", weight: 1 },
    ])
  })
})
