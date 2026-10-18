import { openUrl } from "@tauri-apps/plugin-opener"
import type { PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusReviewerGroup, PatronusRobot, PatronusStartOptions, PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { OutgoingState, PushAction } from "@/lib/push-summary"
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
  /** "Running", "Failing", "Canceling", "Failed", "Needs review", "Merged", "Dry run passed", "Canceled" */
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
 * one, then the newest; of equals the first in the record.
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
  const index = robotIndexOf(robots)
  let best = index.byBranch.get(branchName)
  for (const tip of [localTip, remoteTip]) {
    const candidate = tip == null ? undefined : index.byRevision.get(tip)
    if (candidate !== undefined && (best === undefined || isPreferredRanked(candidate, best))) {
      best = candidate
    }
  }
  return best?.robot
}

interface RankedRobot {
  readonly robot: PatronusRobot
  /** The place in the record, which decides between equals */
  readonly order: number
}

/** The preferred robot per branch name, and per source revision of the robots without a branch */
interface RobotIndex {
  readonly byBranch: ReadonlyMap<string, RankedRobot>
  readonly byRevision: ReadonlyMap<string, RankedRobot>
}

/**
 * Built once per robots record: every row asks for its robot several times on every store change, and a scan of all
 * robots each time made that O(branches × robots)
 */
const robotIndexes = new WeakMap<Readonly<Record<string, PatronusRobot>>, RobotIndex>()

function robotIndexOf(robots: Readonly<Record<string, PatronusRobot>>): RobotIndex {
  let index = robotIndexes.get(robots)
  if (index === undefined) {
    const byBranch = new Map<string, RankedRobot>()
    const byRevision = new Map<string, RankedRobot>()
    Object.values(robots).forEach((robot, order) => {
      const key = robot.branchName ?? robot.sourceRevision
      if (key === null) {
        return
      }
      const map = robot.branchName === null ? byRevision : byBranch
      const current = map.get(key)
      const ranked = { robot, order }
      if (current === undefined || isPreferredRanked(ranked, current)) {
        map.set(key, ranked)
      }
    })
    index = { byBranch, byRevision }
    robotIndexes.set(robots, index)
  }
  return index
}

function isPreferredRanked(a: RankedRobot, b: RankedRobot): boolean {
  if (isPreferred(a.robot, b.robot)) {
    return true
  }
  return !isPreferred(b.robot, a.robot) && a.order < b.order
}

/** Running, then canceling (it goes away; its restart runs), then finished; then the newest */
function activity(robot: PatronusRobot): number {
  return robot.status === "running" ? 2 : robot.status === "canceling" ? 1 : 0
}

function isPreferred(robot: PatronusRobot, other: PatronusRobot): boolean {
  const difference = activity(robot) - activity(other)
  return difference === 0 ? robot.startTime > other.startTime : difference > 0
}

/** What the app learned of a robot beyond its status */
export interface RobotFlags {
  /** The robot failed for missing code-owner approvals */
  readonly needsReview?: boolean
  /** The running robot will fail: a required check failed, Patronus warns */
  readonly failing?: boolean
}

/**
 * How the chip shows a robot. `localTip` is the commit of the local virtual branch, undefined while unknown; `remote` its
 * remote status, null until loaded. A running robot that will fail is "Failing" in `error`, still a running one (its
 * ring, its time, Cancel).
 */
export function getPatronusChipState(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null, flags: RobotFlags = {}): PatronusChipState {
  const described =
    robot.status === "failure" && flags.needsReview === true
      ? { kind: "needsReview" as const, color: "warning" as const, label: "Needs review" }
      : robot.status === "running" && flags.failing === true
        ? { kind: "running" as const, color: "error" as const, label: "Failing" }
        : describeStatus(robot)
  // a merged robot is done: the sync archives the branch once the baseline has it
  const commit = described.kind === "merged" ? "current" : getReviewCommitState(robot.sourceRevision, localTip, remote)
  return { ...described, commit, outdated: isReviewCommitOutdated(commit) }
}

/** The chip of a running robot that will fail ("Failing") */
export function isFailingChip(chip: Pick<PatronusChipState, "kind" | "color">): boolean {
  return chip.kind === "running" && chip.color === "error"
}

/** The colour of the ring of a running robot: primary, `destructive` for one that will fail ("Failing") */
export function robotRingColor(failing: boolean): string {
  return failing ? "text-destructive" : "text-primary"
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
      return robot.pushMode === "dryRun" ? { kind: "passed", color: "success", label: "Dry run passed" } : { kind: "merged", color: "info", label: "Merged" }
    case "canceled":
      return { kind: "canceled", color: "neutral", label: "Canceled" }
    case "unknown":
      return { kind: "unknown", color: "neutral", label: "Unknown status" }
  }
}

