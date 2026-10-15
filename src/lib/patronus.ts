import { openUrl } from "@tauri-apps/plugin-opener"
import type { PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusReviewerGroup, PatronusRobot, PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { ReviewCommitState } from "@/lib/review-commit"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { formatBlockedReason } from "@/lib/stack"
import type { UIColor } from "@/lib/ui-types"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"

/** What the chip shows of a robot */
export type PatronusChipKind = "running" | "failed" | "needsReview" | "merged" | "passed" | "canceled" | "unknown"

/** The subsystem of the problem "Changes require review approval": the code owners did not approve */
export const REVIEW_APPROVAL_SUBSYSTEM = "OWNERSHIP_REVIEW_APPROVAL_CHECK"

export interface PatronusChipState {
  kind: PatronusChipKind
  color: Extract<UIColor, "success" | "error" | "info" | "warning" | "neutral">
  /** "Running", "Canceling", "Failed", "Needs review", "Merged", "Dry run passed", "Canceled" */
  label: string
  /** How the commit the robot took relates to the branch now; `current` for a merged robot */
  commit: ReviewCommitState
  /** The branch changed since the robot took its commit (not only rebuilt on a newer baseline): a `warning` dot */
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
 * How the chip shows a robot. `localTip` is the commit of the local virtual branch, undefined while unknown; `remote` its
 * remote status, null until loaded; `needsReview`: the robot failed for missing code-owner approvals.
 */
export function getPatronusChipState(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null, needsReview = false): PatronusChipState {
  const described = robot.status === "failure" && needsReview
    ? { kind: "needsReview" as const, color: "warning" as const, label: "Needs review" }
    : describeStatus(robot)
  // a merged robot is done: the sync archives the branch once the baseline has it
  const commit = described.kind === "merged" ? "current" : getReviewCommitState(robot.sourceRevision, localTip, remote)
  return { ...described, commit, outdated: isReviewCommitOutdated(commit) }
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

/**
 * A failed robot of the branch as it is, the user has to act: "failed Safe Push"; null for any other robot. The branch as
 * it is: the robot took the local tip, or the same commits before a sync rebuilt them on a newer baseline.
 */
export function formatPatronusAttention(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null): string | null {
  const onBranch = robot.sourceRevision !== null
    && (robot.sourceRevision === localTip || getReviewCommitState(robot.sourceRevision, localTip, remote) === "rebased")
  return robot.status === "failure" && onBranch ? `failed ${formatPushMode(robot.pushMode)}` : null
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
 * What a started, restarted or canceling robot tests, as its toast and Activity say it: "IJPL-1 → master · profile
 * Large Scale Change". The virtual branch (the subject for a robot of a temporary ref), never the ref the robot runs on.
 */
export function describeRobot(robot: PatronusRobot, profileName: string | null = null): string {
  const target = `${robot.branchName ?? robot.name} → ${robot.targetBranch}`
  return profileName === null ? target : `${target} · profile ${profileName}`
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
 * one that failed for missing approvals in a Space merge request. A robot of a pull request that failed for them is:
 * the restart counts the approvals the pull request has by then.
 */
export function canRestartRobot(robot: PatronusRobot, branchName: string, remoteTip: string | null, needsReview = false): boolean {
  // a restart keeps the merge request of the robot, which has no approvals: a new Safe Push with one is the way
  if (!isRobotOnBranch(robot, branchName) || (needsReview && robot.pullRequest == null)) {
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

/** The restart the details offer: Restart, Push and Restart, or none */
export type RobotRestartAction = "restart" | "pushAndRestart" | null

/**
 * A restart takes the newest commit of the remote branch: a branch with work the remote lacks is pushed first ("Push and
 * Restart"), so the robot tests the branch as it is; a plain restart would test the commit the remote has.
 */
export function getRobotRestartAction(
  robot: PatronusRobot,
  branchName: string,
  localTip: string | undefined,
  remote: RemoteStatus | null,
  needsReview = false,
): RobotRestartAction {
  if (getReviewCommitState(robot.sourceRevision, localTip, remote) === "changedLocally") {
    // after the push, the remote branch is at the local tip
    return localTip !== undefined && localTip !== "" && canRestartRobot(robot, branchName, localTip, needsReview) ? "pushAndRestart" : null
  }
  return canRestartRobot(robot, branchName, remote?.remoteTip ?? null, needsReview) ? "restart" : null
}

const restartAdvice: Readonly<Record<NonNullable<RobotRestartAction>, string>> = {
  restart: "Restart to test the current branch.",
  pushAndRestart: "Push and restart to test the current branch.",
}

/**
 * What the details say about the commit the robot took, null when it took the branch as it is. A branch only rebuilt on
 * a newer baseline gets a quiet line; a changed one, the commits and the action the details offer for it (for a running
 * robot of the branch, the cancel first).
 */
export function formatRobotCommitNotice(robot: PatronusRobot, chip: Pick<PatronusChipState, "commit">, localTip: string | undefined, action: RobotRestartAction): string | null {
  if (chip.commit === "current" || robot.sourceRevision === null) {
    return null
  }
  if (chip.commit === "rebased") {
    return "Started on an earlier build of the same commits."
  }
  const now = localTip === undefined || localTip === "" ? "" : ` (now ${formatShortHash(localTip)})`
  const changed = `Started on ${formatShortHash(robot.sourceRevision)}; the branch has changed since${now}: the result won't cover the newest commits.`
  if (action !== null) {
    return `${changed} ${restartAdvice[action]}`
  }
  // canceling, a robot started on the branch can be restarted; one of a temporary ref can't
  if (canCancelRobot(robot) && robot.branchName !== null) {
    return `${changed} Cancel, then ${chip.commit === "changedLocally" ? "push and restart" : "restart"} to test the current branch.`
  }
  return changed
}

/** A push into the branch makes the running robot fail at the end ("branch was modified"); a dry run is not affected */
export function isPushIntoRobotRisky(robot: PatronusRobot, branchName: string): boolean {
  return isRobotActive(robot) && robot.pushMode !== "dryRun" && isRobotOnBranch(robot, branchName)
}

/** A start would be refused: a robot is running on the commit the branch is at */
export function isStartBlocked(robot: PatronusRobot | undefined, localTip: string | undefined): boolean {
  return robot !== undefined && isRobotActive(robot) && robot.sourceRevision !== null && robot.sourceRevision === localTip
}

/** Whether a Safe Push or a Dry Run of a branch can start; `reason` says why not, for the hover of a disabled action */
export type RobotStartCheck = { ok: true } | { ok: false, reason: string }

/**
 * Whether a Safe Push or a Dry Run of the branch can start: not while a robot runs on the commit the branch is at
 * (Patronus would refuse it), nor for a blocked stacked branch (its build lacks its parent's state). `robot` is the robot
 * of the branch (`getBranchRobot`, `useBranchRobot`). A Safe Push also waits for a robot that lands an ancestor of the
 * branch (`landingAncestor`, `findLandingAncestor`): the branch carries the ancestor's commits, two robots would land
 * them; and for a robot of a branch built on it that lands the branch (`landingDescendant`, `findLandingDescendants`).
 * Every entry point asks it: the row slot, the branch menu, the preview footer and the robot details.
 */
export function canStartRobot(
  branch: Branch,
  robot: PatronusRobot | undefined,
  landingAncestor: LandingAncestor | null = null,
  landingDescendant: LandingDescendant | null = null,
): RobotStartCheck {
  if (branch.status === "Blocked") {
    return { ok: false, reason: formatBlockedReason(branch) }
  }
  if (robot !== undefined && isStartBlocked(robot, branch.commits[0]?.hash)) {
    return { ok: false, reason: `A ${formatPushMode(robot.pushMode)} of the last commit is running` }
  }
  if (landingAncestor !== null) {
    return { ok: false, reason: `A ${formatPushMode(landingAncestor.robot.pushMode)} of ${landingAncestor.branch} is running: wait until it merges` }
  }
  if (landingDescendant !== null) {
    return { ok: false, reason: formatLandingReason(branch.name, landingDescendant) }
  }
  return { ok: true }
}

/** An ancestor of a stacked branch whose robot pushes into the target: it lands the ancestor's commits */
export interface LandingAncestor {
  branch: string
  robot: PatronusRobot
}

/**
 * The nearest of `ancestors` (root first, `stackAncestors`) whose robot (`robotOf`) is active and pushes into the
 * target (not a dry run); null when none is
 */
export function findLandingAncestor(ancestors: readonly string[], robotOf: (name: string) => PatronusRobot | undefined): LandingAncestor | null {
  for (const name of [...ancestors].reverse()) {
    const robot = robotOf(name)
    if (robot !== undefined && isRobotActive(robot) && robot.pushMode !== "dryRun") {
      return { branch: name, robot }
    }
  }
  return null
}

/** A branch built on others whose robot pushes into the target: it lands the commits of the branches it is built on */
export interface LandingDescendant {
  branch: string
  robot: PatronusRobot
}

/**
 * The branches a running robot lands, the inverse of `findLandingAncestor`: for each branch of `order` (the table order)
 * whose robot is active and pushes into the target (not a dry run), its ancestors (`ancestorsOf`). An ancestor two such
 * robots land is landed by the first of `order`.
 */
export function findLandingDescendants(
  order: readonly string[],
  ancestorsOf: (name: string) => readonly string[],
  robotOf: (name: string) => PatronusRobot | undefined,
): Map<string, LandingDescendant> {
  const landed = new Map<string, LandingDescendant>()
  for (const name of order) {
    const robot = robotOf(name)
    if (robot === undefined || !isRobotActive(robot) || robot.pushMode === "dryRun") {
      continue
    }
    for (const ancestor of ancestorsOf(name)) {
      if (!landed.has(ancestor)) {
        landed.set(ancestor, { branch: name, robot })
      }
    }
  }
  return landed
}

/** Why a branch can't start while a robot of a branch built on it runs: "A Safe Push of oauth is running: it lands auth with it" */
export function formatLandingReason(branchName: string, landing: LandingDescendant): string {
  return `A ${formatPushMode(landing.robot.pushMode)} of ${landing.branch} is running: it lands ${branchName} with it`
}

/**
 * What the Safe Push slot of a branch row shows:
 * - `start`: "Safe Push…", without a robot and for a robot of an earlier state of the branch (the details of that robot
 *   stay in the preview and in the outgoing overview);
 * - `running`: the live robot, its time and how long it usually takes;
 * - `finished`: the state of a robot of the branch as it is, in its hue; `restart`: the row hover swaps the state text
 *   for "Safe Push…", as a start is possible and useful (not after a merge);
 * - `none`: nothing to show or to start.
 */
export type SafePushSlot
  = | { readonly kind: "start" }
    | { readonly kind: "running", readonly chip: PatronusChipState }
    | { readonly kind: "finished", readonly chip: PatronusChipState, readonly restart: boolean }
    | { readonly kind: "none" }

export interface SafePushSlotInput {
  readonly robot: PatronusRobot | undefined
  readonly localTip: string | undefined
  readonly remote: RemoteStatus | null
  readonly needsReview: boolean
  /** "Safe Push…" is offered: Patronus answered and the branch can be pushed */
  readonly canOffer: boolean
  /** `canStartRobot` allows a start */
  readonly canStart: boolean
}

/** The state of the Safe Push slot (`SafePushSlot`) */
export function getSafePushSlot({ robot, localTip, remote, needsReview, canOffer, canStart }: SafePushSlotInput): SafePushSlot {
  if (robot === undefined) {
    return canOffer ? { kind: "start" } : { kind: "none" }
  }
  const chip = getPatronusChipState(robot, localTip, remote, needsReview)
  if (isRobotActive(robot)) {
    return { kind: "running", chip }
  }
  // a merged robot has no warning dot, yet a branch with other commits since (the same name, new work) starts anew
  if (canOffer && isReviewCommitOutdated(getReviewCommitState(robot.sourceRevision, localTip, remote))) {
    return { kind: "start" }
  }
  return { kind: "finished", chip, restart: canOffer && canStart && chip.kind !== "merged" }
}

/** The text of "Safe Push…" where it can start: what the form does before the robot runs */
export const SAFE_PUSH_DESCRIPTION = "Checks the code-owner review, then starts the robot."

/** The profile allows the mode; no modes listed = any */
export function allowsPushMode(profile: PatronusProfile, mode: PatronusPushMode): boolean {
  return profile.allowedPushModes.length === 0 || profile.allowedPushModes.includes(mode)
}

/**
 * "Patronus starts it from pull request #760" or the Space alternative: a branch with an open pull request in the GitHub
 * repository the branches are mirrored to starts from it (a `/patronus` comment), a draft only for a dry run. null =
 * the start goes through the Patronus API: also for a pull request into another base than the target of Patronus
 * (`isPullRequestIntoTarget`), the pull request of a stacked branch into its parent's branch.
 */
export function getPullRequestStart(githubRepository: string | null, pullRequest: PullRequest | undefined, mode: PatronusPushMode, targetBranch: string | null): PullRequestStart | null {
  if (githubRepository === null || pullRequest === undefined || pullRequest.state !== "open" || !isPullRequestIntoTarget(pullRequest, targetBranch)) {
    return null
  }
  return { number: pullRequest.number, blockedByDraft: pullRequest.isDraft && mode !== "dryRun", command: mode === "dryRun" ? "/patronus dry-run" : "/patronus merge" }
}

/**
 * Whether a pull request merges into the target branch of Patronus; true while either is unknown (no base read, the
 * target not loaded yet)
 */
export function isPullRequestIntoTarget(pullRequest: PullRequest, targetBranch: string | null): boolean {
  return targetBranch === null || !pullRequest.baseRefName || pullRequest.baseRefName === targetBranch
}

/** A start from a GitHub pull request (`getPullRequestStart`) */
export interface PullRequestStart {
  readonly number: number
  /** Patronus doesn't merge a draft pull request */
  readonly blockedByDraft: boolean
  /** The comment, as the form says it */
  readonly command: string
}

/** "Air Plugin Release Team (suggested: Alice, Bob)" */
export function formatReviewerGroup(group: PatronusReviewerGroup): string {
  const suggested = group.reviewers.filter(reviewer => reviewer.suggested && reviewer.available).map(reviewer => reviewer.fullName)
  return suggested.length === 0 ? group.name : `${group.name} (suggested: ${suggested.join(", ")})`
}
