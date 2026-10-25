// The page's words of the reasons Rust decides (`Reason` of `domain_policy`: holds, starts, restarts): one formatter per
// code, so a code Rust gives is one the page words (`crates/domain-policy/reasons.json`, `reasons.test.ts`). The field a
// reason is read from adds the words of its context: a hold of a stack branch says "nothing of it is left to push"
// (`formatStackHold`), a start check carries the reason of a disabled action (`startCheck`).
import type { HoldKind, PatronusPushMode, PatronusRobot, Reason, RobotOf } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { describeLandingRefusal } from "@/lib/landing-gate"
import { formatParkedBlocker } from "@/lib/parked"
import type { LandingAncestor, LandingDescendant, PushHold } from "@/lib/patronus"
import { formatLandingSubject, formatPushMode } from "@/lib/patronus"
import { joinNames } from "@/lib/text"
import { formatStackBranchConflict, formatWholeStack } from "@/lib/whole-stack"

/** Whether a Safe Push or a Dry Run can start; `reason` says why not, for the hover of a disabled action */
export type RobotStartCheck = { ok: true } | { ok: false; reason: string }

/** A start check of a reason Rust gave (`landing.start`, `landing.dryRun`, a restart's `refusal`): none starts */
export function startCheck(reason: Reason | null | undefined): RobotStartCheck {
  return reason == null ? { ok: true } : { ok: false, reason: formatReason(reason) }
}

/** The words of a reason, or null without one */
export function formatReasonOrNull(reason: Reason | null | undefined, statusText?: (branch: string) => string): string | null {
  return reason == null ? null : formatReason(reason, statusText)
}

/** "A Safe Push of auth is running: wait until it merges": a start waits for a robot that lands a part of what it lands */
export function formatLandingWait(landing: LandingAncestor): string {
  return landingWait(landing, landing.robot.pushMode)
}

function landingWait(landing: { readonly branch: string; readonly wholeStack?: boolean }, mode: PatronusPushMode): string {
  return `A ${formatPushMode(mode)} of ${formatLandingSubject(landing)} is running: wait until it merges`
}

/**
 * Why a branch can't start while a robot of a branch built on it runs: "A Safe Push of oauth is running: it lands 2
 * commits of auth with it", "A Safe Push of the whole air stack is running: it lands 1 commit of auth with it"; the
 * branch alone while the number is not told yet, as `bd` says it
 */
export function formatLandingReason(branchName: string, landing: LandingDescendant): string {
  return landingReason(branchName, landing, landing.robot.pushMode, landing.commits)
}

function landingReason(branchName: string, by: { readonly branch: string; readonly wholeStack?: boolean }, mode: PatronusPushMode, commits: number | undefined): string {
  const what = commits === undefined ? branchName : `${formatCommitCount(commits)} of ${branchName}`
  return `A ${formatPushMode(mode)} of ${formatLandingSubject(by)} is running: it lands ${what} with it`
}

/** What the words of a hold read of it and of its robot */
interface HoldWords {
  readonly kind: HoldKind
  readonly branch: string
  readonly wholeStack?: boolean
  readonly mode: PatronusPushMode
  readonly target: string
  readonly commits?: number
}

/** "Landed in master with oauth", of its own robot "Merged by Patronus" */
function landedWords(target: string, by: { readonly branch: string; readonly wholeStack?: boolean }, own: boolean): string {
  return own ? "Merged by Patronus" : `Landed in ${target} with ${formatLandingSubject(by)}`
}

function holdWords(branchName: string, hold: HoldWords): string {
  switch (hold.kind) {
    case "landed":
      return `${landedWords(hold.target, hold, hold.branch === branchName && hold.wholeStack !== true)}: nothing of it is left to push`
    case "own":
      return `A ${formatPushMode(hold.mode)} of ${branchName} is running: wait until it merges, or restart it on the current branch`
    case "span":
      return landingReason(branchName, hold, hold.mode, hold.commits)
  }
}

/**
 * Why a held branch offers no push (`PushHold`): "A Safe Push of auth is running: wait until it merges, or restart it
 * on the current branch", "A Safe Push of oauth is running: it lands auth with it", "Landed in master with oauth:
 * nothing of it is left to push"
 */
export function formatPushHoldReason(branchName: string, hold: PushHold): string {
  return holdWords(branchName, { ...hold, mode: hold.robot.pushMode, target: hold.robot.targetBranch })
}

/**
 * "A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack": why the
 * stack branch of the whole stack is not pushed while its own robot runs, a push into it would fail the robot at the end
 */
