// The page's words of the next step of a subject as Rust decides it (`NextStep` of the view's `nextSteps`,
// `domain_policy::next_step`; spec/branch/next-step.spec.md): the label, the icon, the tone and the `MenuItem` of the
// slot, the reason every surface disables "Safe Push" with, the notice of S. It decides nothing: the row and its facts
// are Rust's, the items run what the caller gives (today's orchestrators: the start, the retarget, the pull request),
// and what runs on the row (an operation, its own start: `NextStepBusy`) is laid over the step here.
import {
  ClipboardCopyIcon,
  ClockIcon,
  DownloadIcon,
  FileTextIcon,
  GitMergeIcon,
  GitPullRequestArrowIcon,
  Layers2Icon,
  MessageSquareWarningIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  TriangleAlertIcon,
  UserPlusIcon,
  LayersIcon,
} from "lucide-react"
import type { LandingFix, MissingReviewGroup, NextStep, Person, StepBlocker, StepFact, StepFailure, StepKind } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { getErrorDetails } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { describeMerge, formatMergeLabel, formatMergeMethod } from "@/lib/merge-pull-request"
import { describeStart, formatPatronusAvailabilityHint, formatStartLabel } from "@/lib/patronus"
import { formatAvailabilityHint } from "@/lib/pull-request"
import { describePushToBaseline, formatPushToBaselineLabel } from "@/lib/push-to-baseline"
import { joinNames } from "@/lib/text"
import { formatWholeStack } from "@/lib/whole-stack"
import { describeLandingRefusals, formatLandingFix, neededCommit } from "@/presenters/landing"
import { formatLandedWhole, formatReason } from "@/presenters/reasons"

/**
 * The step as the slot shows it (`data-step`), a row of [the table](../../spec/branch/next-step.spec.md#the-table) by
 * the page's name: `none` row 1 (the Status cell says why), `pending` row 0 (a fact without an answer, or a read that
 * failed), `hidden` no slot (not on Space)
 */
export type NextStepKind =
  | "hidden"
  | "none"
  | "pending"
  | "needsBranch"
  | "pickRemoteCommits"
  | "updateProject"
  | "restart"
  | "pullRequest"
  | "retarget"
  | "linkStack"
  | "otherBase"
  | "draft"
  | "createPullRequest"
  | "changesRequested"
  | "requestReview"
  | "awaiting"
  | "safePush"

/** `quiet` is a state as a label (a wait, a draft): muted, it opens its object; `destructive` the same in red */
export type NextStepTone = "default" | "quiet" | "destructive"

/** A fact the step waits for (`StepKind.pending`): the slot names it (`data-waits-for`) */
export type PendingFact = StepFact

/** The operation that works on the row: the `id` of its item, its busy state ("Retargeting #12 to master") */
export interface NextStepBusy {
  readonly id: string
  readonly label: string
  /** The fields of the pull request it writes (`OperationKey.resources`): `pr:8:reviewers` */
  readonly resources?: readonly string[]
}

/** What the items of a step run, from the caller: the orchestrators of the page, and the items still built there */
export interface NextStepActions {
  /** The start of the Safe Push (`requestSafePush`): at once, or its popover at the row's slot */
  readonly openSafePush: () => void
  /** Opens the pull request of the subject */
  readonly openPullRequest: () => void
  /** Retargets the pull request of the subject to the target */
  readonly retarget: () => void
  /** Links the stack of GitHub of the subject, a layer of one (`Intent::LinkStack`) */
  readonly linkStack: () => void
  /** Opens the pull request of the whole stack of `root` (row 7, "In #12") */
  readonly openStackPullRequest: (root: string) => void
  /** Runs the fix of what the Safe Push needs (`LandingFix`): the restack of the root of what it lands */
  readonly fixLanding: (fix: LandingFix) => void
  /** Copies the login command of the Safe Push CLI; null without a repository */
  readonly copyLoginCommand: (() => void) | null
  readonly refreshPatronus: () => void
  /** Pick Remote Commits… of the subject: the question of its push button, which lists the commits with the pick */
  readonly pickRemoteCommits: () => void
  /** Update Project (⌘T) of the repository the subject is of */
  readonly updateProject: () => void
  /** "Restart Safe Push" of the robot of the subject (`getRobotActions`); null when it can't restart */
  readonly restart: MenuItem | null
  /**
   * "Create Pull Request…" of the menu, which pushes first, where the GitHub repository of the branches is known (the
   * step's `repository`); null when the subject can't get one
   */
  readonly createPullRequest: MenuItem | null
  /** "Request Review…" of the open pull request, which opens its popover; null without an open pull request */
  readonly requestReview: MenuItem | null
}

