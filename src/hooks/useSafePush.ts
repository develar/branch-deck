import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { getBranchNextStep } from "@/hooks/useBranchNextStep"
import { restackBranch } from "@/hooks/useStackActions"
import { getStackNextStep, wholeStackRestOf } from "@/hooks/useStackNextStep"
import type { ApiError, Land, LandingAllowances, LandProfiles, OperationEvent, OperationId, Outcome, PatronusProfile, PatronusRobot, Plan, ReviewReadiness } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure, notifyError } from "@/lib/errors"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { FROM_MENU } from "@/lib/menu"
import { formatPushMode, openRobot } from "@/lib/patronus"
import { openPullRequest, openPullRequestUrl } from "@/lib/pull-request"
import { pushForAction, pushWholeStack } from "@/lib/push-run"
import { showRobotEventToast } from "@/lib/robot-toast"
import { executeIntent } from "@/lib/run-intent"
import type { ToastActionProps } from "@/lib/toast-message"
import type { StackLandedPart } from "@/lib/whole-stack"
import { formatReadinessSubject, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { demandReviewReadiness, useDemandReviewReadiness } from "@/model/demand"
import { getRobotActionItems } from "@/presenters/actions"
import type { LandPrompt } from "@/presenters/land"
import { presentLand, startRouteOf } from "@/presenters/land"
import { formatLandingFix } from "@/presenters/landing"
import type { BranchNextStep } from "@/presenters/next-step"
import { NO_NEXT_STEP, startStepBlocker } from "@/presenters/next-step"
import { presentLandOutcome, presentLandRefusal } from "@/presenters/outcome"
import {
  branchSyncStore,
  getBranch,
  getLands,
  getStackBranch,
  getStackBranchOf,
  getStackIslandRoots,
  getStackNotBuilt,
  isForkingRoot,
  mainLineOf,
  structureIn,
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
import type { SafePushAsk, SafePushMode, SafePushPlacement } from "@/stores/safe-push"
import { getLandRun, openSafePushPopover, safePushStore, setLandAsking, setLandProgress } from "@/stores/safe-push"
import { getShownBranchTableOrder } from "@/stores/stack-collapse"
import { runUpdateProjectOf } from "@/stores/update-project"

/** What asks a start: its mode, how (a click, a key), where its popover opens, a profile chosen where it was asked */
export interface SafePushRequest {
  readonly mode: SafePushMode
  readonly via: "click" | "key"
  readonly placement?: SafePushPlacement
  /** A profile chosen in the menu or the popover; undefined: the default (the last robot's); null: the default checks */
  readonly profileId?: string | null
  /** A Safe Push without review, asked from the Create Pull Request dialog */
  readonly bypassReview?: boolean
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

/** Forgets the profiles of the robots read (tests) */
export function resetStartOptions(): void {
  profileReads.clear()
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
  }
}

/** The start of the default intent (the default profile, with review): the view carries its decision (`RepositoryView.lands`) */
function isDefaultAsk(request: Omit<SafePushRequest, "placement">): boolean {
  return request.profileId === undefined && request.bypassReview !== true
}

/** The plan the session answers of the start `request` of `branchName` (`prepare`); null without an answer */
export async function prepareStart(branchName: string, request: Omit<SafePushRequest, "placement">): Promise<Plan | null> {
  const repositoryPath = getSelectedRepositoryPath()
  if (repositoryPath === null) {
    return null
  }
  const result = await commands.prepare({ repositoryPath, intent: { kind: "land", ...landOf(branchName, request) } })
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
  const review = reviewShownOf(mode, readiness, plan)
  // asked without review through the API (a comment takes no profile), while the approvals are missing or not known
  const withoutReview =
    request.bypassReview === true && mode === "rebaseAutosquash" && route?.kind !== "comment" && !(review.kind === "loaded" && review.readiness.kind !== "missing")
  return {
    branchName,
    subject,
    mode,
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

/** Resolves on the next change of a store the facts are read from */
function nextChange(): Promise<void> {
  return new Promise(resolve => {
    const stores = [branchSyncStore, reviewReadinessStore, pullRequestsStore, patronusStore]
    let unsubscribes: (() => void)[] = []
    const done = () => {
      unsubscribes.forEach(unsubscribe => unsubscribe())
      unsubscribes = []
      resolve()
    }
    unsubscribes = stores.map(store => store.subscribe(done))
  })
}

/**
 * The start of a click, planned again on every change until its plan waits for nothing (`prepare`): the session answers
 * every fact it waits for (the review it is asked at once, the rules, the profiles), a failure too, so the wait ends
 */
async function settleSafePush(branchName: string, ask: Omit<SafePushRequest, "placement">): Promise<SafePushRead | null> {
  // the profiles as the session has them now, the rules GitHub didn't give read again: the start decides on new answers
  await Promise.all([askStartOptions(branchName), askBranchRulesAgain()])
  for (;;) {
    const read = readSafePush(branchName, ask, await prepareStart(branchName, ask))
    if (read === null || read.prompt.kind !== "wait") {
      return read
    }
    // the rules come with the listing that has the approval: the session reads them then
    await nextChange()
  }
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
    action = { children: "Push", onClick: () => void pushAlone(branchName) }
  } else if (opens !== undefined) {
    action = refusalObjectAction(branchName, opens)
  }
  showStartToast(branchName, mode, `${formatPushMode(mode)} of ${formatReadinessSubject(branchName)} can't start`, reason, action, offersPush)
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
  const root = stackRootOfReadinessKey(branchName)
  if (root !== null && !isForkingRoot(branchSyncStore.getState(), root)) {
    // a straight chain has no stack branch: the Safe Push of its last branch lands the whole stack, at the header of its
    // island as the island's own Safe Push
    const leaf = getStackNextStep(root)?.leaf
    if (leaf != null) {
      await requestSafePush(leaf, { ...request, placement: request.placement ?? (getStackIslandRoots().has(root) ? "stack-header" : "row") })
    }
    return
  }
  // the row of the start: the branch, the root for the whole stack
  const row = root ?? branchName
  const branch = getBranch(row)
  const run = getLandRun(branchName)
  // a start of the row that runs shows in its slot: a second ask has nothing to add
  if (branch === undefined || !isPatronusAvailable() || (run !== null && run.by === null)) {
    return
  }
  const { mode } = request
  if (run?.by != null) {
    // another process runs a start of the row (an agent's `bd safe-push`): it holds what it lands until it ends
    tellRefusal(branchName, mode, run.by.description, false)
    return
  }
  // the whole stack of an island starts at its header, of a small stack at the root's row
  const placement = request.placement ?? (root !== null && getStackIslandRoots().has(root) ? "stack-header" : "row")
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
  if (placement === "row" && !getShownBranchTableOrder().includes(row)) {
    requestReveal({ branchName: row, commitIds: [] })
  }
  // a failed read of the rules is asked again by each start: the click waits for the new answer, the popover reads anew
  void askBranchRulesAgain()
  shownRead = null
  const ask: SafePushAsk = {
    branchName,
    mode,
    placement,
    ...(request.profileId !== undefined && { profileId: request.profileId }),
    ...(request.bypassReview === true && { bypassReview: true }),
  }
  if (request.via === "key" || request.bypassReview === true) {
    openSafePushPopover(ask)
    return
  }
  // the start is planned and run in the repository of the click: what it reads meanwhile decides on it, and it starts
  // there; its button says "Checking…" while it waits for its plan
  const repositoryPath = getSelectedRepositoryPath()
  setLandAsking(branchName, mode, repositoryPath)
  const release = mode === "rebaseAutosquash" ? demandReviewReadiness(branchName) : () => {}
  let read: SafePushRead | null
  try {
    read = await settleSafePush(branchName, request)
  } catch (error) {
    notifyError(`Cannot start the ${formatPushMode(mode)}`, error)
    return
  } finally {
    release()
    setLandAsking(branchName, null, repositoryPath)
  }
  if (read === null) {
    return
  }
  // the plan was read in the session of the selected repository: another one decides nothing here
  const switched = getRepositorySwitchReason(repositoryPath)
  if (switched !== null) {
    notifyError(`Cannot start the ${formatPushMode(mode)}`, switched)
    return
  }
  const decision = read.prompt
  switch (decision.kind) {
    case "start":
      if (read.plan !== null) {
        await startPlanned(read.plan, undefined, repositoryPath)
      }
      return
    case "refuse": {
      // the answer the click waited for may be the step's own reason (the approvals): its notice says it, with its action
      const settled = mode === "rebaseAutosquash" && !decision.offersPush ? currentStepOf(branchName) : undefined
      if (settled?.notice != null && settled.safePushBlocker === decision.reason) {
        tellNotice(branchName, settled, settled.notice, { ...request, placement })
      } else if (decision.opens === "review") {
        // the approvals it misses: the popover names them, where they count and how to get them
        openSafePushPopover(ask)
      } else {
        tellRefusal(branchName, mode, decision.reason, decision.offersPush, decision.opens)
      }
      return
    }
    case "wait":
    case "choose":
    case "confirm":
      openSafePushPopover(ask)
  }
}

// ---- the run ----

/** The subject of a start: the branch, or the key of the whole stack of a root (`stackReadinessKey`) */
function landKey(land: Land): string {
  return land.subject.kind === "branch" ? land.subject.name : stackReadinessKey(land.subject.root)
}

/** The plan of `land` in the session of `repositoryPath` (`prepare`); null without an answer, said under its title */
async function prepareIn(repositoryPath: string, land: Land): Promise<Plan | null> {
  const result = await commands.prepare({ repositoryPath, intent: { kind: "land", ...land } })
  if (result.status === "error" || result.data === null) {
    notifyError(
      `Cannot start the ${formatPushMode(land.mode)} of ${formatReadinessSubject(landKey(land))}`,
      result.status === "error" ? result.error : "The repository has no plan of it yet",
    )
    return null
  }
  return result.data
}

/**
 * Runs the start Rust planned (`execute` of the intent of `plan` on its fingerprint, every confirmation of it accepted:
 * the popover showed them, or the click had none) in `repositoryPath`, the repository it was planned in: the session
 * admits it and shows its steps in the slot of every window (`RepositoryView.operations`), the progress of its push and
 * whom it waits for come on its channel; then its toast and Activity from the outcome (`presentLandOutcome`), or the
 * refusal before its first effect (`presentLandRefusal`). `allow`: Start Anyway, the same start with the overrides of
 * what the landing gate refused, planned again first. Resolves to whether its robot started
 */
export async function startPlanned(plan: Plan, allow?: LandingAllowances, repositoryPath: string = getSelectedRepositoryPath()): Promise<boolean> {
  const { intent } = plan
  if (intent.kind !== "land") {
    return false
  }
  // "Checking…" until the session admits its operation, whose step the slot shows then
  const key = landKey(intent)
  setLandAsking(key, intent.mode === "dryRun" ? "dryRun" : "rebaseAutosquash", repositoryPath)
  return runPlanned(plan, intent, allow, repositoryPath).finally(() => setLandAsking(key, null, repositoryPath))
}

/** `startPlanned` of the start `intent` of `plan` */
async function runPlanned(plan: Plan, intent: Land, allow: LandingAllowances | undefined, repositoryPath: string): Promise<boolean> {
  const planned = allow === undefined ? plan : await prepareIn(repositoryPath, { ...intent, allow: { ...intent.allow, ...allow } })
  if (planned === null) {
    return false
  }
  const events = new Channel<OperationEvent>()
  let op: OperationId | null = null
  events.onmessage = event => {
    if (event.kind === "progress") {
      op = event.op
      setLandProgress(event.op, { percent: event.percent, line: event.line, transferred: null, speed: null })
    }
  }
  const params = { repositoryPath, intent: planned.intent, fingerprint: planned.fingerprint, accepted: planned.confirmations }
  const result = await executeIntent(params, events).finally(() => {
    if (op !== null) {
      setLandProgress(op, null)
    }
  })
  if (result.status === "error") {
    tellStartRefusal(result.error, planned, repositoryPath)
    return false
  }
  setLandProgress(result.data.op, null)
  return tellOutcome(result.data, planned, repositoryPath)
}

/**
 * The toast of how a start ended (`presentLandOutcome`), the next state of the event of the robots of its subject: the
 * robot it started (Open in Patronus, Cancel Safe Push), the comment Patronus accepted, or why the robot did not start,
 * "Start Safe Push" again when the facts changed after the push. Resolves to whether its robot started
 */
function tellOutcome(outcome: Outcome, plan: Plan, repositoryPath: string): boolean {
  const land = plan.intent.kind === "land" ? plan.intent : null
  if (land === null) {
    return false
  }
  const key = landKey(land)
  const toast = presentLandOutcome(outcome, plan.intent, formatReadinessSubject(key))
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
        stepOf: current => getRobotActionItems(current).find(action => action.id === "cancel-robot") ?? null,
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
      showEventToast({ key: subjectKey, render: () => ({ type: "info", title: toast.title, description: toast.description }) })
      return false
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
        notifyError(toast.title, error, { children: `Start ${formatPushMode(land.mode)}`, onClick: () => void startAgain(key, land, repositoryPath) }, opens)
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
    notifyError(`Cannot start the ${formatPushMode(land.mode)} of ${formatReadinessSubject(key)}`, getRepositorySwitchReason(repositoryPath))
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
  const refusal = presentLandRefusal(error, land.mode, formatReadinessSubject(key), land.allow ?? {})
  switch (refusal.kind) {
    case "landing": {
      const anyway = { label: "Start Anyway", onClick: () => void startPlanned(plan, refusal.anyway, repositoryPath) }
      const fix = refusal.fix
      const fixLabel = formatLandingFix(fix)
      const failure = { ...error, message: refusal.reason }
      if (fix?.kind === "restack" && fixLabel !== null) {
        notifyError(refusal.title, failure, { children: fixLabel, onClick: () => void restackBranch(fix.branch, fix.onto) }, anyway)
      } else {
        notifyError(refusal.title, failure, { children: anyway.label, onClick: anyway.onClick })
      }
      return
    }
    case "target":
      notifyError(refusal.title, refusal.error, { children: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) })
      return
    case "changed":
      notifyError(refusal.title, refusal.error, { children: `Start ${formatPushMode(land.mode)}`, onClick: () => void startAgain(key, land, repositoryPath) })
      return
    case "stopped":
      showEventToast({ key: toastKeys.robotSubject(repositoryPath, key, land.mode), render: () => ({ type: "info", title: refusal.title, description: refusal.description }) })
      return
    case "error":
      notifyError(refusal.title, refusal.error)
  }
}

/** The read the popover shows, and what it was read from: the same object while nothing it reads changed */
let shownRead: { inputs: readonly unknown[]; read: SafePushRead | null } | null = null

/**
 * `readSafePush` of the popover, once per change of what it reads (the stores, the plan of the session): the
 * subscriptions of the popover share the plan, and a selector returns the same object for the same state
 */
function readSafePushNow(branchName: string, ask: Omit<SafePushRequest, "placement">, plan: Plan | null): SafePushRead | null {
  const inputs = [
    branchSyncStore.getState(),
    reviewReadinessStore.getState(),
    pullRequestsStore.getState(),
    patronusStore.getState(),
    safePushStore.getState(),
    branchName,
    ask.mode,
    ask.via,
    ask.profileId,
    ask.bypassReview,
    plan,
  ]
  if (shownRead !== null && shownRead.inputs.every((input, index) => input === inputs[index])) {
    return shownRead.read
  }
  const read = readSafePush(branchName, ask, plan)
  shownRead = { inputs, read }
  return read
}

/**
 * The plan the session answers of the start of the popover (`prepare`), asked again with every patch of the view (its
 * facts are the session's) and every other profile; the last plan while the next is asked, null before the first
 */
function useStartPlan(branchName: string, request: Omit<SafePushRequest, "placement">): Plan | null {
  const [answer, setAnswer] = useState<{ readonly key: string; readonly plan: Plan | null } | null>(null)
  const version = useStore(branchSyncStore, state => (state.viewVersion === null ? null : `${state.viewVersion.epoch}:${state.viewVersion.seq}`))
  const key = JSON.stringify([branchName, request.mode, request.profileId ?? "default", request.bypassReview === true])
  const { mode, via, profileId, bypassReview } = request
  useEffect(() => {
    let current = true
    prepareStart(branchName, { mode, via, ...(profileId !== undefined && { profileId }), ...(bypassReview === true && { bypassReview }) }).then(
      plan => current && setAnswer({ key, plan }),
      (error: unknown) => current && notifyError(`Cannot read the ${formatPushMode(mode)} of ${formatReadinessSubject(branchName)}`, error),
    )
    return () => {
      current = false
    }
  }, [branchName, key, mode, via, profileId, bypassReview, version])
  // a plan of another profile is not this start's
  return answer?.key === key ? answer.plan : null
}

/**
 * `readSafePush` of the popover's ask, read again on every change of the stores it reads and of the plan of the
 * session; the profiles and the review demanded meanwhile
 */
export function useSafePushRead(ask: SafePushAsk, profileId: string | null | undefined): SafePushRead | null {
  useEffect(() => {
    void askStartOptions(ask.branchName)
  }, [ask.branchName])
  useDemandReviewReadiness(ask.branchName, ask.mode === "rebaseAutosquash")
  const request: Omit<SafePushRequest, "placement"> = { mode: ask.mode, via: "key", bypassReview: ask.bypassReview, ...(profileId !== undefined && { profileId }) }
  const plan = useStartPlan(ask.branchName, request)
  // the read flows out of the subscriptions: a value read beside them would be memoized by the compiler
  const select = () => readSafePushNow(ask.branchName, request, plan)
  useStore(branchSyncStore, select)
  useStore(reviewReadinessStore, select)
  useStore(pullRequestsStore, select)
  useStore(patronusStore, select)
  return useStore(safePushStore, select)
}