/**
 * A failed robot (also for missing approvals) of the branch as it is: the user has to act, the robots indicator and the
 * Dock badge count it. The branch as it is: the robot took the local tip, or the same commits before a sync rebuilt them
 * on a newer baseline. A branch changed since (a fix) needs nothing.
 */
export function needsRobotAttention(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null): boolean {
  const onBranch = robot.sourceRevision !== null && (robot.sourceRevision === localTip || getReviewCommitState(robot.sourceRevision, localTip, remote) === "rebased")
  return robot.status === "failure" && onBranch
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
export function findMergedRobots(previous: Readonly<Record<string, PatronusRobot>>, next: Readonly<Record<string, PatronusRobot>>): PatronusRobot[] {
  return findFinishedRobots(previous, next, robot => robot.status === "success" && isPushingRobot(robot))
}

/** The robots known as running or canceling in `previous` that finished as `finished` says in `next` */
function findFinishedRobots(
  previous: Readonly<Record<string, PatronusRobot>>,
  next: Readonly<Record<string, PatronusRobot>>,
  finished: (robot: PatronusRobot) => boolean,
): PatronusRobot[] {
  return Object.values(next).filter(robot => {
    const known = previous[robot.id]
    return known !== undefined && isRobotActive(known) && finished(robot)
  })
}

/**
 * The robots that failed between two answers: known as running or canceling before, failed now. A robot first seen
 * already finished is no news.
 */
export function findFailedRobots(previous: Readonly<Record<string, PatronusRobot>>, next: Readonly<Record<string, PatronusRobot>>): PatronusRobot[] {
  return findFinishedRobots(previous, next, robot => robot.status === "failure")
}

/**
 * "IJPL-1 merged into master": the virtual branch, or the subject of a Safe Push of a temporary ref; `label` names a
 * watched robot of no branch ("#101 Rework the session store")
 */
export function formatMergedRobotTitle(robot: PatronusRobot, label?: string): string {
  return `${label ?? robot.branchName ?? robot.name} merged into ${robot.targetBranch}`
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
  openRobotUrl(robot.url)
}

/** Opens the page of a robot by its URL (a toast that crossed windows has no robot) */
export function openRobotUrl(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the Patronus robot", error))
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

/**
 * A running robot of the branch tests a commit the branch moved past: its restart cancels it, pushes and restarts it
 * on the pushed commit, the one way to push into a branch whose Safe Push runs (`PushHold`)
 */
function canPushAndRestartRunning(robot: PatronusRobot, branchName: string, needsReview: boolean): boolean {
  return robot.status === "running" && isRobotOnBranch(robot, branchName) && !(needsReview && robot.pullRequest == null)
}

/** The restart the details offer: a restart, one that pushes first, or none */
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
    // after the push, the remote branch is at the local tip; a running robot is canceled first (`pushBranch`)
    const restartable = canRestartRobot(robot, branchName, localTip ?? null, needsReview) || canPushAndRestartRunning(robot, branchName, needsReview)
    return localTip !== undefined && localTip !== "" && restartable ? "pushAndRestart" : null
  }
  return canRestartRobot(robot, branchName, remote?.remoteTip ?? null, needsReview) ? "restart" : null
}

/**
 * "Restart Safe Push", "Restart Dry Run": the one name of a restart, in the branch menu, the robot details, the robots
 * indicator and the advice of the details. A restart that pushes first has the same name: the push is part of it (the
 * hover says it, and that it cancels a running robot)
 */
export function formatRobotRestartLabel(robot: PatronusRobot): string {
  return `Restart ${formatPushMode(robot.pushMode)}`
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
    return `${changed} ${formatRobotRestartLabel(robot)} to test the current branch.`
  }
  // canceling, a robot started on the branch can be restarted; one of a temporary ref can't
  if (canCancelRobot(robot) && robot.branchName !== null) {
    return `${changed} Cancel ${formatPushMode(robot.pushMode)}, then ${formatRobotRestartLabel(robot)} to test the current branch.`
  }
  return changed
}

/**
 * A robot that holds the pushes of the branches it lands (`PushHold`): running and pushing into the target. A dry run
 * lands nothing, and a canceling robot won't merge: a push next to either is fine.
 */
export function isHoldingRobot(robot: PatronusRobot): boolean {
  return robot.status === "running" && robot.pushMode !== "dryRun"
}

/**
 * Why a branch offers no push: a Safe Push runs on it (`own`, the robot of the branch) or on a branch built on it that
 * lands it (`span`, `findLandingDescendants`). A push now won't land: the robot merges the commit it took, and a push
 * into its own branch makes it fail at the end ("branch was modified"). The restart, which pushes first, is the way through.
 */