/** What the words of a step need beside the step, and what runs on its row */
export interface StepContext {
  /** What the texts call the subject: "auth", "the whole air stack" */
  readonly subject: string
  /** The branch Patronus pushes into, as a pull request names its base ("master") */
  readonly target: string
  /** What the start of a whole stack lands (`describeWholeStackLanding`); null: the subject and what it is built on */
  readonly startDescription: string | null
  /** The operation on the row, its own running start left out for a start (`forStart`); null while none runs */
  readonly busy: NextStepBusy | null
  /**
   * The step as a start reads it (`readSafePush`): the start waits for the readiness itself (its plan, `prepare`), so
   * a readiness without an answer, or a stale one, holds nothing back
   */
  readonly forStart: boolean
  readonly actions: NextStepActions
}

/** The next step of a subject in the page's words, the same on every surface */
export interface BranchNextStep {
  readonly kind: NextStepKind
  /** The action of the slot; null for `none` and `hidden` */
  readonly item: MenuItem | null
  readonly tone: NextStepTone
  /** Why "Safe Push" can't open now, the same on every surface; null when it can */
  readonly safePushBlocker: string | null
  /** Why a Dry Run can't start: never the approvals */
  readonly dryRunBlocker: string | null
  /**
   * Why no start of the subject can start whatever its step: a reason of the subject (a sync, a failure, no commits,
   * the robots, parked, landed), else what runs on the row; all that holds back a Safe Push that skips the approvals
   * (`startStepBlocker`)
   */
  readonly subjectBlocker: string | null
  /** The step depends on the readiness of the subject: the session asks Patronus for it */
  readonly needsReadiness: boolean
  /** The fact a pending step ("Checking…", row 0) waits for, for `data-waits-for`; null for any other step */
  readonly waitsFor: PendingFact | null
  /** The `info` toast of S while "Safe Push" is disabled: a short title naming the subject, the reason; null when it can open */
  readonly notice: { readonly title: string; readonly description: string } | null
  /** "Retarget #7 to master" of an open pull request into another base, for the menu; null otherwise */
  readonly retarget: MenuItem | null
  /** Rows 7 and 11 unlisted: a Safe Push without review offered beside, with why it can't start now; null elsewhere */
  readonly withoutReview: { readonly blocker: string | null } | null
  /** Why nothing of it is left to land or review (row 1): "Landed in origin/master", "Merged by Patronus"; null while work is left */
  readonly landed: string | null
  /** Row 1′: the fix of what the Safe Push needs, for the menu; null elsewhere and for a fix no action runs */
  readonly landingFix: MenuItem | null
}

// ---- the words of the facts ----

/** "Ilya Korennoy", the login without a full name */
export function formatPerson(person: Person): string {
  return person.fullName ?? person.login ?? "a reviewer"
}

/** "1 more approval", "2 more approvals" */
export function formatMoreApprovals(needed: number): string {
  return needed === 1 ? "1 more approval" : `${needed} more approvals`
}

/** The people of the groups, each once, by login (else by name) */
function distinctPeople(people: readonly Person[]): Person[] {
  const seen = new Set<string>()
  return people.filter(person => {
    const key = (person.login ?? person.fullName ?? "").toLowerCase()
    if (seen.has(key)) {
      return false
    }
    seen.add(key)
    return true
  })
}

function groupNames(groups: readonly MissingReviewGroup[]): string {
  return joinNames(groups.map(group => group.name))
}

/** "Ilya Korennoy is requested", "Ilya Korennoy and Alice are requested" */
function formatRequested(people: readonly Person[]): string {
  return `${joinNames(people.map(formatPerson))} ${people.length === 1 ? "is" : "are"} requested`
}

/** The awaited people of the missing groups */
export function awaitedPeople(groups: readonly MissingReviewGroup[]): Person[] {
  return distinctPeople(groups.flatMap(group => group.requested))
}

/** "Awaiting Ilya Korennoy", "Awaiting 2 Reviews": the wait, and a line of its hover */
export function formatAwaitingLabel(people: readonly Person[]): string {
  return people.length === 1 ? `Awaiting ${formatPerson(people[0]!)}` : `Awaiting ${people.length} Reviews`
}

/** "Awaiting Review", "Awaiting 2 Reviews": the wait in a column of steps (`compactLabel`); its hover names the people */
export function formatAwaitingStep(count: number): string {
  return count === 1 ? "Awaiting Review" : `Awaiting ${count} Reviews`
}

/**
 * The lines of the hover of the wait, one per missing group: "Awaiting Ilya Korennoy · IJPL Code Platform", "1 more
 * approval · IJPL Code Platform" for a group that has approvals already or needs more than one
 */
export function describeAwaiting(groups: readonly MissingReviewGroup[]): string[] {
  return groups.map(group => {
    const needed = group.needed ?? 1
    const waiting = group.needed !== null && (needed > 1 || group.approvedBy.length > 0) ? formatMoreApprovals(needed) : formatAwaitingLabel(group.requested)
    return `${waiting} · ${group.name}`
  })
}

