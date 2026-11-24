import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { getBranchNextStep } from "@/hooks/useBranchNextStep"
import { useSourceMemo } from "@/hooks/useSourceMemo"
import { runLandingFix } from "@/hooks/useStackActions"
import { getStackNextStep, wholeStackRestOf } from "@/hooks/useStackNextStep"
import { askUntilSettled } from "@/lib/ask-until-settled"
import type {
  ApiError,
  Confirmation,
  Land,
  LandingAllowances,
  LandingRefusal,
  LandProfiles,
  OperationEvent,
  OperationId,
  Outcome,
  PatronusProfile,
  PatronusRobot,
  Plan,
  ReviewReadiness,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure, notifyError } from "@/lib/errors"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { FROM_MENU } from "@/lib/menu"
import { formatMergeLabel, mergeOf } from "@/lib/merge-pull-request"
import { formatPushMode, openRobot } from "@/lib/patronus"
import { openPullRequest, openPullRequestUrl } from "@/lib/pull-request"
import { pushForAction, pushWholeStack } from "@/lib/push-run"
import { sameJson } from "@/lib/record"
import { repositoryName } from "@/lib/repository-config"
import { getRobotActionItemsNow } from "@/lib/robot-items"
import { showRobotEventToast } from "@/lib/robot-toast"
import type { ToastActionProps } from "@/lib/toast-message"
import type { StackLandedPart } from "@/lib/whole-stack"
import { formatReadinessSubject, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { demandReviewReadiness, useDemandReviewReadiness } from "@/model/demand"
import { executeIntent } from "@/model/intents"
import { getRowPart } from "@/model/store"
import type { LandPrompt } from "@/presenters/land"
import { acceptedOf, presentLand, startRouteOf } from "@/presenters/land"
import { formatLandingFix, landingFixOf } from "@/presenters/landing"
import type { BranchNextStep } from "@/presenters/next-step"
import { NO_NEXT_STEP, startStepBlocker } from "@/presenters/next-step"
import type { FollowingUpdate, LandToast } from "@/presenters/outcome"
import { describeFollowingUpdate, presentLandOutcome, presentLandRefusal, pushTargetOf } from "@/presenters/outcome"
import {
  branchSyncStore,
  getBranch,
  getLandingRoute,
  getLands,
  getStackBranch,
  getStackBranchOf,
  getStackNotBuilt,
  isForkingRoot,
  mainLineOf,
  structureIn,
  useLandingRoute,
} from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import {
  askStartProfiles,
  getBranchRobot,
  getPatronusGitHubRepository,
  getPushHold,
  getRobot,
  getStackPushHold,
  getStackRobot,
  isPatronusAvailable,
  patronusStore,
} from "@/stores/patronus"
import { askBranchRulesAgain, getPullRequest, getPullRequestListing, getStackPullRequest, pullRequestsStore } from "@/stores/pull-requests"
import { getRepositorySwitchReason, getSelectedRepositoryPath, isSelectedRepository } from "@/stores/repository"
import type { ReadinessEntry } from "@/stores/review-readiness"
import { getReadinessTarget, getReviewReadiness, reviewReadinessStore } from "@/stores/review-readiness"
import { readRobotDetails } from "@/stores/robot-details"
import type { SafePushAsk, SafePushMode, SafePushPlacement, StartPlace } from "@/stores/safe-push"
import { getLandRun, openSafePushPopover, safePushStore, setLandAsking, setLandProgress, startPlaceOf } from "@/stores/safe-push"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"
import { getShownBranchTableOrder } from "@/stores/stack-collapse"
import { runUpdateProjectOf, tellFollowingUpdate } from "@/stores/update-project"

/** What asks a start: its mode, how (a click, a key), where its popover opens, a profile chosen where it was asked */
export interface SafePushRequest {
  readonly mode: SafePushMode
  readonly via: "click" | "key"
  readonly placement?: SafePushPlacement
  /** A profile chosen in the menu or the popover; undefined: the default (the last robot's); null: the default checks */
  readonly profileId?: string | null
  /** A Safe Push without review, asked from the Create Pull Request dialog */
  readonly bypassReview?: boolean
  /**
   * A Safe Push with a review after landing (Safe Push and Request Review…): the GitHub logins it asks once the robot
   * merged, as the popover picked them; none refuses the start (`noReviewer`)
   */
  readonly requestReview?: readonly string[]
}

/**
 * The repository lands by a push of the copy into the target (`RepositoryView.route`, Rust's): its start is Push to
 * master, which runs without Patronus
 */
export function landsByPush(): boolean {
  return getLandingRoute()?.kind === "push"
}

/**
 * Rust offers the merge of the pull request of `key` (a branch, `stack:<root>`) on GitHub, or Cancel Auto-Merge: the
 * repository lands by a merge of the pull request, which runs without Patronus (`land::merges_on_github`, Rust's)
 */
export function landsByMerge(key: string): boolean {
  return (getRowPart(key, "actions") ?? []).some(action => action.id === "merge-pull-request" || action.id === "cancel-auto-merge")
}

/** `landsByPush` of a component, following the route of the view */
export function useLandsByPush(): boolean {
  return useLandingRoute()?.kind === "push"
}

/** What the texts call a start of `mode`: "Safe Push", "Dry Run", or "Push to master" where the repository lands by a push */
export function formatStartName(mode: SafePushMode): string {
  return mode === "rebaseAutosquash" && landsByPush() ? `Push to ${getReadinessTarget()}` : formatPushMode(mode)
}

/** "Cannot start the Safe Push of auth", "Cannot push auth into master" */
function startFailureTitle(mode: SafePushMode, subject: string): string {
  return mode === "rebaseAutosquash" && landsByPush() ? `Cannot push ${subject} into ${getReadinessTarget()}` : `Cannot start the ${formatPushMode(mode)} of ${subject}`
}

/** The code-owner review of a Safe Push as the popover says it, of the answer Patronus gave for the subject as it is */
export type ReviewShown =
  /** A dry run, which nobody reviews, or nothing to say yet (the footer says what the start waits for) */
  | { readonly kind: "none" }
  /** GitHub doesn't list the pull requests: the start is not known, Patronus judges the approvals at the start */
  | { readonly kind: "unlisted" }
  /** A repository only on Space: whether its merge request needs approvals */
  | { readonly kind: "space"; readonly required: boolean }
  | { readonly kind: "loaded"; readonly readiness: ReviewReadiness }

/** A start as the stores have it now: what Rust planned of it, what the click and the popover do, and what they show */
export interface SafePushRead {
  /** The branch, or the key of the whole stack of a root (`stackReadinessKey`) */
  readonly branchName: string
  /** What the texts call what starts: the branch's name, "the whole air stack" */
  readonly subject: string
  readonly mode: SafePushMode
  /**
   * It pushes the copy into the target (Push to master), no robot of Patronus: no profile, no review, no Dry Run; the
   * lines of what CI did not pass on the commit it lands and of the rules of the target instead
   */
  readonly byPush: boolean
  /** The target branch ("master") */
  readonly target: string
  /** What Rust planned of the start (`prepare`: its `Plan`); null while it is asked, and before the Patronus view */
  readonly plan: Plan | null
  /** What the click and the popover do with it (`presentLand`) */
  readonly prompt: LandPrompt
  /** The profiles the start offers (`Plan.alternatives`); null while the plan is asked */
  readonly profiles: LandProfiles | null
  /** The profile it runs with; null: the default checks, or a comment on a pull request */
  readonly profile: PatronusProfile | null
  /** The open pull request it comments on (`/patronus merge`); null: through the API */
  readonly fromPullRequest: { readonly number: number; readonly command: string } | null
  /** A Safe Push without review: its profile has to skip the code-owner review */
  readonly withoutReview: boolean
  /** The running Safe Push of an older commit it cancels */
  readonly replaces: PatronusRobot | null
  /** The branches it is built on that it lands with it, root first (`Plan.lands`) */
  readonly ancestors: readonly string[]
  /** The branches built on it: they stay */
  readonly children: readonly string[]
  /** The commits it lands: its own and its ancestors' */
  readonly commitCount: number
  /** The review of the subject as Patronus answered it for the subject as it is; null while it loads, for a dry run */
  readonly readiness: ReadinessEntry | null
  readonly review: ReviewShown
  /** The start of the whole stack of a root (`readWholeStack`); null for the start of a branch */
  readonly wholeStack: WholeStackRead | null
  /**
   * The open pull request of the whole stack the branch is in: its approvals count for the Safe Push of the whole stack
   * only, not for this one of the branch; null otherwise
   */
  readonly wholeStackPullRequest: { readonly root: string; readonly number: number } | null
}

/** What a start of the whole stack lands and pushes */
export interface WholeStackRead {
  readonly root: string
  /**
   * The branches of the stack, the main line first: one robot lands them all; but those merged Safe Pushes landed whole
   * already (`wholeStackRestOf`)
   */
  readonly members: readonly string[]
  /** The branches merged Safe Pushes landed whole already, and who landed them; null while none is */
  readonly landed: StackLandedPart | null
  /** Its stack branch `{prefix}/stack/<root>`, the only branch the start pushes */
  readonly stackBranch: string
  /** The branches of it on the remote, in the order of `members`: they stay as they are there */
  readonly onRemote: readonly string[]
  /**
   * While the sync didn't build the stack branch (`getStackNotBuilt`), the last branch of its main line: the stack
   * lands in pieces, the Safe Push of that branch the first; null otherwise
   */
  readonly piece: string | null
}

/** No reviewer picked yet */
const NO_REVIEWERS: readonly string[] = []

// ---- the profile of the last robot ----

/** The last robot of the branch, or of the whole stack of a key (`stackReadinessKey`): its profile is the default */
function lastRobotOf(branchName: string): PatronusRobot | undefined {
  const root = stackRootOfReadinessKey(branchName)
  return root === null ? getBranchRobot(branchName) : getStackRobot(root)
}

/** The robots whose details were read for their profile in this window: once each */
const profileReads = new Set<string>()

/**
 * The profile of the last robot of the branch, which a list doesn't name: read from its details once (the session reads
 * them while demanded and keeps the profile they name on the robot), as the default profile of a start is the last
 * robot's
 */
async function readLastRobotProfile(branchName: string): Promise<void> {
  const robot = lastRobotOf(branchName)
  if (robot === undefined || robot.profile?.id != null || profileReads.has(robot.id)) {
    return
  }
  profileReads.add(robot.id)
  await readRobotDetails(robot.id)
}

/** Asks what a start of the branch reads of its profiles: the merge profiles of the target, the last robot's profile */
function askStartOptions(branchName: string): Promise<unknown> {
  return Promise.all([askStartProfiles(), readLastRobotProfile(branchName)])
}

// ---- the read ----

/** The start `request` asks of `branchName` (a branch, or the key of a whole stack) as the contract names it (`Land`) */
export function landOf(branchName: string, request: Omit<SafePushRequest, "placement">): Land {
  const root = stackRootOfReadinessKey(branchName)
  const { profileId } = request
  return {
    subject: root === null ? { kind: "branch", name: branchName } : { kind: "wholeStack", root },
    mode: request.mode,
    profile: profileId === undefined ? { kind: "default" } : profileId === null ? { kind: "none" } : { kind: "id", id: profileId },
    withoutApprovals: request.bypassReview === true && request.mode === "rebaseAutosquash",
    ...(request.requestReview !== undefined && request.mode === "rebaseAutosquash" && { requestReview: { reviewers: [...request.requestReview] } }),
  }
}

/** The press a start `land` is of, `landOf` the other way: its mode, its profile, a start without the approvals */
function requestOf(land: Land): SafePushRequest {
  const { profile } = land
  return {
    mode: land.mode === "dryRun" ? "dryRun" : "rebaseAutosquash",
    via: "click",
    ...(profile.kind === "none" ? { profileId: null } : profile.kind === "id" ? { profileId: profile.id } : {}),
    ...(land.withoutApprovals === true && { bypassReview: true }),
    ...(land.requestReview != null && { requestReview: land.requestReview.reviewers }),
  }
}

/** The popover of the start `request` of `branchName`, at `placement` */
function askOf(branchName: string, request: Omit<SafePushRequest, "placement">, placement: SafePushPlacement): SafePushAsk {
  return {
    branchName,
    mode: request.mode,
    placement,
    ...(request.profileId !== undefined && { profileId: request.profileId }),
    ...(request.bypassReview === true && { bypassReview: true }),
    ...(request.requestReview !== undefined && { requestReview: true }),
  }
}

/** The start of the default intent (the default profile, with review): the view carries its decision (`RepositoryView.lands`) */
function isDefaultAsk(request: Omit<SafePushRequest, "placement">): boolean {
  return request.profileId === undefined && request.bypassReview !== true && request.requestReview === undefined
}

/** The plan the session answers of the start `request` of `branchName` (`prepare`); null without an answer */
export function prepareStart(branchName: string, request: Omit<SafePushRequest, "placement">): Promise<Plan | null> {
  return prepareLand(getSelectedRepositoryPath(), landOf(branchName, request))
}

/** The plan the session of `repositoryPath` answers of the start `land` (`prepare`); null without an answer */
async function prepareLand(repositoryPath: string, land: Land): Promise<Plan | null> {
  const result = await commands.prepare({ repositoryPath, intent: { kind: "land", ...land } })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}

/** The readiness of the subject as the session answered it for the subject as it is; null while it is asked again */
function shownReadiness(key: string): ReadinessEntry | null {
  const entry = getReviewReadiness(key)
  return entry === undefined || entry.stale ? null : entry
}

/** The start waits for the review: the pull request it starts from, or the answer of Patronus */
function waitsForReview(plan: Plan): boolean {
  return plan.waits.some(wait => wait.fact.kind === "readiness" || wait.fact.kind === "pullRequests")
}

/** The review as the popover says it: nothing for a dry run, nor while the start waits for the review */
function reviewShownOf(mode: SafePushMode, readiness: ReadinessEntry | null, plan: Plan | null): ReviewShown {
  if (mode === "dryRun" || plan === null || waitsForReview(plan)) {
    return { kind: "none" }
  }
  if (getPatronusGitHubRepository() === null) {
    // a repository only on Space: its merge request counts
    if (readiness === null) {
      return { kind: "none" }
    }
    return readiness.requirement === null ? { kind: "loaded", readiness: readiness.readiness } : { kind: "space", required: readiness.requirement.required }
  }
  if (getPullRequestListing() === "unlisted") {
    return { kind: "unlisted" }
  }
  return readiness === null ? { kind: "none" } : { kind: "loaded", readiness: readiness.readiness }
}

/**
 * The start of the whole stack of `root` as the popover names it: one robot from its stack branch, the only branch it
 * pushes; what merged Safe Pushes landed of it is in the target, the popover names the rest
 */
function readWholeStack(root: string, stackBranch: string): WholeStackRead {
  const piece = getStackNotBuilt(root) === null ? null : (mainLineOf(branchSyncStore.getState(), root).at(-1) ?? null)
  const { members, landed } = wholeStackRestOf(root)
  return { root, members, landed, stackBranch, onRemote: members.filter(name => getBranch(name)?.remoteStatus?.exists === true), piece }
}

/**
 * A start of the branch, or of the whole stack of a key (`stackReadinessKey`), as the stores have it now, presenting the
 * plan Rust answered of it (`prepare`; null while it is asked); null without the branch, or for a whole stack that
 * doesn't fork (a straight chain starts from its last branch). The one reader of a start: the click decides on it, the
 * popover renders it.
 */
export function readSafePush(branchName: string, request: Omit<SafePushRequest, "placement">, plan: Plan | null): SafePushRead | null {
  const root = stackRootOfReadinessKey(branchName)
  const state = branchSyncStore.getState()
  const { mode } = request
  // a subject the view has no step of yet holds nothing back here: the plan of the start waits or refuses
  let step: BranchNextStep
  let wholeStack: WholeStackRead | null = null
  if (root === null) {
    if (getBranch(branchName) === undefined) {
      return null
    }
    step = getBranchNextStep(branchName, { forStart: true }) ?? NO_NEXT_STEP
  } else {
    const stack = getStackBranch(root)
    if (stack === undefined || !isForkingRoot(state, root)) {
      return null
    }
    step = getStackNextStep(root, { forStart: true })?.step ?? NO_NEXT_STEP
    wholeStack = readWholeStack(root, stack.branchName)
  }
  const subject = root === null ? branchName : formatWholeStack(root)
  // a start that skips the approvals (`Plan.approvalsSkippedBy`) is held back by the reasons of the subject
  // (`subjectBlocker`: a sync, a failure, the robots, …), never by the approvals it goes without, as the click decides it
  const stepBlocker = startStepBlocker(step, mode, plan?.approvalsSkippedBy != null)
  const route = plan === null ? null : startRouteOf(plan)
  const profile = route?.kind === "api" ? route.profile : null
  const cancel = plan?.effects.find(effect => effect.kind === "cancelRobot")
  const readiness = mode === "rebaseAutosquash" ? shownReadiness(branchName) : null
  const ancestors = root === null ? (plan?.lands.slice(0, -1) ?? []) : []
  const stackPullRequest = plan?.confirmations.find(confirmation => confirmation.kind === "stackPullRequestApprovalsDontCount")
  const stackOf = stackPullRequest === undefined ? undefined : getStackBranchOf(branchName)
  const byPush = (plan !== null && pushTargetOf(plan.effects) !== null) || landsByPush() || landsByMerge(branchName)
  const review: ReviewShown = byPush ? { kind: "none" } : reviewShownOf(mode, readiness, plan)
  // asked without review through the API (a comment takes no profile), while the approvals are missing or not known
  const withoutReview =
    request.bypassReview === true && mode === "rebaseAutosquash" && route?.kind !== "comment" && !(review.kind === "loaded" && review.readiness.kind !== "missing")
  return {
    branchName,
    subject,
    mode,
    byPush,
    target: getReadinessTarget(),
    plan,
    prompt: presentLand(plan, { subject, via: request.via, stepBlocker, withoutReview }),
    profiles: plan?.alternatives ?? null,
    profile,
    fromPullRequest: route?.kind === "comment" ? { number: route.number, command: mode === "dryRun" ? "/patronus dry-run" : "/patronus merge" } : null,
    withoutReview,
    replaces: cancel?.kind === "cancelRobot" ? (getRobot(cancel.robotId) ?? null) : null,
    ancestors,
    children: root === null ? structureIn(state, branchName).descendants : [],
    commitCount: root === null ? [...ancestors, branchName].reduce((sum, name) => sum + (state.branches[name]?.commitCount ?? 0), 0) : wholeStackRestOf(root).commitCount,
    readiness,
    review,
    wholeStack,
    wholeStackPullRequest:
      stackPullRequest?.kind === "stackPullRequestApprovalsDontCount" && stackOf !== undefined ? { root: stackOf.root, number: stackPullRequest.number } : null,
  }
}

/** A press settled (`settleSafePush`): its read (null: the view has no subject of it), or ended by another repository */
type Settled = { readonly kind: "read"; readonly read: SafePushRead | null } | { readonly kind: "switched" }

/**
 * The start `land` of a press, planned again on every change until its plan waits for nothing (`prepare`): the session
 * answers every fact it waits for (the review it is asked at once, the rules, the profiles, the landing needs of tips a
 * sync moved), a failure too, so the wait ends. The rules come with the listing that has the approval: the session reads
 * them then. A fact may come while the plan is asked, its patch ahead of the plan that still waits for it:
 * `askUntilSettled` asks again then. It is planned in `repositoryPath`, the repository of the press, and read over the
 * table the window shows: once another repository opens, the wait ends (`switched`), also when the first one shows
 * again before the end, which the press says
 */
async function settleSafePush(branchName: string, request: Omit<SafePushRequest, "placement">, land: Land, repositoryPath: string): Promise<Settled> {
  // the recent projects, whose first one is the selected repository
  let switched = !isSelectedRepository(repositoryPath)
  const unsubscribe = repositorySettingsStore.subscribe(() => {
    switched ||= !isSelectedRepository(repositoryPath)
  })
  try {
    // the profiles as the session has them now, the rules GitHub didn't give read again: the start decides on new answers
    await Promise.all([askStartOptions(branchName), askBranchRulesAgain()])
    return await askUntilSettled<Settled>(
      async () => {
        const plan = switched ? null : await prepareLand(repositoryPath, land)
        return switched ? { kind: "switched" } : { kind: "read", read: readSafePush(branchName, request, plan) }
      },
      settled => settled.kind === "switched" || settled.read === null || settled.read.prompt.kind !== "wait",
      [repositorySettingsStore, branchSyncStore, reviewReadinessStore, pullRequestsStore, patronusStore],
    )
  } finally {
    unsubscribe()
  }
}

/**
 * A press of the start `land` (`ask.branchName`, as `request` asked it) settled (`settleSafePush`) and decided as every
 * press is, in `repositoryPath`, the repository of the press: its button says "Checking…" while it waits, the review of
 * a Safe Push asked meanwhile; once another repository opened, it starts nothing and says so. `start` runs the plan, and
 * so does a confirm of what the press showed before (`shown`: Start Anyway, the confirmations its popover or toast
 * showed, which it accepts, `acceptedOf`); a refusal says why (the step's notice, the popover of the approvals it
 * misses, its toast); a choice, a confirm and a confirmation it didn't show open the popover. Resolves to whether its
 * robot started
 */
async function settleStart(ask: SafePushAsk, request: SafePushRequest, land: Land, repositoryPath: string, shown: readonly Confirmation[] | null): Promise<boolean> {
  const { branchName, mode } = ask
  const title = startFailureTitle(mode, formatReadinessSubject(branchName))
  setLandAsking(branchName, mode, repositoryPath)
  const release = mode === "rebaseAutosquash" ? demandReviewReadiness(branchName) : () => {}
  let settled: Settled
  try {
    settled = await settleSafePush(branchName, request, land, repositoryPath)
  } catch (error) {
    notifyError(title, error)
    return false
  } finally {
    release()
    setLandAsking(branchName, null, repositoryPath)
  }
  // the plan was read in the session of the repository of the press, over its table: another one decides nothing here,
  // nor the first one shown again, whose table the press didn't wait on
  if (settled.kind === "switched") {
    notifyError(title, getRepositorySwitchReason(repositoryPath) ?? `Another repository opened while the start in ${repositoryName(repositoryPath)} waited`)
    return false
  }
  const { read } = settled
  if (read === null) {
    return false
  }
  const decision = read.prompt
  const accepted = decision.kind === "confirm" && shown !== null && read.plan !== null ? acceptedOf(read.plan, shown) : null
  if (decision.kind === "start" || accepted !== null) {
    return read.plan !== null && startShown(read.plan, accepted ?? read.plan.confirmations, repositoryPath)
  }
  if (decision.kind !== "refuse") {
    openSafePushPopover(ask)
    return false
  }
  // the answer the press waited for may be the step's own reason (the approvals): its notice says it, with its action
  const step = mode === "rebaseAutosquash" && !decision.offersPush ? currentStepOf(branchName) : undefined
  if (step?.notice != null && step.safePushBlocker === decision.reason) {
    tellNotice(branchName, step, step.notice, { ...request, placement: ask.placement })
  } else if (decision.opens === "review") {
    // the approvals it misses: the popover names them, where they count and how to get them
    openSafePushPopover(ask)
  } else {
    tellRefusal(branchName, mode, decision.reason, decision.offersPush, decision.opens)
  }
  return false
}

/**
 * The toast of a start that didn't run, of the robots of its subject (`toastKeys.robotSubject`): a later start takes its
 * place. Its step closes it when pressed (`closes`): the push alone, a step of the start with no toast of its own, which
 * the row shows, would leave a Push behind that pushes nothing
 */
function showStartToast(branchName: string, mode: SafePushMode, title: string, description: string, action: ToastActionProps | undefined, closes = false): void {
  const key = toastKeys.robotSubject(getSelectedRepositoryPath(), branchName, mode)
  const step =
    action === undefined
      ? undefined
      : {
          label: action.children,
          onClick: () => {
            if (closes) {
              closeEventToasts(key)
            }
            action.onClick()
          },
        }
  showEventToast({ key, render: () => ({ type: "info", title, description, ...(step !== undefined && { action: step }) }) })
}

/**
 * A start that can't run: the reason as an `info` toast (a precondition); the push alone where it is the step (of the
 * stack branch for the whole stack, `pushAlone`), which closes the toast
 */
function tellRefusal(branchName: string, mode: SafePushMode, reason: string, offersPush: boolean, opens?: "pullRequest" | "robot"): void {
  let action: ToastActionProps | undefined
  if (offersPush) {
    action = { children: "Push Branch", onClick: () => void pushAlone(branchName) }
  } else if (opens !== undefined) {
    action = refusalObjectAction(branchName, opens)
  }
  const subject = formatReadinessSubject(branchName)
  const title =
    mode === "rebaseAutosquash" && landsByPush() ? `The push of ${subject} into ${getReadinessTarget()} can't start` : `${formatPushMode(mode)} of ${subject} can't start`
  showStartToast(branchName, mode, title, reason, action, offersPush)
}

/** The action that opens what a refusal names: the pull request of a draft, the robot that holds the branch */
function refusalObjectAction(branchName: string, opens: "pullRequest" | "robot"): ToastActionProps | undefined {
  const root = stackRootOfReadinessKey(branchName)
  if (opens === "pullRequest") {
    const pullRequest = root === null ? getPullRequest(branchName) : getStackPullRequest(root)
    return pullRequest === undefined ? undefined : { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
  }
  const hold = root === null ? getPushHold(branchName) : getStackPushHold(root)
  return hold === null ? undefined : { children: "Open in Patronus", onClick: () => openRobot(hold.robot) }
}

/** The push alone before a start: of the branch (its stack first), or of the stack branch of the whole stack of a key */
export async function pushAlone(branchName: string): Promise<boolean> {
  const root = stackRootOfReadinessKey(branchName)
  if (root !== null) {
    return pushWholeStack(root)
  }
  // a push that failed or stopped while it waited for the lock said so itself
  return pushForAction(branchName)
}

/**
 * The `info` toast of a start the next step refuses (`BranchNextStep.notice`): the pull request it waits in opens from
 * it, and the step the user takes runs from it, the step's own action: "Create Pull Request…" (row 7, "Create Pull
 * Request of the Stack…"), "Request Review…" (row 9), "Retarget #7 to master" (row 5); the whole stack whose stack
 * branch the sync didn't build lands in pieces, the Safe Push of the last branch of its main line the first
 * (`WholeStackRead.piece`)
 */
function tellNotice(branchName: string, step: BranchNextStep, notice: { readonly title: string; readonly description: string }, request: SafePushRequest): void {
  const root = stackRootOfReadinessKey(branchName)
  const pullRequest = root === null ? getPullRequest(branchName) : getStackPullRequest(root)
  const piece = root === null || getStackNotBuilt(root) === null ? undefined : mainLineOf(branchSyncStore.getState(), root).at(-1)
  const item = step.item
  let action: { children: string; onClick: () => void } | undefined
  if (piece !== undefined) {
    action = { children: `Safe Push ${piece}`, onClick: () => void requestSafePush(piece, { ...request, via: "click" }) }
  } else if (pullRequest !== undefined && step.tone !== "default") {
    action = { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
  } else if (step.tone === "default" && item !== null && item.disabled !== true) {
    action = { children: item.label, onClick: () => item.onSelect(FROM_MENU) }
  } else {
    // nothing to take: a robot that runs on it or lands it holds the start back, the notice opens it
    action = refusalObjectAction(branchName, "robot")
  }
  showStartToast(branchName, request.mode, notice.title, notice.description, action)
}

/**
 * The next step of a start as the stores have it now: of the branch, or of the whole stack of a key
 * (`stackReadinessKey`)
 */
function currentStepOf(branchName: string): BranchNextStep | undefined {
  const root = stackRootOfReadinessKey(branchName)
  return (root === null ? getBranchNextStep(branchName) : getStackNextStep(root)?.step) ?? undefined
}

/**
 * A start of a Safe Push or a Dry Run, asked from any surface: the slot of the row or of the island header, the branch
 * menu, S, the preview, the review window, a toast. What every surface knows refuses it first, as the step says it (an
 * `info` toast), a start another process runs on the row too ("`bd` (pid 4242) runs the Safe Push of auth…"). S opens the popover at once, which reads the facts as they load. A click waits for the facts, its
 * button busy ("Checking…"), and then starts at once when there is nothing to say (`presentLand` of Rust's plan), else opens
 * the popover at the slot of the branch (a hidden row revealed first), or says why it can't start.
 */
export async function requestSafePush(branchName: string, request: SafePushRequest): Promise<void> {
  // the start is planned and run in the repository of the click: what it reads meanwhile decides on it, and it starts
  // there
  const repositoryPath = getSelectedRepositoryPath()
  // the row of the start, the branch, the root for the whole stack: of an island at its header, of a small stack at the
  // root's row
  const place = startPlaceOf(branchName, request.placement)
  if (place === null) {
    return
  }
  if (place.subject !== branchName) {
    // a straight chain has no stack branch: the Safe Push of its last branch lands the whole stack, at the header of its
    // island as the island's own Safe Push
    await requestSafePush(place.subject, { ...request, placement: place.placement })
    return
  }
  const { row, placement } = place
  const root = stackRootOfReadinessKey(branchName)
  const branch = getBranch(row)
  const run = getLandRun(branchName)
  // a start of the row that runs shows in its slot: a second ask has nothing to add
  // Patronus lands it, or a push of its copy into the target (which asks no Patronus)
  if (branch === undefined || !(isPatronusAvailable() || landsByPush() || landsByMerge(branchName)) || (run !== null && run.by === null)) {
    return
  }
  const { mode } = request
  if (run?.by != null) {
    // another process runs a start of the row (an agent's `brd safe-push`): it holds what it lands until it ends
    tellRefusal(branchName, mode, run.by.description, false)
    return
  }
  const step = root === null ? getBranchNextStep(row) : getStackNextStep(root)?.step
  if (step == null) {
    return
  }
  // a start that skips the approvals (Start Without Review, or the default start whose decision skips them: a profile
  // known to skip the review) is held back by the reasons of the subject on every row (`subjectBlocker`: a sync, a
  // failure, the robots, parked, landed, what runs on the row), never by the approvals it goes without, as its read
  // decides it
  const lands = getLands(branchName)
  const skipsApprovals = request.bypassReview === true || (isDefaultAsk(request) && lands?.safePush.approvalsSkippedBy != null)
  const blocker = startStepBlocker(step, mode, skipsApprovals)
  if (blocker !== null) {
    if (mode === "rebaseAutosquash" && step.notice !== null) {
      // a step that waits in the pull request (the approvals, a draft, another base) opens it from the notice
      tellNotice(branchName, step, step.notice, { ...request, placement })
    } else {
      // a robot that runs on it or lands it holds the start back: the refusal opens it
      tellRefusal(branchName, mode, blocker, false, "robot")
    }
    return
  }
  // the popover and the progress show at the row's slot: a row a collapsed stack or the filter hides is revealed first
  revealStartRow(place)
  // a failed read of the rules is asked again by each start: the click waits for the new answer, the popover reads anew
  // (the open is a new state of the store of the starts, which its read is memoized on)
  void askBranchRulesAgain()
  const ask = askOf(branchName, request, placement)
  // its rarer forms ask first: the profile, the reviewers
  if (request.via === "key" || request.bypassReview === true || request.requestReview !== undefined) {
    openSafePushPopover(ask)
    return
  }
  await settleStart(ask, request, landOf(branchName, request), repositoryPath, null)
}

/** The row of a start that shows at its slot (`StartPlace`), revealed first where a collapsed stack or the filter hides it */
function revealStartRow(place: StartPlace): void {
  if (place.placement === "row" && !getShownBranchTableOrder().includes(place.row)) {
    requestReveal({ branchName: place.row, commitIds: [] })
  }
}

/**
 * The fix of what a start of `branchName` needs, the fix of `refusal` ("Stack lane-run on stills-codec"), pressed on the
 * slot of row 1′, its item in "⋯" or the toast of a refused start: a rewrite that names the refusal it fixes
 * (`RewriteIntent.fixes`), which the engine runs on fresh landing needs only, while they still refuse with it; refused
 * (`plan_changed`: the fresh needs ask for it no more, a second press after the first ran), the Safe Push popover opens
 * at the row with the needs as they are. The start of a cycle's fix is a click of the Safe Push of what lands both; a
 * fix no rewrite runs here (the group of an unassigned commit) opens the popover at once. The
 * whole stack reads no needs of the view (the gate of its start checks what it lands): its fix runs as asked
 */
export function requestLandingFix(branchName: string, refusal: LandingRefusal): void {
  const fix = landingFixOf(refusal)
  // the popover itself, whatever the step holds back now (a readiness asked again for the tip that moved): it reads the
  // start with the needs as they are, and says what it waits for; where the start of the subject shows (the whole stack
  // at the header of its island)
  const popover = () => {
    const place = startPlaceOf(branchName)
    if (place !== null) {
      revealStartRow(place)
      openSafePushPopover({ branchName: place.subject, mode: "rebaseAutosquash", placement: place.placement })
    }
  }
  // a cycle: the start of what lands both, a click of its Safe Push
  if (fix?.kind === "start") {
    void requestSafePush(fix.subject.kind === "branch" ? fix.subject.name : stackReadinessKey(fix.subject.root), { mode: "rebaseAutosquash", via: "click" })
    return
  }
  const fixes = stackRootOfReadinessKey(branchName) === null ? { fixes: refusal } : {}
  if (fix == null || !runLandingFix(fix, { ...fixes, onPlanChanged: popover })) {
    popover()
  }
}

// ---- the run ----

/** The subject of a start: the branch, or the key of the whole stack of a root (`stackReadinessKey`) */
function landKey(land: Land): string {
  return land.subject.kind === "branch" ? land.subject.name : stackReadinessKey(land.subject.root)
}

/**
 * Runs the start Rust planned (`execute` of the intent of `plan` on its fingerprint, every confirmation of it accepted:
 * the popover showed them, or the click had none) in `repositoryPath`, the repository it was planned in: the session
 * admits it and shows its steps in the slot of every window (`RepositoryView.operations`), the progress of its push and
 * whom it waits for come on its channel; then its toast and Activity from the outcome (`presentLandOutcome`), or the
 * refusal before its first effect (`presentLandRefusal`). `anyway`: Start Anyway, the same start with the overrides of
 * what the landing gate refused, settled and decided again first as a click is (`startAnyway`). Resolves to whether its
 * robot started
 */
export function startPlanned(plan: Plan, anyway?: StartAnyway, repositoryPath: string = getSelectedRepositoryPath()): Promise<boolean> {
  return anyway === undefined ? startShown(plan, plan.confirmations, repositoryPath) : startAnyway(plan, anyway, repositoryPath)
}

/** `startPlanned` of `plan` as planned, the user having accepted `accepted`, which hold every confirmation of it */
async function startShown(plan: Plan, accepted: readonly Confirmation[], repositoryPath: string): Promise<boolean> {
  const { intent } = plan
  if (intent.kind !== "land") {
    return false
  }
  // "Checking…" until the session admits its operation, whose step the slot shows then
  const key = landKey(intent)
  setLandAsking(key, intent.mode === "dryRun" ? "dryRun" : "rebaseAutosquash", repositoryPath)
  return runPlanned(plan, intent, accepted, repositoryPath).finally(() => setLandAsking(key, null, repositoryPath))
}

/**
 * Start Anyway of the start of `plan`, which its popover or toast showed: with the overrides of what the landing gate
 * refused, which lift only the refusals it showed and those a Start Anyway before lifted (one more, of newer needs, joins
 * them, never takes their place), settled and decided as a click is (`settleStart`): it waits for the facts a click
 * waits for (the landing needs of tips a sync moved, the review asked again for a tip that moved), starts nothing once
 * another repository opened, says a refusal as the click says it, and accepts only the confirmations of `plan`: one
 * more (a robot it would cancel now) opens the popover, which asks again
 */
async function startAnyway(plan: Plan, anyway: StartAnyway, repositoryPath: string): Promise<boolean> {
  const { intent } = plan
  if (intent.kind !== "land") {
    return false
  }
  const land: Land = { ...intent, allow: { ...intent.allow, ...anyway.allow }, anyway: withRefusals(intent.anyway ?? [], anyway.refusals) }
  const key = landKey(land)
  const request = requestOf(land)
  const place = startPlaceOf(key, anyway.placement)
  if (place !== null) {
    revealStartRow(place)
  }
  return settleStart(askOf(key, request, place?.placement ?? "row"), request, land, repositoryPath, plan.confirmations)
}

/** `shown`, then each refusal of `more` it doesn't hold yet */
function withRefusals(shown: readonly LandingRefusal[], more: readonly LandingRefusal[]): LandingRefusal[] {
  return [...shown, ...more.filter(refusal => !shown.some(it => sameJson(it, refusal)))]
}

/**
 * Start Anyway: the overrides of the landing gate (`LandingAllowances`) and the refusals the user saw, which they lift
 * only (`Land.anyway`): a refusal of newer needs refuses the start, whose toast asks again
 */
export interface StartAnyway {
  readonly allow: LandingAllowances
  readonly refusals: readonly LandingRefusal[]
  /** Where its popover asked it; undefined from a toast: where the start of its subject shows (`startPlaceOf`) */
  readonly placement?: SafePushPlacement
}

/** `startShown` of the start `intent` of `planned`, `accepted` the confirmations the user accepted */
async function runPlanned(planned: Plan, intent: Land, accepted: readonly Confirmation[], repositoryPath: string): Promise<boolean> {
  const events = new Channel<OperationEvent>()
  let op: OperationId | null = null
  // a push into the target tells what it pushed at its end, then the update that follows it on the same channel
  const push = pushTargetOf(planned.effects) === null ? null : followPush(planned, intent, repositoryPath)
  events.onmessage = event => {
    if (event.kind === "progress" && (op === null || op === event.op)) {
      op = event.op
      setLandProgress(event.op, { percent: event.percent, line: event.line, transferred: null, speed: null })
    } else if (event.kind === "done") {
      push?.done(event.outcome)
    }
  }
  const params = { repositoryPath, intent: planned.intent, fingerprint: planned.fingerprint, accepted: [...accepted] }
  const result = await executeIntent(params, events).finally(() => {
    if (op !== null) {
      setLandProgress(op, null)
    }
  })
  if (result.status === "error") {
    // after the end of the push the error is of the update that follows it (or of the call itself, the IPC), which its
    // toast tells; before, a refusal
    if (push?.failed(result.error) !== true) {
      tellStartRefusal(result.error, planned, repositoryPath)
    }
    return false
  }
  setLandProgress(result.data.op, null)
  return push === null ? tellOutcome(result.data, planned, repositoryPath) : push.answered(result.data)
}

/** The commits a start of `land` lands, as the table counts them now: of the branches of its plan, of the rest of a whole stack */
function landedCommitCount(plan: Plan, land: Land): number {
  if (land.subject.kind === "wholeStack") {
    return wholeStackRestOf(land.subject.root).commitCount
  }
  const branches = branchSyncStore.getState().branches
  return plan.lands.reduce((sum, name) => sum + (branches[name]?.commitCount ?? 0), 0)
}

/** What the run of a push into the target hears on its channel: its own end, then the update that follows it */
interface PushFollow {
  /** A `done` of the channel: the push's, then the update's */
  done(outcome: Outcome): void
  /** The answer of `execute`, after the update: what the channel missed; resolves to whether it landed */
  answered(outcome: Outcome): boolean
  /**
   * `execute` failed (an error answered after the end of the push, or the call itself): an update that runs ends with it
   * in the toast of the push, which keeps no "Rebuilding on the new master…" to close by hand; false where no update
   * runs (no push told yet: a refusal of the start), which the caller says
   */
  failed(error: ApiError): boolean
}

/**
 * The toast of a push of the copy into the target (Push to master), one per push: "Pushed auth into master · 3
 * commits", or of a target that moved meanwhile "master moved: auth is rebuilt on the new commits" (nothing pushed), each
 * with the update that follows it in place ("Rebuilding on the new master…", which stays while the update runs, then what
 * it did, the push again after a moved target; Update Project when it did not move the branch); a refusal of the server
 * in its words (`notifyError`)
 */
function followPush(plan: Plan, land: Land, repositoryPath: string): PushFollow {
  const key = landKey(land)
  const subject = formatReadinessSubject(key)
  const commits = landedCommitCount(plan, land)
  const target = pushTargetOf(plan.effects) ?? getReadinessTarget()
  const toastKey = toastKeys.robotSubject(repositoryPath, key, land.mode)
  let pushed: LandToast | null = null
  let update: FollowingUpdate | null = null
  const show = () => {
    if (pushed === null || (pushed.kind !== "pushed" && pushed.kind !== "moved") || update === null) {
      return
    }
    const following = describeFollowingUpdate(update)
    const moved = pushed.kind === "moved"
    // a moved target: the update builds the copy anew, whose push the step offers again
    const description = !moved
      ? following.description
      : update.kind === "running" || following.offersUpdate
        ? `Nothing was pushed · ${following.description}`
        : `${following.description} · Push to ${target} again`
    const title = pushed.title
    // while the update runs the toast stays: its end comes into the same toast, however long the update takes, and an
    // end that came after the time of a toast opened a second one beside it as the first went
    const running = update.kind === "running"
    showEventToast({
      key: toastKey,
      supersedes: [toastKeys.update(repositoryPath)],
      render: () => ({
        type: moved ? "info" : "success",
        title,
        description,
        ...(running && { timeout: 0 }),
        ...(following.offersUpdate && { action: { label: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) } }),
      }),
    })
  }
  const tellPush = (outcome: Outcome) => {
    pushed = presentLandOutcome(outcome, plan.intent, subject, commits)
    if (pushed.kind === "pushed" || pushed.kind === "moved") {
      // Update Project follows both (`land_direct::update_after`)
      update = { kind: "running" }
      show()
    } else {
      // the server refused it, or it stopped: as every start says it
      tellOutcome(outcome, plan, repositoryPath, pushed)
    }
  }
  return {
    done(outcome) {
      if (pushed === null) {
        tellPush(outcome)
      } else if (update?.kind === "running") {
        const followed = tellFollowingUpdate(repositoryPath, outcome)
        if (followed !== null) {
          update = followed
          show()
        }
      }
    },
    answered(outcome) {
      if (pushed === null) {
        tellPush(outcome)
      }
      // an update follows and told no end: it did not run (refused before its first effect)
      if (update?.kind === "running") {
        update = { kind: "failed", error: null }
        show()
      }
      return pushed?.kind === "pushed"
    },
    failed(error) {
      if (update?.kind !== "running") {
        return false
      }
      update = { kind: "failed", error }
      show()
      return true
    },
  }
}

/**
 * The toast of how a start ended (`presentLandOutcome`), the next state of the event of the robots of its subject: the
 * robot it started (Open in Patronus, Cancel Safe Push), the comment Patronus accepted, or why the robot did not start,
 * "Start Safe Push" again when the facts changed after the push. Resolves to whether its robot started
 */
function tellOutcome(outcome: Outcome, plan: Plan, repositoryPath: string, presented?: LandToast): boolean {
  const land = plan.intent.kind === "land" ? plan.intent : null
  if (land === null) {
    return false
  }
  const key = landKey(land)
  const toast = presented ?? presentLandOutcome(outcome, plan.intent, formatReadinessSubject(key))
  const pushInto = pushTargetOf(plan.effects)
  const merge = mergeOf(plan.effects)
  const startName = merge !== null ? formatMergeLabel(merge.number, merge.auto) : pushInto === null ? `Start ${formatPushMode(land.mode)}` : `Push to ${pushInto}`
  const subjectKey = toastKeys.robotSubject(repositoryPath, key, land.mode)
  switch (toast.kind) {
    case "started":
      // a start asked by mistake costs only the robot: its toast cancels it while it can be canceled
      showRobotEventToast({
        repositoryPath,
        robot: toast.robot,
        type: "success",
        title: toast.title,
        description: toast.description,
        stepLast: true,
        stepOf: current => getRobotActionItemsNow(current).find(action => action.id === "cancel-robot") ?? null,
      })
      return true
    case "commented":
      showEventToast({
        key: subjectKey,
        supersedes: land.mode === "dryRun" ? [] : [toastKeys.approval(repositoryPath, key)],
        render: () => ({
          type: "success",
          title: toast.title,
          description: toast.description,
          action: { label: `Open Pull Request #${toast.number}`, onClick: () => openPullRequestUrl(toast.url) },
        }),
      })
      return true
    case "stopped":
    case "moved":
      showEventToast({ key: subjectKey, render: () => ({ type: "info", title: toast.title, description: toast.description }) })
      return false
    case "pushed":
      showEventToast({ key: subjectKey, render: () => ({ type: "success", title: toast.title, description: toast.description }) })
      return true
    case "autoMerge": {
      // one toast of the merge of the pull request: the news of the session says it in place once GitHub merged it
      const number = toast.number
      showEventToast({
        key: toastKeys.autoMerge(repositoryPath, number),
        render: () => ({ type: "success", title: toast.title, description: toast.description }),
      })
      return true
    }
    case "unknown": {
      // the row shows the start as not known yet, with Forget This Start; the toast opens where its robot would be
      const url = toast.url
      showEventToast({
        key: subjectKey,
        render: () => ({
          type: "warning",
          title: toast.title,
          description: toast.description,
          ...(url !== null && { action: { label: "Open Pull Request", onClick: () => openPullRequestUrl(url) } }),
        }),
      })
      return false
    }
    case "failed": {
      const error: ApiError | string = toast.error === null ? toast.description : { ...toast.error, message: toast.description }
      const canceled = toast.canceled
      const opens = canceled === null ? undefined : { label: "Open in Patronus", onClick: () => openRobot(canceled) }
      if (toast.again) {
        notifyError(toast.title, error, { children: startName, onClick: () => void startAgain(key, land, repositoryPath) }, opens)
      } else {
        notifyError(toast.title, error, opens === undefined ? undefined : { children: opens.label, onClick: opens.onClick })
      }
      return false
    }
  }
}

/** "Start Safe Push" of a start the facts changed under: a new start of the subject, planned on the table as it is now */
function startAgain(key: string, land: Land, repositoryPath: string): Promise<void> {
  if (!isSelectedRepository(repositoryPath)) {
    notifyError(startFailureTitle(land.mode === "dryRun" ? "dryRun" : "rebaseAutosquash", formatReadinessSubject(key)), getRepositorySwitchReason(repositoryPath))
    return Promise.resolve()
  }
  return requestSafePush(key, { mode: land.mode === "dryRun" ? "dryRun" : "rebaseAutosquash", via: "click" })
}

/**
 * The toast of a start refused before its first effect (`presentLandRefusal`): the landing gate with its fix ("Stack
 * lane-run on stills-codec") and Start Anyway, a conflict with the newest target with Update Project of the repository
 * of the start (a fetch only, once the main window shows another one), a plan the facts changed with the start again
 */
function tellStartRefusal(error: ApiError, plan: Plan, repositoryPath: string): void {
  const land = plan.intent.kind === "land" ? plan.intent : null
  if (land === null) {
    return
  }
  const key = landKey(land)
  const pushInto = pushTargetOf(plan.effects)
  const refusal = presentLandRefusal(error, land.mode, formatReadinessSubject(key), land.allow ?? {}, pushInto)
  switch (refusal.kind) {
    case "landing": {
      const anyway = { label: "Start Anyway", onClick: () => void startPlanned(plan, { allow: refusal.anyway, refusals: refusal.refusals }, repositoryPath) }
      const fix = refusal.fix
      const fixLabel = formatLandingFix(fix)
      const failure = { ...error, message: refusal.reason }
      if (fix !== null && fixLabel !== null) {
        notifyError(refusal.title, failure, { children: fixLabel, onClick: () => requestLandingFix(key, refusal.refusals[0]!) }, anyway)
      } else {
        notifyError(refusal.title, failure, { children: anyway.label, onClick: anyway.onClick })
      }
      return
    }
    case "target":
      notifyError(refusal.title, refusal.error, { children: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) })
      return
    case "changed":
      notifyError(refusal.title, refusal.error, {
        children: pushInto === null ? `Start ${formatPushMode(land.mode)}` : `Push to ${pushInto}`,
        onClick: () => void startAgain(key, land, repositoryPath),
      })
      return
    case "stopped":
      showEventToast({ key: toastKeys.robotSubject(repositoryPath, key, land.mode), render: () => ({ type: "info", title: refusal.title, description: refusal.description }) })
      return
    case "error":
      notifyError(refusal.title, refusal.error)
  }
}

/**
 * The plan the session answers of the start of the popover (`prepare`), asked again with every patch of the view (its
 * facts are the session's) and every other profile; the last plan while the next is asked, null before the first
 */
function useStartPlan(branchName: string, request: Omit<SafePushRequest, "placement">): Plan | null {
  const [answer, setAnswer] = useState<{ readonly key: string; readonly plan: Plan | null } | null>(null)
  const version = useStore(branchSyncStore, state => (state.viewVersion === null ? null : `${state.viewVersion.epoch}:${state.viewVersion.seq}`))
  // the reviewers by value: a picker gives a new list of the same logins on a render
  const reviewers = request.requestReview?.join("\n") ?? null
  const key = JSON.stringify([branchName, request.mode, request.profileId ?? "default", request.bypassReview === true, reviewers])
  const { mode, via, profileId, bypassReview } = request
  useEffect(() => {
    let current = true
    const requestReview = reviewers === null ? undefined : reviewers === "" ? [] : reviewers.split("\n")
    const asked = {
      mode,
      via,
      ...(profileId !== undefined && { profileId }),
      ...(bypassReview === true && { bypassReview }),
      ...(requestReview !== undefined && { requestReview }),
    }
    prepareStart(branchName, asked).then(
      plan => current && setAnswer({ key, plan }),
      (error: unknown) => current && notifyError(`Cannot read the ${formatStartName(mode)} of ${formatReadinessSubject(branchName)}`, error),
    )
    return () => {
      current = false
    }
  }, [branchName, key, mode, via, profileId, bypassReview, reviewers, version])
  // a plan of another profile is not this start's
  return answer?.key === key ? answer.plan : null
}

/**
 * `readSafePush` of the popover's ask, read again on every change of the stores it reads and of the plan of the
 * session; the profiles and the review demanded meanwhile. `reviewers`: whom the review after landing of a Safe Push
 * and Request Review… asks, as its popover picked them
 */
export function useSafePushRead(ask: SafePushAsk, profileId: string | null | undefined, reviewers: readonly string[] = NO_REVIEWERS): SafePushRead | null {
  useEffect(() => {
    void askStartOptions(ask.branchName)
  }, [ask.branchName])
  useDemandReviewReadiness(ask.branchName, ask.mode === "rebaseAutosquash")
  const request: Omit<SafePushRequest, "placement"> = {
    mode: ask.mode,
    via: "key",
    bypassReview: ask.bypassReview,
    ...(profileId !== undefined && { profileId }),
    ...(ask.requestReview === true && { requestReview: reviewers }),
  }
  const plan = useStartPlan(ask.branchName, request)
  // the read flows out of the subscriptions: a value read beside them would be memoized by the compiler. One read per
  // change of what it reads (the stores, the plan of the session), the same object for the same state, shared by the
  // subscriptions of this popover alone (`useSourceMemo`): another popover keeps its own
  const memo = useSourceMemo<SafePushRead | null>()
  const select = () => {
    const stores = [branchSyncStore.getState(), reviewReadinessStore.getState(), pullRequestsStore.getState(), patronusStore.getState(), safePushStore.getState()]
    return memo([ask.branchName, request.mode, request.via, request.profileId, request.bypassReview, request.requestReview?.join("\n"), plan, ...stores], () =>
      readSafePush(ask.branchName, request, plan),
    )
  }
  useStore(branchSyncStore, select)
  useStore(reviewReadinessStore, select)
  useStore(pullRequestsStore, select)
  useStore(patronusStore, select)
  return useStore(safePushStore, select)
}
