import type { PatronusRobot, RobotCommitMap, RobotCommitState } from "@/lib/bindings"
import { getReviewCommitState } from "@/lib/review-commit"
import type { RemoteStatus } from "@/stores/branch-sync"

/**
 * Which commits of a branch the running robot that tests it covers: `all` while it took the branch as it is (or the
 * same commits on an older baseline), else the state of each commit by its original hash, from the commit map
 * (`get_robot_commit_map`). A commit the map does not name is not known.
 */
export type RobotCoverage = "all" | ReadonlyMap<string, RobotCommitState>

/** Whether the robot covers every commit of the branches it tests, needs the commit map, or nothing is known yet */
export type RobotCoverageNeed = "all" | "map" | null

/**
 * What the commit map is needed for: the robot took the branch it tests (its own, or the stack branch) at another tip
 * than the one the branch has now, and the branch changed since by content (`getReviewCommitState`). A robot without
 * a source revision, or a branch whose remote status is not loaded yet, tells nothing.
 */
export function robotCoverageNeed(robot: PatronusRobot, tip: string | null | undefined, remote: RemoteStatus | null): RobotCoverageNeed {
  if (robot.sourceRevision === null || tip == null || tip === "") {
    return null
  }
  if (robot.sourceRevision === tip) {
    return "all"
  }
  if (remote === null) {
    return null
  }
  const state = getReviewCommitState(robot.sourceRevision, tip, remote)
  return state === "rebased" || state === "current" ? "all" : "map"
}

/** The states of the commits of `branchName` in the commit map, by their original hash */
export function branchCoverageOf(map: RobotCommitMap, branchName: string): ReadonlyMap<string, RobotCommitState> {
  const states = new Map<string, RobotCommitState>()
  if (map.sourceMissing) {
    return states
  }
  for (const commit of map.commits) {
    if (commit.localId !== null && commit.branch === branchName) {
      states.set(commit.localId, commit.state)
    }
  }
  return states
}

/** How a commit row draws the robot on the rail of its branch (`bd-lane-rail`) */
export interface RobotRailMark {
  /**
   * `tip`: the newest commit the robot took, below a commit it lacks, a dot in the colour of the robot; `uncovered`: a
   * commit added or changed since the robot started, a hollow `warning` dot; null: the plain dot
   */
  readonly dot: "tip" | "uncovered" | null
  /** Why the robot lacks the commit: the row says it in its marker; null for a commit it took or one not known */
  readonly change: "added" | "changed" | null
  /** The line of the robot from the dot up to the top of the rail: the first row, which the robot covers */
  readonly up: boolean
  /** The line of the robot from the dot down to the dot of the next row: both rows covered */
  readonly down: boolean
}

function isCovered(coverage: RobotCoverage, hash: string): boolean {
  if (coverage === "all") {
    return true
  }
  const state = coverage.get(hash)
  return state === "tested" || state === "rebased"
}

function changeOf(coverage: RobotCoverage, hash: string): "added" | "changed" | null {
  if (coverage === "all") {
    return null
  }
  const state = coverage.get(hash)
  return state === "added" || state === "changed" ? state : null
}

/**
 * The marks of the rows of a commit list in the order it shows them (`newestFirst`, else oldest first): the line of the
 * robot runs over the commits it covers, in its colour as the landing span, and stops at the newest one; the commits
 * added or changed since it started have hollow `warning` dots on the plain line. Rows whose state is not known keep
 * the plain dot.
 */
export function robotRailMarks(hashes: readonly string[], coverage: RobotCoverage, newestFirst: boolean): ReadonlyMap<string, RobotRailMark> {
  const marks = new Map<string, RobotRailMark>()
  hashes.forEach((hash, index) => {
    const covered = isCovered(coverage, hash)
    const change = changeOf(coverage, hash)
    const newer = hashes[newestFirst ? index - 1 : index + 1]
    const tip = covered && newer !== undefined && changeOf(coverage, newer) !== null
    const next = hashes[index + 1]
    marks.set(hash, {
      dot: tip ? "tip" : change === null ? null : "uncovered",
      change,
      up: covered && index === 0,
      down: covered && next !== undefined && isCovered(coverage, next),
    })
  })
  return marks
}

/**
 * The data attributes of a row that draw the running robot on the rail of the branch (`bd-lane-rail` in index.css):
 * the dot, and the line of the robot up to the top of the rail or down to the next dot
 */
export function robotRailAttributes(mark: RobotRailMark | undefined): Record<`data-${string}`, string> {
  if (mark === undefined) {
    return {}
  }
  const line = mark.up && mark.down ? "both" : mark.up ? "up" : mark.down ? "down" : null
  return { ...(mark.dot === null ? {} : { "data-robot-dot": mark.dot }), ...(line === null ? {} : { "data-robot-line": line }) }
}