/** Why Safe Push waits for the approvals of the missing groups (rows 7 to 10), with the pull request where they count */
function formatApprovalBlocker(
  kind: "createPullRequest" | "changesRequested" | "requestReview" | "awaiting",
  groups: readonly MissingReviewGroup[],
  pullRequest: number | null,
): string {
  const names = groupNames(groups)
  const on = pullRequest === null ? "" : ` on #${pullRequest}`
  switch (kind) {
    case "createPullRequest":
      return `Code owners must approve in a pull request: ${names}`
    case "changesRequested": {
      const people = distinctPeople(groups.flatMap(group => group.changesRequestedBy))
      return `${joinNames(people.map(formatPerson))} requested changes${on}`
    }
    case "awaiting":
      return `Waits for the approval of ${names}: ${formatRequested(awaitedPeople(groups))}${on}`
    case "requestReview": {
      const asked = groups.some(group => group.requested.length > 0)
      return asked
        ? `Waits for the approval of ${names}: fewer people are requested${on} than it needs`
        : `Waits for the approval of ${names}: nobody of ${groups.length === 1 ? "it" : "them"} is requested${on}`
    }
  }
}

/** What a pending step waits for (`PendingFact`): the hover of "Checking…", why Safe Push is disabled, and who answers */
const PENDING_FACTS: Readonly<Record<PendingFact, (subject: string) => { readonly reason: string; readonly from: string }>> = {
  patronus: () => ({ reason: "Waits for the answer of Patronus", from: "Patronus" }),
  pullRequests: () => ({ reason: "Waits for GitHub to list the pull requests", from: "GitHub" }),
  landingNeeds: subject => ({ reason: `Checking what ${subject} needs to land…`, from: "the check of what it needs" }),
  robotProblems: () => ({ reason: "Waits for Patronus to tell why the Safe Push failed", from: "Patronus" }),
  readiness: () => ({ reason: "Waits for Patronus to check the approvals", from: "Patronus" }),
  permission: () => ({ reason: "Waits for GitHub to tell what you may do in the repository", from: "GitHub" }),
  targetRules: () => ({ reason: "Waits for GitHub to tell the rules of the target", from: "GitHub" }),
  mergeState: () => ({ reason: "Waits for GitHub to tell whether it can merge the pull request", from: "GitHub" }),
}

/** Why the step of `subject` waits for `fact`, in the words of its hover and of every disabled Safe Push */
export function formatPendingFact(fact: PendingFact, subject = "the branch"): string {
  return PENDING_FACTS[fact](subject).reason
}

/** A stale answer of the readiness: its step stays while the new one loads, its action waits */
const STALE_READINESS = "Waits for Patronus to check the approvals again"

/** "GitHub: offline: Patronus checks the approvals when the robot starts", of a failed listing of the pull requests */
export function formatUnlisted(reason: string): string {
  return `${reason}: Patronus checks the approvals when the robot starts`
}

/** Why no start is decided while Patronus didn't give the view of its robots, and no earlier one stands for it */
export function formatViewFailure(failure: string): string {
  return `Patronus didn't tell what its robots hold and land (${failure})`
}

/** Why a restart of a failed robot is not offered while Patronus didn't give why it failed */
export function formatProblemsFailure(failure: string): string {
  return `Patronus didn't tell why the Safe Push failed (${failure}): Check Approvals Again asks again`
}

/** Why a read the step decides on failed: the Safe Push CLI not logged in, Patronus failing, the view, the problems */
export function formatStepFailure(failure: StepFailure): string {
  switch (failure.kind) {
    case "notLoggedIn":
      return formatPatronusAvailabilityHint({ status: "notLoggedIn" }) ?? "Patronus doesn't answer"
    case "patronusError":
      return formatPatronusAvailabilityHint({ status: "error", message: failure.message }) ?? "Patronus doesn't answer"
    case "viewFailed":
      return formatViewFailure(getErrorDetails(failure.reason))
    case "problemsFailed":
      return formatProblemsFailure(failure.message)
  }
}

/** Nobody of a missing group can approve (`noEligibleReviewer`): Request Review… can't help */
function formatNoEligibleReviewer(groups: readonly MissingReviewGroup[]): string {
  return `Nobody of ${groupNames(groups)} can approve these changes`
}

/** Patronus checks the approvals only when the robot starts: the start stays possible while GitHub tells a wait */
const UNJUDGED = "Patronus can't tell yet whether the approvals there count: the start checks them"

/** A reason of the subject that holds every start back (a sync, a failure, the robots, parked, landed), never the step's own */
function isOwn(blocker: StepBlocker | null): boolean {
  return blocker === null || blocker.code === "step" || blocker.code === "staleReadiness"
}

/**
 * The words of a reason of the subject (`isOwn` false); the step's own reasons are worded by its kind
 * (`presentNextStep`), "" here
 */
export function formatStepBlocker(blocker: StepBlocker): string {
  switch (blocker.code) {
    case "rule":
      return formatReason(blocker.reason)
    case "syncing":
      return "Wait for the sync to finish"
    case "syncFailed":
      return blocker.problem ?? "The branch failed to sync"
    case "noCommits":
      return "No commits to push"
    case "step":
    case "staleReadiness":
      return ""
  }
}

