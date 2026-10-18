import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { findBranchRobot, findStackRobot, getPatronusChipState, isFailingChip, isRobotActive, needsRobotAttention } from "@/lib/patronus"
import type { RobotTiming } from "@/lib/robot-eta"
import { formatWholeStackStart } from "@/lib/whole-stack"
import type { Branch, StackBranch } from "@/stores/branch-sync"

/** The landing overview lists the robots that finished this long before now */
export const FINISHED_ROBOTS_MS = 24 * 60 * 60_000

export interface RobotOverviewEntry {
  /**
   * The virtual branch of the robot; for a Safe Push of a temporary ref, the branch whose tip it took; "Whole air stack"
   * for the robot of the whole stack (`stackRoot`)
   */
  readonly branchName: string
  /** The branch has a row to show (it may be archived since the robot merged it); the root's for the whole stack */
  readonly hasRow: boolean
  /** The root of the stack whose whole stack the robot lands (on its stack branch): a click shows the root's row */
  readonly stackRoot?: string
  readonly robot: PatronusRobot
  readonly chip: PatronusChipState
}

/** A running robot that will fail: a required check failed (`RobotFlags.failing`), its chip says "Failing" */
export function isFailingEntry(entry: RobotOverviewEntry): boolean {
  return isFailingChip(entry.chip)
}

export interface RobotOverview {
  /** The active robots of the branches (running or canceling), in the table order; the failing ones among them */
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
 * The robots of the repository's branches for the header (`LandingIndicator`) and the Dock: the running ones, the ones
 * that finished in the last day, and the branches whose robot failed on the branch as it is. Only the robots already
 * listed: the overview asks Patronus nothing, and only the robots of branches, of the whole stacks and the watched ones
 * are polled, so a running robot of no branch is left out unless the app watches it (`watched`: a Safe Push of a
 * reviewed pull request, listed by what it tests, after the branches). The robot of the whole stack of a stack branch
 * (`stacks`, by root) comes after the branches, as "Whole air stack", on its stack branch as it is. `order` is the table
 * order, `needsReview` the robots that failed for missing approvals, `failing` the running ones that will fail.
 */
export function collectRobotOverview(
  order: readonly string[],
  branches: Readonly<Record<string, Branch>>,
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
  needsReview: Readonly<Record<string, boolean>>,
  now: number,
  watched: Readonly<Record<string, string>> = {},
  failing: Readonly<Record<string, true>> = {},
  stacks: Readonly<Record<string, StackBranch>> = {},
): RobotOverview {
  if (robots === undefined) {
    return EMPTY_ROBOT_OVERVIEW
  }
  const flagsOf = (id: string) => ({ needsReview: needsReview[id] === true, failing: failing[id] === true })
  const entry = (branchName: string, robot: PatronusRobot): RobotOverviewEntry => {
    const branch = branches[branchName]
    return {
      branchName,
      hasRow: branch !== undefined,
      robot,
      chip: getPatronusChipState(robot, branch?.commits[0]?.hash, branch?.remoteStatus ?? null, flagsOf(robot.id)),
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
    } else if (needsRobotAttention(robot, localTip, remote)) {
      attention.push(entry(name, robot))
    }
  }

  const since = (now - FINISHED_ROBOTS_MS) / 1000
  // the whole stack: its robot on its stack branch as it is, a click shows the root
  const stackFinished: RobotOverviewEntry[] = []
  for (const stack of Object.values(stacks)) {
    const robot = findStackRobot(robots, stack.root, stack.tip, stack.remoteStatus?.remoteTip ?? null)
    if (robot === undefined) {
      continue
    }
    const tip = stack.tip ?? undefined
    const stackEntry: RobotOverviewEntry = {
      branchName: formatWholeStackStart(stack.root),
      hasRow: branches[stack.root] !== undefined,
      stackRoot: stack.root,
      robot,
      chip: getPatronusChipState(robot, tip, stack.remoteStatus, flagsOf(robot.id)),
    }
    if (isRobotActive(robot)) {
      running.push(stackEntry)
    } else if (needsRobotAttention(robot, tip, stack.remoteStatus)) {
      attention.push(stackEntry)
    } else if (robot.finishTime !== null && robot.finishTime >= since) {
      stackFinished.push(stackEntry)
    }
  }
  const watchedFinished: RobotOverviewEntry[] = []
  for (const [id, label] of Object.entries(watched)) {
    const robot = robots[id]
    if (robot === undefined) {
      continue
    }
    // no branch: the robot itself is what the user acts on
    const watchedEntry: RobotOverviewEntry = { branchName: label, hasRow: false, robot, chip: getPatronusChipState(robot, robot.sourceRevision ?? undefined, null, flagsOf(id)) }
    if (isRobotActive(robot)) {
      running.push(watchedEntry)
    } else if (robot.status === "failure") {
      attention.push(watchedEntry)
    } else if (robot.finishTime !== null && robot.finishTime >= since) {
      watchedFinished.push(watchedEntry)
    }
  }

  // a branch is listed once: its running robot, or the one it has to act on
  const listed = new Set([...running, ...attention].map(it => it.branchName))
  const newest = new Map<string, PatronusRobot>()
  for (const robot of Object.values(robots)) {
    // the robot of the whole stack is listed by its stack above
    if (isRobotActive(robot) || robot.finishTime === null || robot.finishTime < since || robot.stackRoot != null) {
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
  const finished = [...[...newest].map(([branchName, robot]) => entry(branchName, robot)), ...stackFinished, ...watchedFinished]
  finished.sort((a, b) => (b.robot.finishTime ?? 0) - (a.robot.finishTime ?? 0))
  return { running, attention, finished }
}

/** What the Dock (the taskbar on Windows) shows of the running robots (`setProgressBar`) */
export type DockProgress = { readonly status: "none" } | { readonly status: "indeterminate" } | { readonly status: "normal"; readonly progress: number }

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
