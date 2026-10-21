import { openUrl } from "@tauri-apps/plugin-opener"
import type {
  PatronusAvailability,
  PatronusProfile,
  PatronusPushMode,
  PatronusReviewerGroup,
  PatronusRobot,
  PatronusStartOptions,
  PullRequest,
  RobotLanding,
  RobotLandingRequest,
  RobotSpan,
} from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { shellWord } from "@/lib/prompt-text"
import type { OutgoingState, PushAction } from "@/lib/push-summary"
import type { ReviewCommitState } from "@/lib/review-commit"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { formatBlockedReason, stackAncestors } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import type { UIColor } from "@/lib/ui-types"
import { formatWholeStack, formatWholeStackStart } from "@/lib/whole-stack"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"

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

/**
 * What a robot lands: a virtual branch (`branchName`), or the whole stack of a root (`stackRoot`, its stack branch
 * `{prefix}/stack/<root>`); `robotSubjectOf` is null for a robot of a temporary ref (the IDE, the CLI)
 */
export type RobotSubject = { readonly kind: "branch"; readonly name: string } | { readonly kind: "stack"; readonly root: string }

/** The subject of a robot started on a virtual branch or on a stack branch; null for one of a temporary ref */
export function robotSubjectOf(robot: Pick<PatronusRobot, "branchName" | "stackRoot">): RobotSubject | null {
  if (robot.branchName !== null) {
    return { kind: "branch", name: robot.branchName }
  }
  return robot.stackRoot == null ? null : { kind: "stack", root: robot.stackRoot }
}

/**
 * How a sentence names what a robot tests: "auth", "the whole air stack", else `label` (a watched robot of no branch,
 * "#101 Rework the session store") or the robot's name (the subject of a Safe Push of a temporary ref)
 */
export function formatRobotSubject(robot: Pick<PatronusRobot, "branchName" | "stackRoot" | "name">, label?: string): string {
  if (label !== undefined) {
    return label
  }
  const subject = robotSubjectOf(robot)
  if (subject === null) {
    return robot.name
  }
  return subject.kind === "branch" ? subject.name : formatWholeStack(subject.root)
}

/** `formatRobotSubject` at the start of a sentence: "Whole air stack merged into master" */
export function formatRobotSubjectStart(robot: Pick<PatronusRobot, "branchName" | "stackRoot" | "name">, label?: string): string {
  return label === undefined && robot.branchName === null && robot.stackRoot != null ? formatWholeStackStart(robot.stackRoot) : formatRobotSubject(robot, label)
}

/** Still running, a cancel included: the app polls its status */
export function isRobotActive(robot: PatronusRobot): boolean {
  return robot.status === "running" || robot.status === "canceling"
}

/**
 * The robot of a virtual branch: one started on the branch, or a Safe Push of a temporary ref (the IDE, the CLI) on the
 * local or the pushed tip of the branch. A robot of the whole stack (`stackRoot`, on its stack branch) is no branch's,
 * whatever its revision. A running robot wins over a canceling one (a restart replaces it) over a finished one, then the
 * newest; of equals the first in the record.
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
  return preferredOnTips(index, index.byBranch.get(branchName), [localTip, remoteTip])?.robot
}

/** `own`, or a robot of a temporary ref on one of the tips that is preferred over it (`isPreferredRanked`) */
function preferredOnTips(index: RobotIndex, own: RankedRobot | undefined, tips: readonly (string | null | undefined)[]): RankedRobot | undefined {
  let best = own
  for (const tip of tips) {
    const candidate = tip == null ? undefined : index.byRevision.get(tip)
    if (candidate !== undefined && (best === undefined || isPreferredRanked(candidate, best))) {
      best = candidate
    }
  }
  return best
}

/**
 * The robot of the whole stack of `root`: one started on its stack branch (`stackRoot`), or a Safe Push of a temporary
 * ref on the local or the pushed tip of the stack branch (`find_stack_robot` of `bd`); the same preference as
 * `findBranchRobot`
 */
export function findStackRobot(
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
  root: string,
  localTip: string | null | undefined,
  remoteTip: string | null,
): PatronusRobot | undefined {
  if (robots === undefined) {
    return undefined
  }
  const index = robotIndexOf(robots)
  return preferredOnTips(index, index.byStack.get(root), [localTip, remoteTip])?.robot
}

interface RankedRobot {
  readonly robot: PatronusRobot
  /** The place in the record, which decides between equals */
  readonly order: number
}

/**
 * The preferred robot per branch name, per root of the robots of the whole stack, and per source revision of the robots
 * of a temporary ref
 */