// ---- the items ----

/** The items and the words of the step itself, before the reasons of the subject and what runs on the row lie over it */
interface Shown {
  readonly kind: NextStepKind
  readonly item: MenuItem | null
  readonly tone?: NextStepTone
  /** The step's own reason (`StepBlocker.step`: row 0's fact, 5′'s base, 6's draft, 7 to 10's approvals) */
  readonly own?: string
  /** The notice of S for the step's own reason, while no reason of the subject outranks it */
  readonly notice?: { readonly title: string; readonly description: string }
  readonly retarget?: MenuItem
  readonly landingFix?: MenuItem | null
}

/** "Safe Push" ("Safe Push · 3" with what it is built on), disabled with `blocker` */
function safePushItem(step: NextStep, context: StepContext, blocker: string | null): MenuItem {
  const lands = context.startDescription ?? describeStart("rebaseAutosquash", context.subject, step.lands, context.target)
  // a failed listing is no "no pull request": the start can't count the approvals of one, Patronus checks them
  const unlisted = formatAvailabilityHint(step.unlisted)
  return {
    id: "safe-push",
    label: formatStartLabel("rebaseAutosquash", step.lands),
    icon: ShieldCheckIcon,
    kbds: ["s"],
    disabled: blocker !== null,
    description: blocker ?? (unlisted === null ? lands : `${lands} · ${formatUnlisted(unlisted)}`),
    onSelect: context.actions.openSafePush,
  }
}

/** A quiet step: the state as its label, opening the pull request it waits in (O on the row opens it too) */
function quietItem(actions: NextStepActions, label: string, description: string, icon: MenuItem["icon"]): MenuItem {
  return { id: "open-pull-request", label, icon, kbds: ["o"], description, onSelect: actions.openPullRequest }
}

/** "Retarget #7 to master" of the open pull request `number` into `base` */
function retargetItem(step: NextStep, context: StepContext, number: number, base: string): MenuItem {
  const target = step.retarget?.target ?? context.target
  return {
    id: "retarget-pull-request",
    label: `Retarget #${number} to ${target}`,
    icon: GitPullRequestArrowIcon,
    description: `Changes the base of #${number} from ${base} to ${target} on GitHub: Patronus merges a pull request into its base`,
    onSelect: context.actions.retarget,
  }
}

/** What helps after a read failed: Copy Command of the login of the Safe Push CLI, else Refresh */
function helpItem(help: Extract<StepKind, { kind: "unavailable" }>["help"], reason: string, actions: NextStepActions): MenuItem | null {
  if (help === null) {
    return null
  }
  const copy = actions.copyLoginCommand
  if (help === "copyLoginCommand" && copy !== null) {
    return {
      id: "copy-safe-push-login",
      label: "Copy Login Command",
      icon: ClipboardCopyIcon,
      description: `${reason}: the command goes to the clipboard, for a terminal`,
      onSelect: copy,
    }
  }
  return { id: "refresh-patronus", label: "Refresh Patronus", icon: RefreshCwIcon, description: reason, onSelect: actions.refreshPatronus }
}

/**
 * Row 1′: the build of what the Safe Push lands needs a commit of a branch outside it: "Needs stills-codec", which runs
 * the fix ("Stack lane-run on stills-codec"), the reason in its hover; a fix no action runs (the move of a commit) opens
 * the Safe Push popover, which says it and starts anyway
 */
function needsBranch(kind: Extract<StepKind, { kind: "needsBranch" }>, context: StepContext): Shown {
  const decision = describeLandingRefusals(kind.refusals, context.subject)
  const needed = neededCommit(kind.refusals[0]!)
  const label = `Needs ${needed.branch === "" ? formatShortHash(needed.id) : needed.branch}`
  const reason = decision?.reason ?? ""
  const fix = decision?.fix ?? null
  const fixLabel = formatLandingFix(fix)
  const landingFix: MenuItem | null =
    fix === null || fixLabel === null ? null : { id: "landing-fix", label: fixLabel, icon: Layers2Icon, description: reason, onSelect: () => context.actions.fixLanding(fix) }
  const item: MenuItem =
    landingFix === null
      ? { id: "landing-needs", label, icon: TriangleAlertIcon, description: reason, onSelect: context.actions.openSafePush }
      : { ...landingFix, label, icon: TriangleAlertIcon }
  return { kind: "needsBranch", item, landingFix }
}

