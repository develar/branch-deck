import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { findBranchRobot, getPatronusChipState, isRobotActive, needsRobotAttention } from "@/lib/patronus"
import type { RobotTiming } from "@/lib/robot-eta"
import type { Branch } from "@/stores/branch-sync"

/** The robots indicator lists the robots that finished this long before now */
export const FINISHED_ROBOTS_MS = 24 * 60 * 60_000

export interface RobotOverviewEntry {
  /** The virtual branch of the robot; for a Safe Push of a temporary ref, the branch whose tip it took */
  readonly branchName: string
  /** The branch has a row to show (it may be archived since the robot merged it) */
  readonly hasRow: boolean
  readonly robot: PatronusRobot
  readonly chip: PatronusChipState
}

export interface RobotOverview {
  /** The active robots of the branches (running or canceling), in the table order */
  readonly running: readonly RobotOverviewEntry[]
  /**
   * The robots that failed on the branch as it is (`needsRobotAttention`), also for missing code-owner approvals
   * ("Needs review"), however long ago, in the table order: the user has to act
   */
  readonly attention: readonly RobotOverviewEntry[]
  /** The other robots that finished in the last day, the newest of each branch, newest first */
  readonly finished: readonly RobotOverviewEntry[]
}

export const EMPTY_ROBOT_OVERVIEW: RobotOverview = { running: [], attention: [], finished: [] }

/**
 * The robots of the repository's branches for the header (`RobotsIndicator`) and the Dock: the running ones, the ones
 * that finished in the last day, and the branches whose robot failed on the branch as it is. Only the robots already
 * listed: the overview asks Patronus nothing, and only the robots of branches are polled, so a running robot of no
 * branch is left out. `order` is the table order, `needsReview` the robots that failed for missing approvals.
 */
export function collectRobotOverview(
  order: readonly string[],
  branches: Readonly<Record<string, Branch>>,
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
  needsReview: Readonly<Record<string, boolean>>,
  now: number,
): RobotOverview {
  if (robots === undefined) {
    return EMPTY_ROBOT_OVERVIEW
  }
  const entry = (branchName: string, robot: PatronusRobot): RobotOverviewEntry => {
    const branch = branches[branchName]
    return {
      branchName,
      hasRow: branch !== undefined,
      robot,
      chip: getPatronusChipState(robot, branch?.commits[0]?.hash, branch?.remoteStatus ?? null, needsReview[robot.id] === true),
    }
  }

  const running: RobotOverviewEntry[] = []
  const attention: RobotOverviewEntry[] = []
  // the branch of a robot of a temporary ref is the one whose tip it took
  const branchOfRevision = new Map<string, string>()
  for (const name of order) {
    const branch = branches[name]
    const localTip = branch?.commits[0]?.hash
    const remote = branch?.remoteStatus ?? null
    for (const tip of [localTip, remote?.remoteTip]) {
      if (tip != null && tip !== "" && !branchOfRevision.has(tip)) {
        branchOfRevision.set(tip, name)
      }
    }
    const robot = findBranchRobot(robots, name, localTip, remote?.remoteTip ?? null)
    if (robot === undefined) {
      continue
    }
    if (isRobotActive(robot)) {
      running.push(entry(name, robot))
    }
    else if (needsRobotAttention(robot, localTip, remote)) {
      attention.push(entry(name, robot))
    }
  }

  // a branch is listed once: its running robot, or the one it has to act on
  const listed = new Set([...running, ...attention].map(it => it.branchName))
  const since = (now - FINISHED_ROBOTS_MS) / 1000
  const newest = new Map<string, PatronusRobot>()
  for (const robot of Object.values(robots)) {
    if (isRobotActive(robot) || robot.finishTime === null || robot.finishTime < since) {
      continue
    }
    const branchName = robot.branchName ?? (robot.sourceRevision === null ? undefined : branchOfRevision.get(robot.sourceRevision))
    if (branchName === undefined || listed.has(branchName)) {
      continue
    }
    const known = newest.get(branchName)
    if (known === undefined || (known.finishTime ?? 0) < robot.finishTime) {
      newest.set(branchName, robot)
    }
  }
  const finished = [...newest].map(([branchName, robot]) => entry(branchName, robot))
  finished.sort((a, b) => (b.robot.finishTime ?? 0) - (a.robot.finishTime ?? 0))
  return { running, attention, finished }
}

/** "2 running", "1 failed", "1 needs review": the parts of the indicator's text that apply, in this order */
export interface RobotIndicatorPart {
  readonly kind: "running" | "failed" | "needsReview"
  readonly text: string
}

export function getRobotIndicatorParts(overview: RobotOverview): RobotIndicatorPart[] {
  const parts: RobotIndicatorPart[] = []
  if (overview.running.length > 0) {
    parts.push({ kind: "running", text: `${overview.running.length} running` })
  }
  const reviews = overview.attention.filter(it => it.chip.kind === "needsReview").length
  const failed = overview.attention.length - reviews
  if (failed > 0) {
    parts.push({ kind: "failed", text: `${failed} failed` })
  }
  if (reviews > 0) {
    parts.push({ kind: "needsReview", text: `${reviews} ${reviews === 1 ? "needs" : "need"} review` })
  }
  return parts
}

/** The accessible name of the indicator: "Patronus robots: 2 running, 1 failed" */
export function formatRobotIndicatorLabel(parts: readonly RobotIndicatorPart[]): string {
  return `Patronus robots: ${parts.map(part => part.text).join(", ")}`
}

/** What the Dock (the taskbar on Windows) shows of the running robots (`setProgressBar`) */
export type DockProgress = { readonly status: "none" } | { readonly status: "indeterminate" } | { readonly status: "normal", readonly progress: number }

/**
 * The progress of the running robot nearest to finishing, 0 to 100; indeterminate when no running robot has an
 * estimate left (no history, or past its usual time); none while nothing runs
 */
export function getDockProgress(timings: readonly RobotTiming[]): DockProgress {
  if (timings.length === 0) {
    return { status: "none" }
  }
  let nearest: RobotTiming | null = null
  for (const timing of timings) {
    if (timing.remaining !== null && (nearest === null || timing.remaining < nearest.remaining!)) {
      nearest = timing
    }
  }
  return nearest === null ? { status: "indeterminate" } : { status: "normal", progress: Math.round(nearest.progress! * 100) }
}

/** The branches the sync left with a conflict, which the user resolves: they count in the badge of the Dock */
export function countConflicts(branches: Readonly<Record<string, Branch>>): number {
  return Object.values(branches).filter(branch => branch.status === "MergeConflict" || branch.status === "AnalyzingConflict").length
}