export function formatStackPushHoldReason(root: string, robot: Pick<PatronusRobot, "pushMode">): string {
  return stackHeldWords(root, robot.pushMode)
}

function stackHeldWords(root: string, mode: PatronusPushMode): string {
  return `A ${formatPushMode(mode)} of ${formatWholeStack(root)} is running: wait until it merges, or restart it on the current stack`
}

/** A robot of a reason, as the words of a subject name it */
function subjectOf(of: RobotOf): { readonly branch: string; readonly wholeStack?: boolean } {
  return of.wholeStack ? { branch: of.branch, wholeStack: true } : { branch: of.branch }
}

type Words = { readonly [C in Reason["code"]]: (reason: Extract<Reason, { code: C }>, statusText: (branch: string) => string) => string }

const words = {
  blocked: ({ by }) => (by === null ? "Blocked: fix the parent branch first" : `Blocked by ${by}: fix it first, then sync`),
  lastCommit: ({ mode }) => `A ${formatPushMode(mode)} of the last commit is running`,
  descendant: ({ branch, by, mode, carried }) => landingReason(branch, subjectOf(by), mode, carried?.commits.length),
  ancestor: ({ landing, mode }) => landingWait(subjectOf(landing), mode),
  landed: ({ by, own, target }) => landedWords(target, subjectOf(by), own),
  held: ({ branch, hold, mode, target }) => {
    const commits = hold.carried?.commits.length
    return holdWords(branch, { kind: hold.kind, ...subjectOf(hold.by), mode, target, ...(commits !== undefined && { commits }) })
  },
  noStackBranch: ({ root }) => `The ${root} stack has no stack branch: only a stack that forks builds one`,
  stackNotBuilt: ({ state, blockedBy, conflict, error }) => {
    switch (state) {
      case "Built":
        return "The stack branch is not built yet: sync first"
      case "Blocked":
        return blockedBy === null ? "A branch of the stack is blocked: fix it first, then sync" : `Blocked by ${blockedBy}: fix it first, then sync`
      case "Conflict":
        return formatStackBranchConflict(conflict)
      case "Stale":
        return error ?? "The stack branch is not up to date: sync first"
    }
  },
  stackParked: ({ member }) => `It holds parked ${member}: land the stack in pieces`,
  stackLanded: ({ by, own, target }) => (own ? "Merged by Patronus" : `Landed in ${target} with ${joinNames([...new Set(by.map(it => formatLandingSubject(subjectOf(it))))])}`),
  stackHeld: ({ root, mode }) => stackHeldWords(root, mode),
  cantPush: ({ branch }, statusText) => `${branch} can't be pushed: ${statusText(branch) === "" ? "it failed to sync" : statusText(branch)}`,
  dropsRemote: ({ branch, behind }) =>
    `The remote branch of ${branch} has ${behind === 1 ? "a commit" : `${behind} commits`} that ${branch} lacks: push it from its row, which shows ${behind === 1 ? "it" : "them"}`,
  parked: ({ with: parent, note }) => formatParkedBlocker({ with: parent, note }),
  unprotected: ({ target }) => `Patronus does not push into ${target}`,
  draft: ({ number }) => `#${number} is a draft: Patronus merges it once it is ready for review`,
  otherBase: ({ number, base }) => `Approvals in #${number} don't count: its base is ${base}`,
  approvalsMissing: () => "Patronus checks the approvals when the robot starts and fails without them",
  dismissesApprovals: ({ number }) => `The push dismisses the approvals of #${number}: push first, then ask for the review again`,
  landingRefused: ({ refusal }) => describeLandingRefusal(refusal, refusal.branch),
  targetConflict: ({ commit, files }) => `${formatShortHash(commit)} conflicts in ${joinNames(files)} with the newest target: Patronus would fail the robot at once`,
} satisfies Words as Words

/**
 * The page's words of `reason`. `statusText` names why the sync did not build a branch (its row's status text), for
 * the push an action can't make (`cantPush`)
 */
export function formatReason(reason: Reason, statusText: (branch: string) => string = () => ""): string {
  return (words[reason.code] as (reason: Reason, statusText: (branch: string) => string) => string)(reason, statusText)
}

/**
 * Why the stack branch of a whole stack is not pushed (`StackLanding.hold`): a parked member, merged Safe Pushes that
 * landed every branch of it ("Landed in master with deploy and side-test: nothing of it is left to push"), its own
 * robot, a stack branch the sync did not build; null while nothing holds it
 */
export function formatStackHold(reason: Reason | null | undefined): string | null {
  if (reason == null) {
    return null
  }
  const text = formatReason(reason)
  return reason.code === "stackLanded" ? `${text}: nothing of it is left to push` : text
}