/** Row 7: the pull request that collects the approvals, or "In #12" of the whole stack whose own Safe Push they count for */
function createPullRequest(kind: Extract<StepKind, { kind: "createPullRequest" }>, context: StepContext): Shown {
  const { subject, actions } = context
  const reason = formatApprovalBlocker("createPullRequest", kind.groups, null)
  const stack = kind.stack
  if (stack !== null) {
    const counts = `Approvals in #${stack.number}, the pull request of ${formatWholeStack(stack.root)}, count for its Safe Push only`
    const item: MenuItem = {
      id: "open-stack-pull-request",
      label: `In #${stack.number}`,
      icon: GitPullRequestArrowIcon,
      description: `${reason}\n${counts}\nOpen Pull Request #${stack.number}`,
      onSelect: () => actions.openStackPullRequest(stack.root),
    }
    const notice = { title: `Safe Push of ${subject} needs a pull request`, description: `Code owners must approve in it: ${groupNames(kind.groups)} · ${counts}` }
    return { kind: "createPullRequest", item, tone: "quiet", own: reason, notice }
  }
  const item = (kind.repository ? actions.createPullRequest : null) ?? {
    id: "create-pull-request",
    label: "Create Pull Request…",
    icon: GitPullRequestArrowIcon,
    disabled: true,
    description: "The GitHub repository of the branches is unknown",
    onSelect: () => {},
  }
  const notice = { title: `Safe Push of ${subject} needs a pull request`, description: `Code owners must approve in it: ${groupNames(kind.groups)}` }
  return { kind: "createPullRequest", item: { ...item, description: item.disabled === true ? item.description : reason }, own: reason, notice }
}

/** Row 9: Request Review… of the pull request; nobody of a group can approve: no request helps */
function requestReview(kind: Extract<StepKind, { kind: "requestReview" }>, context: StepContext): Shown {
  const reason = formatApprovalBlocker("requestReview", kind.groups, kind.number)
  const request = context.actions.requestReview ?? { id: "request-review", label: "Request Review…", icon: UserPlusIcon, onSelect: () => {} }
  const item: MenuItem = kind.noEligible
    ? { ...request, disabled: true, description: formatNoEligibleReviewer(kind.groups) }
    : request.disabled === true
      ? request
      : { ...request, description: `${reason}. ${request.description ?? ""}`.trim() }
  const notice = { title: `Safe Push of ${context.subject} waits for a review request`, description: `${groupNames(kind.groups)} · nobody is requested on #${kind.number}` }
  return { kind: "requestReview", item, own: reason, notice }
}

/** Row merge″: what holds the merge of the pull request back, its reason the blocker of the step; a fix in red */
function mergeHeld(step: NextStep, number: number, context: StepContext): Shown {
  const reason = step.safePush?.code === "rule" ? step.safePush.reason : null
  const words = reason === null ? `The merge of #${number} waits` : formatReason(reason)
  const [label, icon, fix]: [string, MenuItem["icon"], boolean] = (() => {
    switch (reason?.code) {
      case "mergeConflict":
        return ["Conflict", TriangleAlertIcon, true]
      case "mergeChangesRequested":
        return ["Changes Requested", MessageSquareWarningIcon, true]
      case "mergeChecksFail":
        return ["Checks Fail", TriangleAlertIcon, true]
      case "mergeBehind":
        return ["Update Needed", RefreshCwIcon, false]
      case "mergeHeadBehind":
        return ["Push Branch First", RefreshCwIcon, false]
      case "mergeDraft":
        return ["Draft", FileTextIcon, false]
      case "mergeNeedsApprovals":
        return ["Awaiting Approval", ClockIcon, false]
      case "mergeChecksRunning":
        return ["Checks Running", ClockIcon, false]
      default:
        return ["Merge Blocked", ClockIcon, false]
    }
  })()
  const item = quietItem(context.actions, label, `${words}\nOpen Pull Request #${number}`, icon)
  const notice = { title: `The merge of #${number} of ${context.subject} waits`, description: words }
  return { kind: fix ? "changesRequested" : "awaiting", item, tone: fix ? "destructive" : "quiet", own: words, notice }
}

/** Row 10: someone of every missing group is asked: the wait, its hover a line per group */
function awaiting(kind: Extract<StepKind, { kind: "awaiting" }>, context: StepContext): Shown {
  const { groups, number } = kind
  const reason = formatApprovalBlocker("awaiting", groups, number)
  const hover = [...describeAwaiting(groups), `Open Pull Request #${number}`].join("\n")
  const people = awaitedPeople(groups)
  const waitsFor = people.length === 1 ? formatPerson(people[0]!) : `${people.length} reviews`
  const item = { ...quietItem(context.actions, formatAwaitingLabel(people), hover, ClockIcon), compactLabel: formatAwaitingStep(people.length) }
  const notice = { title: `Safe Push of ${context.subject} waits for ${waitsFor}`, description: `${groupNames(groups)} · requested on #${number}` }
  return { kind: "awaiting", item, tone: "quiet", own: reason, notice }
}