interface RobotIndex {
  readonly byBranch: ReadonlyMap<string, RankedRobot>
  readonly byStack: ReadonlyMap<string, RankedRobot>
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
    const byStack = new Map<string, RankedRobot>()
    const byRevision = new Map<string, RankedRobot>()
    Object.values(robots).forEach((robot, order) => {
      // the whole stack's robot runs on its stack branch: never a branch's, not even by its revision
      const [map, key] = robot.branchName !== null ? [byBranch, robot.branchName] : robot.stackRoot != null ? [byStack, robot.stackRoot] : [byRevision, robot.sourceRevision]
      if (key === null) {
        return
      }
      const current = map.get(key)
      const ranked = { robot, order }
      if (current === undefined || isPreferredRanked(ranked, current)) {
        map.set(key, ranked)
      }
    })
    index = { byBranch, byStack, byRevision }
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
 * A failed robot (also for missing approvals) of the branch as it is: the user has to act, the landing overview (a move)
 * and the Dock badge count it. The branch as it is: the robot took the local tip, or the same commits before a sync rebuilt them
 * on a newer baseline. A branch changed since (a fix) needs nothing.
 */
export function needsRobotAttention(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null): boolean {
  const onBranch = robot.sourceRevision !== null && (robot.sourceRevision === localTip || getReviewCommitState(robot.sourceRevision, localTip, remote) === "rebased")
  return robot.status === "failure" && onBranch
}

/**
 * A failed robot of the branch as it is (`needsRobotAttention`) that nothing else carries: once a running Safe Push of a
 * branch built on it or of the whole stack holds the branch, a merged one landed it whole (`PushHold`), the baseline
 * has all of it, or a running Safe Push of a branch it is built on lands a part of it (`carried`), its row waits (`isSettled` of the next step), and so the landing overview, the island,
 * Attention and the Dock badge ask nothing for its old failure.
 */
export function needsBranchRobotAttention(robot: PatronusRobot, localTip: string | undefined, remote: RemoteStatus | null, carried: boolean): boolean {
  return !carried && needsRobotAttention(robot, localTip, remote)
}

/**
 * A failed robot of the whole stack on its stack branch as it is (`needsRobotAttention`), only while the sync built it:
 * a stack branch not rebuilt (kept for its remote branch once the stack no longer forks, blocked, a conflict) is not the
 * stack's commits, its restart is refused (`getStackBranchBlocker`) and the stack lands in pieces, so its old failure
 * asks for nothing; nor while a running Safe Push of a branch of it lands a part of it (`carried`, `findLandingMember`):
 * its restart waits for that robot.
 */
export function needsStackRobotAttention(robot: PatronusRobot, stack: Pick<StackBranch, "state" | "tip" | "remoteStatus">, carried = false): boolean {
  return !carried && stack.state === "Built" && needsRobotAttention(robot, stack.tip ?? undefined, stack.remoteStatus)
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
 * "IJPL-1 merged into master", "Whole air stack merged into master": the virtual branch, the whole stack, or the
 * subject of a Safe Push of a temporary ref; `label` names a watched robot of no branch ("#101 Rework the session store")
 */
export function formatMergedRobotTitle(robot: PatronusRobot, label?: string): string {
  return `${formatRobotSubjectStart(robot, label)} merged into ${robot.targetBranch}`
}

/**
 * What a started, restarted or canceling robot tests, as its toast and Activity say it: "IJPL-1 → master · profile
 * Large Scale Change", "Whole air stack → master". The virtual branch or the whole stack (the subject for a robot of a
 * temporary ref), never the ref the robot runs on.
 */
export function describeRobot(robot: PatronusRobot, profileName: string | null = null): string {
  const target = `${formatRobotSubjectStart(robot)} → ${robot.targetBranch}`
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

/**
 * The command the hint "Run ./safePush.cmd once to log in to Patronus" names, ready to paste in a terminal: into the
 * repository, then the Safe Push CLI, which logs in when it runs (PowerShell on Windows)
 */
export function formatSafePushLoginCommand(repositoryPath: string, powershell: boolean): string {
  const cd = `cd ${shellWord(repositoryPath, powershell)}`
  return powershell ? `${cd}; .\\safePush.cmd` : `${cd} && ./safePush.cmd`
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
 * A robot started on the branch or on the stack branch of the whole stack `on` names (a branch name is a branch), not on
 * a temporary ref: Patronus can restart it on the newest commit of that branch
 */
export function isRobotOn(robot: PatronusRobot, on: string | RobotSubject): boolean {
  if (typeof on === "string") {
    return isRobotOnBranch(robot, on)
  }
  return on.kind === "branch" ? isRobotOnBranch(robot, on.name) : robot.branchName === null && robot.stackRoot === on.root
}

/**
 * Patronus restarts a failed or canceled robot of a branch on its newest commit, a canceling one too; a successful dry
 * run only when the branch has a newer commit on the remote. A merged robot or a running one is never restarted, nor
 * one that failed for missing approvals in a Space merge request. A robot of a pull request that failed for them is:
 * the restart counts the approvals the pull request has by then.
 */
export function canRestartRobot(robot: PatronusRobot, on: string | RobotSubject, remoteTip: string | null, needsReview = false): boolean {
  // a restart keeps the merge request of the robot, which has no approvals: a new Safe Push with one is the way
  if (!isRobotOn(robot, on) || (needsReview && robot.pullRequest == null)) {
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
function canPushAndRestartRunning(robot: PatronusRobot, on: string | RobotSubject, needsReview: boolean): boolean {
  return robot.status === "running" && isRobotOn(robot, on) && !(needsReview && robot.pullRequest == null)
}

/** The restart the details offer: a restart, one that pushes first, or none */
export type RobotRestartAction = "restart" | "pushAndRestart" | null

/**
 * A restart takes the newest commit of the remote branch: a branch with work the remote lacks is pushed first ("Push and
 * Restart"), so the robot tests the branch as it is; a plain restart would test the commit the remote has. `on` is the
 * branch, or the whole stack whose stack branch `localTip` and `remote` describe.
 */
export function getRobotRestartAction(
  robot: PatronusRobot,
  on: string | RobotSubject,
  localTip: string | undefined,
  remote: RemoteStatus | null,
  needsReview = false,
): RobotRestartAction {
  if (getReviewCommitState(robot.sourceRevision, localTip, remote) === "changedLocally") {
    // after the push, the remote branch is at the local tip; a running robot is canceled first (`pushBranch`)
    const restartable = canRestartRobot(robot, on, localTip ?? null, needsReview) || canPushAndRestartRunning(robot, on, needsReview)
    return localTip !== undefined && localTip !== "" && restartable ? "pushAndRestart" : null
  }
  return canRestartRobot(robot, on, remote?.remoteTip ?? null, needsReview) ? "restart" : null
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
  // the robot of the whole stack tests the stack branch
  const what = robot.branchName === null && robot.stackRoot != null ? "stack" : "branch"
  const changed = `Started on ${formatShortHash(robot.sourceRevision)}; the ${what} has changed since${now}: the result won't cover the newest commits.`
  if (action !== null) {
    return `${changed} ${formatRobotRestartLabel(robot)} to test the current ${what}.`
  }
  // canceling, a robot started on the branch or the stack branch can be restarted; one of a temporary ref can't
  if (canCancelRobot(robot) && robotSubjectOf(robot) !== null) {
    return `${changed} Cancel ${formatPushMode(robot.pushMode)}, then ${formatRobotRestartLabel(robot)} to test the current ${what}.`
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
 * Or a merged Safe Push landed all of it (`landed`, `findLandedBy`): its own, one of a branch built on it, or of the
 * whole stack. Its commits are in the target, a push sends nothing that lands, until the update archives the branch.
 */
export interface PushHold {
  kind: "own" | "span" | "landed"
  /** The branch of the robot: the branch itself for `own`; the root of the stack for the robot of the whole stack */
  branch: string
  robot: PatronusRobot
  /** The robot of the whole stack of the root `branch` holds every branch of the stack (`LandingDescendant.wholeStack`) */
  wholeStack?: boolean
  /** For `span`: how many commits of the branch the robot lands (`LandingDescendant.commits`) */
  commits?: number
}

/**
 * The hold of a branch: a merged Safe Push that landed it (`landed`, `findLandedBy`), then its own robot started on the
 * branch (`ownRobot`, `findBranchRobot`), then a span of a branch built on it (`span`, `findLandingDescendants`); null
 * when none holds (`isHoldingRobot`)
 */
export function findPushHold(
  branchName: string,
  ownRobot: PatronusRobot | undefined,
  span: LandingDescendant | null | undefined,
  landed: LandedBy | null | undefined = null,
): PushHold | null {
  // its commits are in the target for good: nothing a push sends lands; commits after them are work for the remote
  if (landed != null && landed.after.length === 0) {
    return { kind: "landed", branch: landed.branch, robot: landed.robot, ...(landed.wholeStack === true && { wholeStack: true }) }
  }
  // a Safe Push of a temporary ref (the IDE, the CLI) is not on the branch: a push leaves it as it is
  if (ownRobot !== undefined && isHoldingRobot(ownRobot) && isRobotOnBranch(ownRobot, branchName)) {
    return { kind: "own", branch: branchName, robot: ownRobot }
  }
  if (span != null && isHoldingRobot(span.robot)) {
    return {
      kind: "span",
      branch: span.branch,
      robot: span.robot,
      ...(span.wholeStack === true && { wholeStack: true }),
      ...(span.commits !== undefined && { commits: span.commits }),
    }
  }
  return null
}

/**
 * Why a held branch offers no push: "A Safe Push of auth is running: wait until it merges, or restart it on the current
 * branch", "A Safe Push of oauth is running: it lands auth with it", "Landed in master with oauth: nothing of it is left
 * to push"
 */
export function formatPushHoldReason(branchName: string, hold: PushHold): string {
  if (hold.kind === "landed") {
    return `${formatLandedReason({ ...hold, own: hold.branch === branchName && hold.wholeStack !== true })}: nothing of it is left to push`
  }
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
    return { ok: false, reason: formatLandingWait(landingAncestor) }
  }
  return { ok: true }
}

/** "A Safe Push of auth is running: wait until it merges": a start waits for a robot that lands a part of what it lands */
export function formatLandingWait(landing: LandingAncestor): string {
  return `A ${formatPushMode(landing.robot.pushMode)} of ${formatLandingSubject(landing)} is running: wait until it merges`
}

/** An ancestor of a stacked branch whose robot pushes into the target: it lands the ancestor's commits */
export interface LandingAncestor {
  /** The ancestor; for the robot of the whole stack (`wholeStack`) the root of the stack */
  branch: string
  robot: PatronusRobot
  /** The robot of the whole stack of the root `branch` lands it: it lands every branch of the stack */
  wholeStack?: boolean
}

/** "oauth", "the whole air stack": the branch or the whole stack whose robot lands another one, or landed it */
export function formatLandingSubject(landing: { readonly branch: string; readonly wholeStack?: boolean }): string {
  return landing.wholeStack === true ? formatWholeStack(landing.branch) : landing.branch
}

/**
 * The first of `members` of a stack (stack order) whose own robot (`robotOf`) is active and pushes into the target:
 * the robot of the whole stack waits for it, the stack branch carries the commits it lands (`find_landing_member` of
 * `bd`); null when none runs
 */
export function findLandingMember(members: readonly string[], robotOf: (name: string) => PatronusRobot | undefined): LandingAncestor | null {
  for (const name of members) {
    const robot = robotOf(name)
    if (robot !== undefined && isRobotActive(robot) && robot.pushMode !== "dryRun") {
      return { branch: name, robot }
    }
  }
  return null
}

/**
 * Whether the robot of the whole stack can start or restart: only on a stack branch the sync built
 * (`getStackBranchBlocker`), not while a robot runs on its commit, and, unless `dryRun`, not while the robot of a branch
 * of the stack lands a part of it (`landingMember`, `findLandingMember`): two robots would land the same commits
 */
export function canStartStackRobot(
  blocker: string | null,
  robot: PatronusRobot | undefined,
  tip: string | undefined,
  remote: RemoteStatus | null,
  landingMember: LandingAncestor | null,
  dryRun = false,
): RobotStartCheck {
  if (blocker !== null) {
    return { ok: false, reason: blocker }
  }
  if (robot !== undefined && isStartBlocked(robot, tip, remote)) {
    return { ok: false, reason: `A ${formatPushMode(robot.pushMode)} of the last commit is running` }
  }
  if (!dryRun && landingMember !== null) {
    return { ok: false, reason: formatLandingWait(landingMember) }
  }
  return { ok: true }
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

/**
 * A branch built on others whose robot pushes into the target: it lands the commits of the branches it is built on; or
 * the robot of the whole stack (`wholeStack`, `branch` its root), which lands every branch of the stack
 */
export interface LandingDescendant {
  branch: string
  robot: PatronusRobot
  /** The robot of the whole stack of the root `branch` (its stack branch `{prefix}/stack/<root>`) */
  wholeStack?: boolean
  /**
   * How many commits of the landed branch the robot's commit has (`robotSpanOf`): all of them while it took the branches
   * as they are, else as the content told; undefined until it is told
   */
  commits?: number
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

/**
 * Why a branch can't start while a robot of a branch built on it runs: "A Safe Push of oauth is running: it lands 2
 * commits of auth with it", "A Safe Push of the whole air stack is running: it lands 1 commit of auth with it"; the
 * branch alone while the number is not told yet, as `bd` says it
 */
export function formatLandingReason(branchName: string, landing: LandingDescendant): string {
  const what = landing.commits === undefined ? branchName : `${formatCommitCount(landing.commits)} of ${branchName}`
  return `A ${formatPushMode(landing.robot.pushMode)} of ${formatLandingSubject(landing)} is running: it lands ${what} with it`
}

/**
 * A branch a merged Safe Push landed (`findLandedBy`): its own robot's branch, an ancestor the robot of a branch built on
 * it carried, or a branch of the stack the robot of the whole stack landed. Its commits are in the target until the
 * update archives the branch: its row says who landed it, and while nothing was added after them (`after`) it offers no
 * Safe Push, Dry Run or push.
 */
export interface LandedBy {
  /** The branch of the robot (the branch itself for `own`); for the robot of the whole stack (`wholeStack`) the root */
  readonly branch: string
  readonly robot: PatronusRobot
  /** The robot of the whole stack of the root `branch` landed it, with every branch of the stack */
  readonly wholeStack?: boolean
  /** The robot ran on this branch itself: its row shows the robot ("Merged") */
  readonly own?: boolean
  /**
   * The commits of the branch added after the ones the robot landed (original hashes, newest first): the only work left
   * of it; empty when all of it landed
   */
  readonly after: readonly string[]
}

/**
 * The robot of the branch row (`robot`) is the merged one that landed all of it (`landed`): a rebuild since gave the
 * branch new hashes, not new work, so its row keeps the robot ("Merged") and offers no new start
 */
export function isLandedByOwnRobot(landed: LandedBy | null | undefined, robot: PatronusRobot | undefined): boolean {
  return isLandedWhole(landed) && landed.own === true && robot !== undefined && landed.robot.id === robot.id
}

/** All of the branch landed: nothing was added after the commits the robot took */
export function isLandedWhole(landed: LandedBy | null | undefined): landed is LandedBy {
  return landed != null && landed.after.length === 0
}

/**
 * A merged Safe Push landed the commit `originalHash` of a branch it landed (`landed`, `findLandedBy`): every commit but
 * those added after the landed ones. It is in the target: a push of it lands nothing, so it offers no Push Up to Here
 * and no push arrow, as a commit the remote has.
 */
export function isLandedCommit(landed: LandedBy | null | undefined, originalHash: string): boolean {
  return landed != null && !landed.after.includes(originalHash)
}

/** " · 1 commit after it": what is left of a branch a merged Safe Push landed; empty when nothing is */
function formatAfter(landed: Pick<LandedBy, "after">): string {
  return landed.after.length === 0 ? "" : ` · ${formatCommitCount(landed.after.length)} after it`
}

/**
 * "Landed with oauth", "Landed with the whole air stack", "Landed with oauth · 1 commit after it", of its own robot
 * "Landed · 1 commit after it": the Status cell of a branch a merged Safe Push landed. Null for one its own robot landed
 * whole: its robot chip says "Merged"
 */
export function formatLandedWith(landed: LandedBy): string | null {
  if (landed.own === true) {
    return landed.after.length === 0 ? null : `Landed${formatAfter(landed)}`
  }
  return `Landed with ${formatLandingSubject(landed)}${formatAfter(landed)}`
}

/**
 * "Landed in master with oauth", of its own robot "Merged by Patronus": why a branch a merged Safe Push landed whole starts
 * no Safe Push and no Dry Run
 */
export function formatLandedReason(landed: Pick<LandedBy, "branch" | "robot" | "wholeStack" | "own">): string {
  return landed.own === true ? "Merged by Patronus" : `Landed in ${landed.robot.targetBranch} with ${formatLandingSubject(landed)}`
}

/**
 * Why a group of branches has nothing left to push or land once merged Safe Pushes landed every one of them whole
 * (`isLandedWhole` of each; the whole stack of `root`, a branch and the branches built on it): "Merged by Patronus"
 * while the robot of the whole stack of `root` landed them all, else "Landed in master with oauth and the whole air
 * stack", each robot named once, in the order they merged. Null while one of them is left, and for no branch.
 */
export function formatLandedWholeReason(landed: readonly (LandedBy | null | undefined)[], root: string): string | null {
  if (landed.length === 0 || !landed.every(isLandedWhole)) {
    return null
  }
  const all = landed as readonly LandedBy[]
  if (all.every(it => it.wholeStack === true && it.branch === root)) {
    return "Merged by Patronus"
  }
  const robots = [...new Map(all.map(it => [it.robot.id, it] as const)).values()].sort((a, b) => (a.robot.finishTime ?? 0) - (b.robot.finishTime ?? 0))
  return `Landed in ${robots[0]!.robot.targetBranch} with ${joinNames([...new Set(robots.map(formatLandingSubject))])}`
}

/**
 * "A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack": why the
 * stack branch of the whole stack is not pushed while its own robot runs, a push into it would fail the robot at the end
 */
export function formatStackPushHoldReason(root: string, robot: PatronusRobot): string {
  return `A ${formatPushMode(robot.pushMode)} of ${formatWholeStack(root)} is running: wait until it merges, or restart it on the current stack`
}

/**
 * What the branches a merged robot landed are found in: the branches of the table, the stack branches, and the answers
 * of `get_robot_landings` for robots whose branches got new copies since (`landingRequestOf`, by `landingKey`)
 */
export interface LandedSource {
  readonly branchOf: (name: string) => Branch | undefined
  readonly stackOf: (root: string) => StackBranch | undefined
  /** By `landingKey`, what a robot landed by content; null: nothing it could tell */
  readonly landings?: Readonly<Record<string, RobotLanding | null>>
  /** By `landingKey`, what a running robot lands by content (`spanRequestOf`); null: nothing it could tell */
  readonly spans?: Readonly<Record<string, RobotSpan | null>>
}

/**
 * The robot took the branch or the stack branch as it is: its tip, or the same commits by content before a sync rebuilt
 * them on a newer baseline (the remote branch at the robot's commit, up to date with the branch, `getReviewCommitState`)
 */
export function tookAsItIs(sourceRevision: string, tip: string | null | undefined, remote: RemoteStatus | null): boolean {
  return sourceRevision === tip || (tip != null && remote !== null && getReviewCommitState(sourceRevision, tip, remote) === "rebased")
}

/** A branch a merged robot landed (`findLandedBranches`) */
export interface LandedBranch {
  readonly branch: Branch
  /** The branch the robot ran on */
  readonly own: boolean
  /** The commits added after the landed ones, original hashes, newest first */
  readonly after: readonly string[]
}

/**
 * The commits of the branch above the one the robot took (`tookAsItIs`, or one of its commits the branch built new ones
 * on since: the copies below are the same objects, so the branch and what it is built on are as the robot took them),
 * newest first; null when the hashes don't tell
 */
function commitsAbove(sourceRevision: string, branch: Branch): readonly string[] | null {
  if (tookAsItIs(sourceRevision, branch.commits[0]?.hash, branch.remoteStatus)) {
    return []
  }
  const at = branch.commits.findIndex(commit => commit.hash === sourceRevision)
  return at < 0 ? null : branch.commits.slice(0, at).map(commit => commit.originalHash)
}

/** What a merged robot may have landed: the branch it ran on and those it is built on, root first; or the stack's */
function landingCandidates(robot: PatronusRobot, source: LandedSource): readonly string[] | null {
  if (robot.branchName !== null) {
    return source.branchOf(robot.branchName) === undefined ? null : [...stackAncestors(robot.branchName, source.branchOf).map(it => it.name), robot.branchName]
  }
  if (robot.stackRoot != null) {
    const stack = source.stackOf(robot.stackRoot)
    return stack === undefined || stack.members.length === 0 ? null : stack.members
  }
  return null
}

/**
 * Whether the hashes tell what a merged robot landed: it ran on a commit of its branch as the table has it
 * (`commitsAbove`), or on the stack branch as it is (`tookAsItIs`)
 */
function hashesTell(robot: PatronusRobot, revision: string, source: LandedSource): boolean {
  if (robot.branchName !== null) {
    const branch = source.branchOf(robot.branchName)
    return branch !== undefined && commitsAbove(revision, branch) !== null
  }
  const stack = robot.stackRoot == null ? undefined : source.stackOf(robot.stackRoot)
  return stack !== undefined && tookAsItIs(revision, stack.tip, stack.remoteStatus)
}

/** A robot that pushes into the target, of a branch or of the whole stack: a dry run lands nothing, a robot of a temporary ref is no branch's */
function isLandingRobot(robot: PatronusRobot): robot is PatronusRobot & { sourceRevision: string } {
  return robot.pushMode !== "dryRun" && robot.sourceRevision !== null && robotSubjectOf(robot) !== null
}

/** A robot that merged into the target (`isLandingRobot`) */
function isMergedLanding(robot: PatronusRobot): robot is PatronusRobot & { sourceRevision: string } {
  return robot.status === "success" && isLandingRobot(robot)
}

/**
 * The question for `get_robot_landings` of a merged robot whose hashes don't tell what it landed (`hashesTell`): a change
 * below its commit (an amend, a new commit of a branch it is built on; for the whole stack any change of a branch of
 * it, which rebuilds the stack branch) gave the branches new copies. Null for a robot the hashes answer, a dry run,
 * one of a temporary ref, or of no branch of the table.
 */
export function landingRequestOf(robot: PatronusRobot, source: LandedSource): RobotLandingRequest | null {
  if (!isMergedLanding(robot) || hashesTell(robot, robot.sourceRevision, source)) {
    return null
  }
  const branches = landingCandidates(robot, source)
  if (branches === null) {
    return null
  }
  return { robotId: robot.id, branchName: robot.branchName, stackRoot: robot.stackRoot ?? null, sourceRevision: robot.sourceRevision, branches: [...branches], tip: null }
}

/**
 * The question for `get_robot_spans` of a running robot that pushes into the target, when the hashes don't tell what it
 * lands (`hashesTell`): a restack or an amend below its commit after its start gave the branches new copies, so the
 * stack as it is now may hold a parent its commit never had. `branchName` is the branch the robot was matched to
 * (`findBranchRobot`, also by the tip for a robot of a temporary ref), null for the robot of the whole stack. Null for a
 * robot the hashes answer, a dry run, a root (it lands only itself) and a robot of no branch of the table.
 */
export function spanRequestOf(robot: PatronusRobot, branchName: string | null, source: LandedSource): RobotLandingRequest | null {
  if (!isRobotActive(robot) || robot.pushMode === "dryRun" || robot.sourceRevision === null) {
    return null
  }
  const subject = branchName === null ? robot : { ...robot, branchName, stackRoot: null }
  if ((subject.branchName === null && subject.stackRoot == null) || hashesTell(subject, robot.sourceRevision, source)) {
    return null
  }
  const branches = landingCandidates(subject, source)
  if (branches === null || (subject.branchName !== null && branches.length < 2)) {
    return null
  }
  return { robotId: robot.id, branchName: subject.branchName, stackRoot: subject.stackRoot ?? null, sourceRevision: robot.sourceRevision, branches: [...branches], tip: null }
}

/** What a running robot lands of `candidates` (`robotSpanOf`) */
export interface RobotSpanView {
  /** The candidates its commit has commits of, in their order */
  readonly names: readonly string[]
  /** By branch, how many of its commits the robot's commit has; null until the content tells */
  readonly counts: ReadonlyMap<string, number> | null
}

/**
 * What a running robot lands of `candidates` (root first): every candidate with all its commits while the hashes tell
 * (`spanRequestOf` is null: it took the branches as they are); else the answer of `get_robot_spans` (`source.spans`): a
 * parent a restack placed below after its start is not one. Until the content tells, or when it can't (the robot's
 * commit is not in the repository), every candidate, the number not known.
 */
export function robotSpanOf(robot: PatronusRobot, branchName: string | null, candidates: readonly string[], source: LandedSource): RobotSpanView {
  const request = spanRequestOf(robot, branchName, source)
  if (request === null) {
    return { names: candidates, counts: new Map(candidates.map(name => [name, source.branchOf(name)?.commits.length ?? 0])) }
  }
  const branches = source.spans?.[landingKey(request, source)]?.branches
  if (branches == null) {
    return { names: candidates, counts: null }
  }
  const counts = new Map(branches.map(span => [span.branch, span.commits.length]))
  return { names: candidates.filter(name => counts.has(name)), counts }
}

/**
 * What the answer of a landing request depends on: the robot's commit, the tip it ran on and the commits of the branches
 * as the table has them; an answer for another key is of another state of the branches
 */
export function landingKey(request: RobotLandingRequest, source: LandedSource): string {
  const subject =
    request.stackRoot == null ? (request.branchName === null ? undefined : source.branchOf(request.branchName)?.commits[0]?.hash) : source.stackOf(request.stackRoot)?.tip
  const branches = request.branches.map(name => `${name}:${(source.branchOf(name)?.commits ?? []).map(commit => `${commit.originalHash}>${commit.hash ?? ""}`).join(",")}`)
  return [request.robotId, request.sourceRevision, subject ?? "", ...branches].join("\n")
}

/**
 * The branches a merged robot landed (not a dry run, not a robot of a temporary ref; the caller knows it merged), with
 * the commits added after them:
 * - by the hashes: the robot of a branch ran on a commit of it as the table has it (`commitsAbove`), so the branch and
 *   every branch it is built on landed, the branch's commits above that commit are after it; the robot of the whole
 *   stack ran on the stack branch as it is, so every branch of the stack landed;
 * - else by content, the answer of `get_robot_landings` for the branches as the table has them (`landingKey`): after a
 *   change below the robot's commit its tested commits against each branch's, so a commit added above the landed ones is
 *   the only work left, and a branch with a commit changed since is not listed.
 */
export function findLandedBranches(robot: PatronusRobot, source: LandedSource): LandedBranch[] {
  if (!isLandingRobot(robot)) {
    return []
  }
  const revision = robot.sourceRevision
  const candidates = landingCandidates(robot, source)
  if (candidates === null) {
    return []
  }
  if (hashesTell(robot, revision, source)) {
    const own = robot.branchName === null ? undefined : source.branchOf(robot.branchName)
    const above = own === undefined ? [] : (commitsAbove(revision, own) ?? [])
    return candidates.flatMap(name => {
      const branch = source.branchOf(name)
      return branch === undefined ? [] : [{ branch, own: name === robot.branchName, after: name === robot.branchName ? above : [] }]
    })
  }
  const request = landingRequestOf(robot, source)
  const landing = request === null ? undefined : source.landings?.[landingKey(request, source)]
  return (landing?.branches ?? []).flatMap(landed => {
    const branch = source.branchOf(landed.branch)
    return branch === undefined || !candidates.includes(landed.branch) ? [] : [{ branch, own: landed.branch === robot.branchName, after: landed.after }]
  })
}

/**
 * The branches merged Safe Pushes landed (`findLandedBranches`), each with the robot that landed it: the one that left
 * the fewest commits after it, of equals the first merge; a robot first seen merged (after a restart, a `bd` landing)
 * included: the table has the branches as the robot took them, by hash or by content
 */
export function findLandedBy(robots: Readonly<Record<string, PatronusRobot>>, source: LandedSource): Map<string, LandedBy> {
  const merged = Object.values(robots)
    .filter(isMergedLanding)
    .sort((a, b) => (a.finishTime ?? 0) - (b.finishTime ?? 0))
  const landed = new Map<string, LandedBy>()
  for (const robot of merged) {
    const subject = robotSubjectOf(robot)
    if (subject === null) {
      continue
    }
    for (const { branch, own, after } of findLandedBranches(robot, source)) {
      const known = landed.get(branch.name)
      if (known !== undefined && known.after.length <= after.length) {
        continue
      }
      const by = subject.kind === "branch" ? { branch: subject.name, robot } : { branch: subject.root, robot, wholeStack: true }
      landed.set(branch.name, { ...by, ...(own && { own: true }), after })
    }
  }
  return landed
}

/**
 * The robot of a branch row (`useRowRobot`):
 * - `start`: none to show, "Safe Push" starts one; also for a robot of an earlier state of the branch (the details of
 *   that robot stay in the preview and in the landing overview);
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
  /** "Safe Push" is offered: Patronus answered and the branch can be pushed */
  readonly canOffer: boolean
  /**
   * Its merged robot landed all of the branch as it is now, by content (`findLandedBy`): a rebuild since gave the branch
   * new hashes, not new work
   */
  readonly landed?: boolean
}

/** The robot of a branch row (`NextStepSlot`, `BranchRowStatus`) */
export function getSafePushSlot({ robot, localTip, remote, needsReview, failing, canOffer, landed = false }: SafePushSlotInput): SafePushSlot {
  if (robot === undefined) {
    return canOffer ? { kind: "start" } : { kind: "none" }
  }
  const chip = getPatronusChipState(robot, localTip, remote, { needsReview, failing })
  if (isRobotActive(robot)) {
    return { kind: "running", chip }
  }
  // a merged robot has no warning dot, yet a branch with other commits since (the same name, new work) starts anew
  if (canOffer && !landed && isReviewCommitOutdated(getReviewCommitState(robot.sourceRevision, localTip, remote))) {
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

/**
 * The name of a start on its button and menu item: "Safe Push", "Safe Push · 3" for a stacked branch, which lands with
 * the branches it is built on in one robot (the count on the button, the names on its hover), "Dry Run". No "…": a start
 * opens a popover only when it has something to say first, and no input is asked
 */
export function formatStartLabel(mode: PatronusPushMode, branchCount: number): string {
  const name = formatPushMode(mode)
  return branchCount > 1 ? `${name} · ${branchCount}` : name
}

/**
 * What a start does, on the hover of its button and menu item: "Lands auth into master once the checks pass", "Lands
 * oauth with the 2 branches it is built on into master in one Safe Push"; a dry run "Checks auth on master without
 * pushing into it"
 */
export function describeStart(mode: PatronusPushMode, subject: string, branchCount: number, target: string): string {
  const builtOn = branchCount > 2 ? ` with the ${branchCount - 1} branches it is built on` : branchCount === 2 ? " with the branch it is built on" : ""
  return mode === "dryRun"
    ? `Checks ${subject}${builtOn} on ${target} without pushing into it`
    : `Lands ${subject}${builtOn} into ${target}${branchCount > 1 ? " in one Safe Push" : " once the checks pass"}`
}

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
 * (`isPullRequestIntoTarget`), a release branch, or a virtual branch until the push or the next step retargets it.
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