export interface PushHold {
  kind: "own" | "span"
  /** The branch of the robot: the branch itself for `own` */
  branch: string
  robot: PatronusRobot
}

/**
 * The hold of a branch: its own robot started on the branch (`ownRobot`, `findBranchRobot`) before a span of a branch built on it (`span`,
 * `findLandingDescendants`); null when neither holds (`isHoldingRobot`)
 */
export function findPushHold(branchName: string, ownRobot: PatronusRobot | undefined, span: LandingDescendant | null | undefined): PushHold | null {
  // a Safe Push of a temporary ref (the IDE, the CLI) is not on the branch: a push leaves it as it is
  if (ownRobot !== undefined && isHoldingRobot(ownRobot) && isRobotOnBranch(ownRobot, branchName)) {
    return { kind: "own", branch: branchName, robot: ownRobot }
  }
  if (span != null && isHoldingRobot(span.robot)) {
    return { kind: "span", branch: span.branch, robot: span.robot }
  }
  return null
}

/**
 * Why a held branch offers no push: "A Safe Push of auth is running: wait until it merges, or restart it on the current
 * branch", "A Safe Push of oauth is running: it lands auth with it"
 */
export function formatPushHoldReason(branchName: string, hold: PushHold): string {
  return hold.kind === "own"
    ? `A ${formatPushMode(hold.robot.pushMode)} of ${branchName} is running: wait until it merges, or restart it on the current branch`
    : formatLandingReason(branchName, hold)
}

/**
 * A start would repeat a running robot: one runs on the commit the branch is at (Patronus would refuse the start), or on
 * the remote branch while it has the same commits by content, only rebuilt since on a newer baseline (`rebased`, as
 * the remote status compares): it tests and lands the branch as it is
 */
export function isStartBlocked(robot: PatronusRobot | undefined, localTip: string | undefined, remote: RemoteStatus | null): boolean {
  if (robot === undefined || !isRobotActive(robot) || robot.sourceRevision === null) {
    return false
  }
  return robot.sourceRevision === localTip || getReviewCommitState(robot.sourceRevision, localTip, remote) === "rebased"
}

/** What a start pushes before the robot runs (`getStartPush`) */
export type StartPush = { readonly kind: "unknown" } | { readonly kind: "none" } | { readonly kind: "stack" } | { readonly kind: "branch"; readonly action: OutgoingState }

/**
 * What a Safe Push or a Dry Run pushes before the robot starts from the remote branch, by content as the push button
 * decides it (`pushAction`, `getPushAction`): nothing when the remote branch has the commits of the branch, also when
 * the sync only rebuilt them on a newer baseline (Patronus rebases the remote branch itself; a push would replace it with
 * copies of the same commits and move the head of its pull request for nothing); the stack while its ancestors have work
 * for the remote (`pushesStack`, `stackPushOrder`); `unknown` while the remote status of the branch is not loaded: the
 * start waits for it.
 */
export function getStartPush(pushAction: PushAction | null, pushesStack: boolean): StartPush {
  if (pushAction === null) {
    return { kind: "unknown" }
  }
  if (pushesStack) {
    return { kind: "stack" }
  }
  return pushAction === "upToDate" ? { kind: "none" } : { kind: "branch", action: pushAction }
}