/** The items of the step of `step.kind`; `general` the reason of the subject (or what runs on the row) that holds every start back */
function show(step: NextStep, context: StepContext, general: string | null): Shown {
  const { subject, actions } = context
  const kind = step.kind
  switch (kind.kind) {
    case "hidden":
      return { kind: "hidden", item: null }
    case "settled":
      return { kind: "none", item: null }
    case "pending": {
      const { reason, from } = PENDING_FACTS[kind.fact](subject)
      // its spinner where the step will be, muted: the app reads what decides it
      const item: MenuItem = { id: "next-step-pending", label: "Checking…", disabled: true, running: true, description: reason, onSelect: () => {} }
      return { kind: "pending", item, tone: "quiet", own: reason, notice: { title: `Safe Push of ${subject} waits for ${from}`, description: reason } }
    }
    case "unavailable": {
      const reason = formatStepFailure(kind.failure)
      const restart = kind.robotId === null ? null : actions.restart
      const item = helpItem(kind.help, reason, actions) ?? { ...(restart ?? safePushItem(step, context, reason)), disabled: true, description: reason }
      return { kind: "pending", item, own: reason, notice: { title: `Safe Push of ${subject} can't start`, description: reason } }
    }
    case "needsBranch":
      return needsBranch(kind, context)
    case "pickRemoteCommits": {
      // row 1″: a push, also the one before a start, would drop the work of someone else on the remote branch
      const commits = formatCommitCount(kind.count)
      const reason = `The remote has ${commits} of someone else that ${subject} doesn't: a push drops ${kind.count === 1 ? "it" : "them"}`
      const item: MenuItem = { id: "next-step-pick-remote", label: "Pick Remote Commits", icon: DownloadIcon, description: reason, onSelect: actions.pickRemoteCommits }
      return { kind: "pickRemoteCommits", item, own: reason, notice: { title: `${subject} lacks ${commits} of the remote`, description: reason } }
    }
    case "updateProject": {
      // row push′: the copy is built on the commit before the new ones of the fetched baseline, a push of it is no
      // fast-forward; the update builds it anew on them, then the step offers the push
      const reason = `${kind.upstream} has new commits: Update Project builds ${subject} anew on them before its push`
      const item: MenuItem = { id: "next-step-update-project", label: "Update Project", icon: RefreshCwIcon, description: reason, onSelect: actions.updateProject }
      return { kind: "updateProject", item, own: reason, notice: { title: `${kind.upstream} has new commits`, description: reason } }
    }
    case "restart":
      return actions.restart === null ? { kind: "safePush", item: safePushItem(step, context, general) } : { kind: "restart", item: actions.restart }
    case "robotPullRequest": {
      // nobody can tell whether the approvals count: a restart may fail again, the pull request collects them
      const item = kind.number === null ? actions.createPullRequest : quietItem(actions, "Open Pull Request", `Open Pull Request #${kind.number}`, GitPullRequestArrowIcon)
      return item === null ? { kind: "safePush", item: safePushItem(step, context, general) } : { kind: "pullRequest", item }
    }
    case "retarget": {
      // a parent's branch: every pull request of a virtual branch goes into the target; short in its column
      const retarget = retargetItem(step, context, kind.number, kind.base)
      const reason = `Approvals in #${kind.number} don't count: its base is ${kind.base}`
      return { kind: "retarget", item: { ...retarget, compactLabel: `Retarget #${kind.number}`, description: `${reason}, a virtual branch. ${retarget.description}` }, retarget }
    }
    case "linkStack": {
      // a layer into the branch below it outside a stack of GitHub: its merge would land nothing until the stack is linked
      const reason = `#${kind.number} goes into the branch below it outside a stack of GitHub: its merge would land nothing`
      const item: MenuItem = {
        id: "link-github-stack",
        label: "Link GitHub Stack",
        compactLabel: "Link Stack",
        icon: LayersIcon,
        description: `${reason}. Links the stack: each layer into the branch below it`,
        onSelect: actions.linkStack,
      }
      return { kind: "linkStack", item, own: reason, notice: { title: `#${kind.number} of ${subject} is outside its GitHub stack`, description: reason } }
    }
    case "otherBase": {
      // another base may be meant (a release branch): the step opens the pull request, the retarget is in the menu
      const retarget = retargetItem(step, context, kind.number, kind.base)
      const reason = `Approvals in #${kind.number} don't count: its base is ${kind.base}`
      const item = { ...quietItem(actions, "Open Pull Request", `${reason}. Open Pull Request #${kind.number}`, GitPullRequestArrowIcon), id: "open-pull-request-other-base" }
      const notice = { title: `Approvals in #${kind.number} of ${subject} don't count`, description: `Its base is ${kind.base}, not ${step.retarget?.target ?? context.target}` }
      return { kind: "otherBase", item, tone: "quiet", own: reason, retarget, notice }
    }
    case "draft": {
      const reason = `#${kind.number} is a draft: Patronus merges it once it is ready for review`
      const item = { ...quietItem(actions, "Draft", `${reason}. Open Pull Request #${kind.number}`, FileTextIcon), id: "open-pull-request-draft" }
      return {
        kind: "draft",
        item,
        tone: "quiet",
        own: reason,
        notice: { title: `#${kind.number} of ${subject} is a draft`, description: "Patronus merges it once it is ready for review" },
      }
    }
    case "createPullRequest":
      return createPullRequest(kind, context)
    case "changesRequested": {
      // only the changes requested by a member of a group still short of approvals block, as Patronus decides
      const reason = formatApprovalBlocker("changesRequested", kind.groups, kind.number)
      const item = quietItem(actions, "Changes Requested", `${reason}. Open Pull Request #${kind.number}`, MessageSquareWarningIcon)
      return { kind: "changesRequested", item, tone: "destructive", own: reason, notice: { title: `Safe Push of ${subject} waits for changes`, description: reason } }
    }
    case "requestReview":
      return requestReview(kind, context)
    case "awaiting":
      return awaiting(kind, context)
    case "changesRequestedUnjudged": {
      // Patronus can't judge the approvals: GitHub tells what the pull request waits for, the start stays possible
      const reason = `${joinNames(kind.by)} requested changes on #${kind.number} · ${UNJUDGED}`
      return {
        kind: "changesRequested",
        item: quietItem(actions, "Changes Requested", `${reason}\nOpen Pull Request #${kind.number}`, MessageSquareWarningIcon),
        tone: "destructive",
      }
    }
    case "awaitingUnjudged": {
      const { reviewers, number } = kind
      const reason = `Waits for ${joinNames(reviewers)} on #${number} · ${UNJUDGED}`
      const label = reviewers.length === 1 ? `Awaiting ${reviewers[0]!}` : `Awaiting ${reviewers.length} Reviews`
      const item = { ...quietItem(actions, label, `${reason}\nOpen Pull Request #${number}`, ClockIcon), compactLabel: formatAwaitingStep(reviewers.length) }
      return { kind: "awaiting", item, tone: "quiet" }
    }
    case "safePush":
      return { kind: "safePush", item: safePushItem(step, context, general) }
    case "pushToBaseline": {
      // the repository lands by a push of the copy into the target: the start of the row, by the route Rust decides
      const label = formatPushToBaselineLabel(kind.target, step.lands)
      const description = general ?? context.startDescription ?? describePushToBaseline(subject, step.lands, kind.target)
      return { kind: "safePush", item: { ...safePushItem(step, context, general), id: "push-to-baseline", label, icon: GitMergeIcon, description } }
    }
    case "openPullRequest": {
      // row pr: GitHub merges a pull request, so the step is its creation
      const reason = `${subject} has no open pull request: GitHub merges it once you create one`
      const item = actions.createPullRequest ?? {
        id: "create-pull-request",
        label: "Create Pull Request…",
        icon: GitPullRequestArrowIcon,
        disabled: true,
        description: "The GitHub repository of the branches is unknown",
        onSelect: () => {},
      }
      const notice = { title: `${subject} needs a pull request`, description: reason }
      return { kind: "createPullRequest", item: { ...item, description: item.disabled === true ? item.description : reason }, own: reason, notice }
    }
    case "mergePullRequest": {
      // row merge: GitHub merges the pull request now, or once its checks pass; the start of the row
      const label = formatMergeLabel(kind.number, kind.auto)
      const description = general ?? context.startDescription ?? describeMerge(subject, step.lands, kind.number, context.target, kind.method, kind.auto)
      return { kind: "safePush", item: { ...safePushItem(step, context, general), id: "merge-pull-request", label, icon: GitMergeIcon, description } }
    }
    case "autoMerging": {
      // row merge′: GitHub merges it by itself once its checks pass; Cancel Auto-Merge is in the menus
      const reason = `GitHub merges #${kind.number} by ${formatMergeMethod(kind.method)} once its checks pass`
      const item = quietItem(actions, "Merges when checks pass", `${reason}\nOpen Pull Request #${kind.number}`, ClockIcon)
      return { kind: "awaiting", item, tone: "quiet", own: reason, notice: { title: `#${kind.number} of ${subject} merges by itself`, description: reason } }
    }
    case "mergeHeld":
      return mergeHeld(step, kind.number, context)
  }
}

