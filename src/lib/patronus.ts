import { openUrl } from "@tauri-apps/plugin-opener"
import type { PatronusAvailability, PatronusPushMode, PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { UIColor } from "@/lib/ui-types"

/** What the chip shows of a robot */
export type PatronusChipKind = "running" | "failed" | "merged" | "passed" | "canceled" | "unknown"

export interface PatronusChipState {
  kind: PatronusChipKind
  color: Extract<UIColor, "success" | "error" | "info" | "neutral">
  /** "Running", "Canceling", "Failed", "Merged", "Dry run passed", "Canceled" */
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
 * local or the pushed tip of the branch. A running robot wins, then the newest.
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

function isPreferred(robot: PatronusRobot, other: PatronusRobot): boolean {
  const active = isRobotActive(robot)
  if (active !== isRobotActive(other)) {
    return active
  }
  return robot.startTime > other.startTime
}

/** How the chip shows a robot. `localTip` is the commit of the local virtual branch, undefined while unknown. */
export function getPatronusChipState(robot: PatronusRobot, localTip: string | undefined): PatronusChipState {
  const described = describeStatus(robot)
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

/** A failed robot of the commit the branch is at: the user has to act */
export function needsPatronusAttention(robot: PatronusRobot, localTip: string | undefined): boolean {
  return robot.status === "failure" && robot.sourceRevision !== null && robot.sourceRevision === localTip
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
