import { openUrl } from "@tauri-apps/plugin-opener"
import type { PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusReviewerGroup, PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { UIColor } from "@/lib/ui-types"

/** What the chip shows of a robot */
export type PatronusChipKind = "running" | "failed" | "needsReview" | "merged" | "passed" | "canceled" | "unknown"

/** The subsystem of the problem "Changes require review approval": the code owners did not approve */
export const REVIEW_APPROVAL_SUBSYSTEM = "OWNERSHIP_REVIEW_APPROVAL_CHECK"

export interface PatronusChipState {
  kind: PatronusChipKind
  color: Extract<UIColor, "success" | "error" | "info" | "warning" | "neutral">
  /** "Running", "Canceling", "Failed", "Needs review", "Merged", "Dry run passed", "Canceled" */
  label: string
  /** The robot tests another commit than the local branch tip: a `warning` dot */
  outdated: boolean
}

/** Still running, a cancel included: the app polls its status */
export function isRobotActive(robot: PatronusRobot): boolean {
  return robot.status === "running" || robot.status === "canceling"
}

/**
 * The robot of a virtual branch: one started on the branch, or a Safe Push of a temporary ref (the IDE, the CLI) on the
 * local or the pushed tip of the branch. A running robot wins over a canceling one (a restart replaces it) over a finished
 * one, then the newest.
 */
export function findBranchRobot(
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
  branchName: string,
  localTip: string | undefined,
  remoteTip: string | null,
): PatronusRobot | undefined {
  if (robots === undefined) {
    return undefined
  }
  let best: PatronusRobot | undefined
  for (const robot of Object.values(robots)) {
    const onBranch = robot.branchName === null
      ? robot.sourceRevision !== null && (robot.sourceRevision === localTip || robot.sourceRevision === remoteTip)
      : robot.branchName === branchName
    if (onBranch && (best === undefined || isPreferred(robot, best))) {
      best = robot
    }
  }
  return best
}

/** Running, then canceling (it goes away; its restart runs), then finished; then the newest */
function activity(robot: PatronusRobot): number {
  return robot.status === "running" ? 2 : robot.status === "canceling" ? 1 : 0
}

function isPreferred(robot: PatronusRobot, other: PatronusRobot): boolean {
  const difference = activity(robot) - activity(other)
  return difference === 0 ? robot.startTime > other.startTime : difference > 0
}

/**
 * How the chip shows a robot. `localTip` is the commit of the local virtual branch, undefined while unknown;
 * `needsReview`: the robot failed for missing code-owner approvals.
 */
export function getPatronusChipState(robot: PatronusRobot, localTip: string | undefined, needsReview = false): PatronusChipState {
  const described = robot.status === "failure" && needsReview
    ? { kind: "needsReview" as const, color: "warning" as const, label: "Needs review" }
    : describeStatus(robot)
  return {
    ...described,
    // a merged robot is done: the sync archives the branch once the baseline has it
    outdated: described.kind !== "merged" && isRobotOutdated(robot, localTip),
  }
}

function describeStatus(robot: PatronusRobot): Pick<PatronusChipState, "kind" | "color" | "label"> {
  switch (robot.status) {
    case "running":
      return { kind: "running", color: "neutral", label: "Running" }
    case "canceling":
      return { kind: "running", color: "neutral", label: "Canceling" }
    case "failure":
      return { kind: "failed", color: "error", label: "Failed" }
    case "success":
      return robot.pushMode === "dryRun"
        ? { kind: "passed", color: "success", label: "Dry run passed" }
        : { kind: "merged", color: "info", label: "Merged" }
    case "canceled":
      return { kind: "canceled", color: "neutral", label: "Canceled" }
    case "unknown":
      return { kind: "unknown", color: "neutral", label: "Unknown status" }
  }
}

/** The robot took another commit than the local tip: the branch changed since it started */
export function isRobotOutdated(robot: PatronusRobot, localTip: string | undefined): boolean {
  return robot.sourceRevision !== null && localTip !== undefined && localTip !== "" && robot.sourceRevision !== localTip
}

/** A failed robot of the commit the branch is at, the user has to act: "failed Safe Push"; null for any other robot */
export function formatPatronusAttention(robot: PatronusRobot, localTip: string | undefined): string | null {
  const failedOnTip = robot.status === "failure" && robot.sourceRevision !== null && robot.sourceRevision === localTip
  return failedOnTip ? `failed ${formatPushMode(robot.pushMode)}` : null
}

const pushModeNames: Readonly<Record<PatronusPushMode, string>> = {
  dryRun: "Dry Run",
  rebase: "Safe Push (rebase)",
  rebaseAutosquash: "Safe Push",
  squashAll: "Safe Push (squash)",
  merge: "Safe Merge",
  unknown: "Robot",
}

export function formatPushMode(mode: PatronusPushMode): string {
  return pushModeNames[mode]
}

/** Not a dry run: a success pushes the changes into the target branch */
export function isPushingRobot(robot: PatronusRobot): boolean {
  return robot.pushMode !== "dryRun"
}

/**
 * The robots that merged between two answers: known as running or canceling before, successful now, and not a dry run.
 * A robot first seen already finished is no news.
 */
export function findMergedRobots(
  previous: Readonly<Record<string, PatronusRobot>>,
  next: Readonly<Record<string, PatronusRobot>>,
): PatronusRobot[] {
  return Object.values(next).filter((robot) => {
    const known = previous[robot.id]
    return known !== undefined && isRobotActive(known) && robot.status === "success" && isPushingRobot(robot)
  })
}

/** "IJPL-1 merged into master": the virtual branch, or the subject of a Safe Push of a temporary ref */
export function formatMergedRobotTitle(robot: PatronusRobot): string {
  return `${robot.branchName ?? robot.name} merged into ${robot.targetBranch}`
}

/**
 * A quiet hint why robots are not shown, null when they are (or the repository is not on Space, which needs no hint).
 * Never a toast: Patronus status is an extra.
 */
export function formatPatronusAvailabilityHint(availability: PatronusAvailability | null): string | null {
  switch (availability?.status) {
    case "notLoggedIn":
      return "Run ./safePush.cmd once to log in to Patronus"
    case "error":
      return `Patronus: ${availability.message}`
    default:
      return null
  }
}

/** Opens the robot page of the Patronus web UI */
export function openRobot(robot: PatronusRobot): void {
  openUrl(robot.url).catch((error: unknown) => notifyError("Cannot open the Patronus robot", error))
}

/** A robot started on this very branch (not on a temporary ref): Patronus can restart it on the newest commit */
export function isRobotOnBranch(robot: PatronusRobot, branchName: string): boolean {
  return robot.branchName === branchName
}

/**
 * Patronus restarts a failed or canceled robot of a branch on its newest commit, a canceling one too; a successful dry
 * run only when the branch has a newer commit on the remote. A merged robot or a running one is never restarted, nor
 * one that failed for missing approvals.
 */
export function canRestartRobot(robot: PatronusRobot, branchName: string, remoteTip: string | null, needsReview = false): boolean {
  // a restart keeps the merge request of the robot, which has no approvals: a new Safe Push with one is the way
  if (!isRobotOnBranch(robot, branchName) || needsReview) {
    return false
  }
  switch (robot.status) {
    case "failure":
    case "canceled":
    case "canceling":
      return true
    case "success":
      return robot.pushMode === "dryRun" && remoteTip !== null && remoteTip !== robot.sourceRevision
    case "running":
    case "unknown":
      return false
  }
}

export function canCancelRobot(robot: PatronusRobot): boolean {
  return robot.status === "running"
}

/** A push into the branch makes the running robot fail at the end ("branch was modified"); a dry run is not affected */
export function isPushIntoRobotRisky(robot: PatronusRobot, branchName: string): boolean {
  return isRobotActive(robot) && robot.pushMode !== "dryRun" && isRobotOnBranch(robot, branchName)
}

/** A start would be refused: a robot is running on the commit the branch is at */
export function isStartBlocked(robot: PatronusRobot | undefined, localTip: string | undefined): boolean {
  return robot !== undefined && isRobotActive(robot) && robot.sourceRevision !== null && robot.sourceRevision === localTip
}

/** The profile allows the mode; no modes listed = any */
export function allowsPushMode(profile: PatronusProfile, mode: PatronusPushMode): boolean {
  return profile.allowedPushModes.length === 0 || profile.allowedPushModes.includes(mode)
}

/** "Air Plugin Release Team (suggested: Alice, Bob)" */
export function formatReviewerGroup(group: PatronusReviewerGroup): string {
  const suggested = group.reviewers.filter(reviewer => reviewer.suggested && reviewer.available).map(reviewer => reviewer.fullName)
  return suggested.length === 0 ? group.name : `${group.name} (suggested: ${suggested.join(", ")})`
}