/**
 * The field of the pull request each of these actions writes: a retarget and a review request of one pull request run
 * side by side (`OperationKey.resources`)
 */
const PULL_REQUEST_FIELDS: Readonly<Record<string, string>> = { "retarget-pull-request": "base", "request-review": "reviewers" }

/**
 * Whether the step `id` writes another field of the pull request than the running action `busy` writes: none of the
 * fields `busy` names (a review request that pushes first may retarget too, so it names the base as well)
 */
function writesOtherField(id: string | undefined, busy: NextStepBusy): boolean {
  const field = id === undefined ? undefined : PULL_REQUEST_FIELDS[id]
  return field !== undefined && busy.resources !== undefined && !busy.resources.some(resource => resource.endsWith(`:${field}`))
}

/**
 * An item while `busy` runs on the row: the action of the slot running itself (a retarget, a restart), else waiting for
 * it, unless it writes another field of the pull request (a retarget during a review request); a quiet step only opens
 * the pull request and stays
 */
function withBusy(item: MenuItem, tone: NextStepTone, busy: NextStepBusy): MenuItem {
  const reason = `${busy.label}…`
  if (item.id === busy.id) {
    return { ...item, running: true, disabled: true, description: reason }
  }
  return tone === "default" && item.disabled !== true && !writesOtherField(item.id, busy) ? { ...item, disabled: true, description: reason } : item
}