/** Whether a Safe Push or a Dry Run of a branch can start; `reason` says why not, for the hover of a disabled action */
export type RobotStartCheck = { ok: true } | { ok: false; reason: string }

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
  if (robot !== undefined && isStartBlocked(robot, branch.commits[0]?.hash, branch.remoteStatus)) {
    return { ok: false, reason: `A ${formatPushMode(robot.pushMode)} of the last commit is running` }
  }
  // a robot that lands the branch itself says so: it lands its ancestors too (`findLandingAncestor` sees it there)
  if (landingDescendant !== null) {
    return { ok: false, reason: formatLandingReason(branch.name, landingDescendant) }
  }
  if (landingAncestor !== null) {
    return { ok: false, reason: `A ${formatPushMode(landingAncestor.robot.pushMode)} of ${landingAncestor.branch} is running: wait until it merges` }
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
export function findLandingAncestor(
  ancestors: readonly string[],
  robotOf: (name: string) => PatronusRobot | undefined,
  spanOf: (name: string) => LandingDescendant | undefined = () => undefined,
): LandingAncestor | null {
  for (const name of [...ancestors].reverse()) {
    const robot = robotOf(name)
    if (robot !== undefined && isRobotActive(robot) && robot.pushMode !== "dryRun") {
      return { branch: name, robot }
    }
    // a robot of a side branch built on the ancestor lands it too: the branch would land its commits a second time
    const span = spanOf(name)
    if (span !== undefined) {
      return span
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
 * The robot of a branch row (`useRowRobot`):
 * - `start`: none to show, "Safe Push…" starts one; also for a robot of an earlier state of the branch (the details of
 *   that robot stay in the preview and in the outgoing overview);
 * - `running`: the live robot, its time and how long it usually takes;
 * - `finished`: the state of a robot of the branch as it is, in its hue; `offered`: a next step can be offered
 *   (Patronus answered, the branch can be pushed);
 * - `none`: nothing to show or to start.
 */
export type SafePushSlot =
  | { readonly kind: "start" }
  | { readonly kind: "running"; readonly chip: PatronusChipState }
  | { readonly kind: "finished"; readonly chip: PatronusChipState; readonly offered: boolean }
  | { readonly kind: "none" }

export interface SafePushSlotInput {
  readonly robot: PatronusRobot | undefined
  readonly localTip: string | undefined
  readonly remote: RemoteStatus | null
  readonly needsReview: boolean
  /** The running robot will fail */
  readonly failing: boolean
  /** "Safe Push…" is offered: Patronus answered and the branch can be pushed */
  readonly canOffer: boolean
}

/** The robot of a branch row (`NextStepSlot`, `BranchRowStatus`) */
export function getSafePushSlot({ robot, localTip, remote, needsReview, failing, canOffer }: SafePushSlotInput): SafePushSlot {
  if (robot === undefined) {
    return canOffer ? { kind: "start" } : { kind: "none" }
  }
  const chip = getPatronusChipState(robot, localTip, remote, { needsReview, failing })
  if (isRobotActive(robot)) {
    return { kind: "running", chip }
  }
  // a merged robot has no warning dot, yet a branch with other commits since (the same name, new work) starts anew
  if (canOffer && isReviewCommitOutdated(getReviewCommitState(robot.sourceRevision, localTip, remote))) {
    return { kind: "start" }
  }
  return { kind: "finished", chip, offered: canOffer }
}

/**
 * The robot the Status cell of a branch row shows: a running one (also of an earlier state of the branch, with its
 * warning dot), a finished one of the branch as it is; null for the git status
 */
export function getRowRobotChip(slot: SafePushSlot): PatronusChipState | null {
  return slot.kind === "running" || slot.kind === "finished" ? slot.chip : null
}

/** The text of "Safe Push…" where it can start: what the form does before the robot runs */
export const SAFE_PUSH_DESCRIPTION = "Checks the code-owner review, then starts the robot."

/** The profile allows the mode; no modes listed = any */
export function allowsPushMode(profile: PatronusProfile, mode: PatronusPushMode): boolean {
  return profile.allowedPushModes.length === 0 || profile.allowedPushModes.includes(mode)
}

/**
 * The profiles a Safe Push without review offers, by what Patronus says of them before a start: those known to skip the
 * code-owner review (the profile of the last robot, its options said it), the others, which Patronus doesn't describe
 * (a start with one that checks the review fails at once), and the emergency ones (they skip all checks). A profile
 * known to check the review is left out. The default: the profile of the last robot if it skips the review, else none
 * (the user chooses)
 */
export interface BypassProfiles {
  skipping: PatronusProfile[]
  other: PatronusProfile[]
  emergency: PatronusProfile[]
  defaultProfileId: string | null
}

export function getBypassProfiles(options: PatronusStartOptions, mode: PatronusPushMode): BypassProfiles {
  const offered = options.profiles.filter(profile => allowsPushMode(profile, mode))
  const skipping = offered.filter(profile => profile.skipsReview === true)
  return {
    skipping,
    other: offered.filter(profile => profile.skipsReview === null),
    emergency: options.emergencyProfiles.filter(profile => allowsPushMode(profile, mode)),
    defaultProfileId: skipping.some(profile => profile.id === options.defaultProfileId) ? options.defaultProfileId : null,
  }
}

/**
 * "Patronus starts it from pull request #760" or the Space alternative: a branch with an open pull request in the GitHub
 * repository the branches are mirrored to starts from it (a `/patronus` comment), a draft only for a dry run. null =
 * the start goes through the Patronus API: also for a pull request into another base than the target of Patronus
 * (`isPullRequestIntoTarget`), the pull request of a stacked branch into its parent's branch.
 */
export function getPullRequestStart(
  githubRepository: string | null,
  pullRequest: PullRequest | undefined,
  mode: PatronusPushMode,
  targetBranch: string | null,
): PullRequestStart | null {
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

/** "3 files", "1 file", "all files" for a group that reviews every change; null when nobody tells which */
export function formatReviewerGroupFiles(group: PatronusReviewerGroup): string | null {
  if (group.branchWide) {
    return "all files"
  }
  if (group.pathsSource === "unknown" || group.paths.length === 0) {
    return null
  }
  return group.paths.length === 1 ? "1 file" : `${group.paths.length} files`
}