/** The kinds of the rows a step stands on before any of its own items: what runs on the row lies over none of them */
const BARE: ReadonlySet<NextStepKind> = new Set(["hidden", "none", "pending"])

/**
 * The next step `step` of a subject in the page's words (see `NextStepKind`): its items, why every Safe Push and Dry
 * Run can't start (a reason of the subject first, then what runs on the row, then the step's own), the notice of S, what
 * landed it. Pure: the context carries the words' facts and the actions, so every surface renders the same items.
 */
export function presentNextStep(step: NextStep, context: StepContext): BranchNextStep {
  const { busy, forStart } = context
  const running = busy === null ? null : `${busy.label}…`
  // a start reads only a fresh answer of the readiness: one still to come or a stale one holds nothing back, the start
  // waits for it itself
  const readinessWaits = forStart && (step.staleReadiness || (step.kind.kind === "pending" && step.kind.fact === "readiness"))
  const general = isOwn(step.safePush) ? running : formatStepBlocker(step.safePush!)
  const shown = show(step, context, general)
  const blockerOf = (blocker: StepBlocker | null): string | null => {
    if (!isOwn(blocker)) {
      return formatStepBlocker(blocker!)
    }
    if (running !== null || blocker === null || readinessWaits) {
      return running
    }
    return blocker.code === "staleReadiness" ? STALE_READINESS : (shown.own ?? null)
  }
  const safePushBlocker = blockerOf(step.safePush)
  const tone = shown.tone ?? "default"
  const stale = step.safePush?.code === "staleReadiness" && !readinessWaits
  const ownNotice = general !== null ? undefined : stale ? { title: `Safe Push of ${context.subject} waits for Patronus`, description: STALE_READINESS } : shown.notice
  const notice = safePushBlocker === null || shown.kind === "hidden" ? null : (ownNotice ?? { title: `Safe Push of ${context.subject} can't start`, description: safePushBlocker })
  let item = shown.item
  // a step of a stale answer stays, its action waits for the new one; a quiet step only opens the pull request
  if (item !== null && step.staleReadiness && !forStart && tone === "default" && item.disabled !== true) {
    item = { ...item, disabled: true, description: STALE_READINESS }
  }
  let retarget = shown.retarget ?? null
  if (busy !== null && !BARE.has(shown.kind)) {
    item = item === null ? null : withBusy(item, tone, busy)
    // the retarget the menu offers beside the step runs too
    retarget = retarget !== null && retarget.id === busy.id ? { ...retarget, running: true, disabled: true, description: running! } : retarget
  }
  return {
    kind: shown.kind,
    item,
    tone,
    safePushBlocker,
    dryRunBlocker: blockerOf(step.dryRun),
    subjectBlocker: general,
    needsReadiness: step.needsReadiness,
    waitsFor: step.kind.kind === "pending" ? step.kind.fact : null,
    notice,
    retarget,
    withoutReview: step.withoutReview === null ? null : { blocker: blockerOf(step.withoutReview.blocker) },
    landed: formatLandedWhole(step.landed),
    landingFix: shown.landingFix ?? null,
  }
}

/**
 * Why the step holds a start in `mode` back, as Rust's next step says it: a Dry Run by its own blocker; a Safe Push that
 * skips the approvals (`skipsApprovals`: Start Without Review, a profile known to skip the review) by the reasons of the
 * subject on every row (`subjectBlocker`: a sync, a failure, no commits, the robots, parked, landed, what runs on the
 * row; a decision has no fact of a sync), never by the step's own (the approvals it goes without, a draft or another
 * base no start through the API reads); its decision says the rest. The one rule of the click and of the read of a
 * start (`requestSafePush`, `readSafePush`)
 */
export function startStepBlocker(step: BranchNextStep, mode: "rebaseAutosquash" | "dryRun", skipsApprovals: boolean): string | null {
  if (mode === "dryRun") {
    return step.dryRunBlocker
  }
  return skipsApprovals ? step.subjectBlocker : step.safePushBlocker
}

/** The step of a subject the view has none of yet (no row): no slot, nothing to say against a start, which decides itself */
export const NO_NEXT_STEP: BranchNextStep = {
  kind: "hidden",
  item: null,
  tone: "default",
  safePushBlocker: null,
  dryRunBlocker: null,
  subjectBlocker: null,
  needsReadiness: false,
  waitsFor: null,
  notice: null,
  retarget: null,
  withoutReview: null,
  landed: null,
  landingFix: null,
}
