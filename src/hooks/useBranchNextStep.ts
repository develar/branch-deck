import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import { describeApprovalNews } from "@/lib/approval-news"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import type { BranchNextStep, NextStepBusy, NextStepFacts, NextStepPullRequest, StackPullRequestFact } from "@/lib/branch-next-step"
import { formatLandedState, getBranchNextStep } from "@/lib/branch-next-step"
import { notifyError } from "@/lib/errors"
import { isVirtualBranchBase } from "@/lib/git/new-pull-request"
import { isFullyLanded } from "@/lib/landed"
import { announce } from "@/lib/os-notification"
import { canStartRobot, formatLandedReason, formatPushMode, getSafePushSlot, isLandedByOwnRobot, isLandedWhole, isPullRequestIntoTarget } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { getRobotActions } from "@/lib/patronus-actions"
import { changesRequestedBy, openPullRequest, waitingReviewers } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { formatWholeStack, stackReadinessKey } from "@/lib/whole-stack"
import { recordApproval, recordRetarget } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getParkedState, getStackBranchOf, useBranchSync } from "@/stores/branch-sync"
import { landedStore } from "@/stores/landed"
import type { Operations } from "@/stores/operations"
import { getObjectBusy, operationsStore, runOperation } from "@/stores/operations"
import {
  getBranchRobot,
  getLandedBy,
  getLandingAncestor,
  getLandingBranchCount,
  getLandingSpan,
  getPushHold,
  isPatronusAvailable,
  isRobotFailing,
  isRobotNeedingReview,
  patronusStore,
} from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, getStackPullRequest, pullRequestsStore, retargetPullRequest } from "@/stores/pull-requests"
import { getSelectedProject } from "@/stores/repository"
import { openRequestReview } from "@/stores/request-review-popover"
import { freshReadiness, getReadinessTarget, getReviewReadiness, reviewReadinessStore, useShowReviewReadiness } from "@/stores/review-readiness"
import { getSafePushStarting, safePushStore } from "@/stores/safe-push"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/**
 * The baseline ("origin/master") while the fetched baseline has every commit of the branch, before the update archives
 * it (as `useLandedIn`); null otherwise
 */
export function getLandedIn(name: string): string | null {
  const landed = landedStore.getState()
  if (landed.repositoryPath !== getSelectedProject()?.path) {
    return null
  }
  const incoming = landed.landing.find(branch => branch.name === name)
  if (incoming === undefined || !isFullyLanded(incoming, true)) {
    return null
  }
  const status = branchSyncStore.getState().baselineStatus
  return status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
}

/** The pull request of the branch as the next step needs it */
function pullRequestFactOf(branch: Branch, target: string): NextStepPullRequest | null {
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest === undefined) {
    return null
  }
  // a merged pull request of other commits than the branch has now: new work under the same name
  const mergedCurrent = pullRequest.state === "merged" && !isReviewCommitOutdated(getReviewCommitState(pullRequest.headRefOid, branch.commits[0]?.hash, branch.remoteStatus))
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base: pullRequest.baseRefName ?? "",
    intoTarget: isPullRequestIntoTarget(pullRequest, target),
    mergedCurrent,
    baseIsVirtualBranch: isVirtualBranchBase(pullRequest.baseRefName ?? ""),
    waitingFor: waitingReviewers(pullRequest),
    changesRequestedBy: changesRequestedBy(pullRequest),
    checksFailing: pullRequest.checks?.state === "failing",
    changesRequested: pullRequest.reviewDecision === "changesRequested",
  }
}

/**
 * Retargets the open pull request of the branch to the target of Patronus, as a pull request names it ("master"), as
 * every pull request of a virtual branch goes, and says so (Activity too): the step of row 5, the menu, and the Safe
 * Push form of a branch whose pull request merges into a virtual branch
 */
export async function retargetToTarget(branchName: string): Promise<void> {
  const pullRequest = getPullRequest(branchName)
  if (getBranch(branchName) === undefined || pullRequest === undefined) {
    return
  }
  const base = getReadinessTarget()
  const failure = `Cannot retarget pull request #${pullRequest.number}`
  // an operation of the pull request: a second click joins it, the slot and the menu say it runs
  await runOperation(
    { id: "retarget-pull-request", subject: `pr:${pullRequest.number}`, objects: [branchName], kind: "remote", label: `Retargeting #${pullRequest.number} to ${base}` },
    () =>
      retargetPullRequest(branchName, base).then(
        () => {
          const title = `Retargeted #${pullRequest.number} to ${base}`
          const description = `${branchName} · its approvals count for a Safe Push now`
          recordRetarget(title, description, pullRequest.url)
          showToast({ title, description, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
        },
        (error: unknown) => notifyError(failure, error),
      ),
    reason => showToast({ type: "info", title: failure, description: reason }),
  )
}

/**
 * How a start reads the next step (`readSafePushFacts`): the review only as Patronus answered it for the branch as it is
 * (a stale answer is not asked yet: the start waits for the new one), and its own running start is no reason against it
 */
export interface StartStepOptions {
  readonly forStart: true
}

/** The open pull request of the whole stack that holds the branch (`getStackBranchOf`); null without one */
function stackPullRequestFactOf(name: string): StackPullRequestFact | null {
  const stack = getStackBranchOf(name)
  const pullRequest = stack === undefined ? undefined : getStackPullRequest(stack.root)
  if (stack === undefined || pullRequest?.state !== "open") {
    return null
  }
  return { number: pullRequest.number, subject: formatWholeStack(stack.root), open: () => openPullRequest(getStackPullRequest(stack.root) ?? pullRequest) }
}

/**
 * The operation on the row for the next step (`NextStepFacts.busy`): an operation of the registry, or the running start
 * of a Safe Push of the row, which is no reason against the start it runs itself (`options`)
 */
export function busyFact(key: string, options: StartStepOptions | null): NextStepBusy | null {
  const operation = getObjectBusy(key)
  if (operation !== undefined) {
    return { id: operation.id, label: operation.label }
  }
  const starting = options === null ? getSafePushStarting(key) : null
  return starting == null ? null : { id: "", label: `Starting the ${formatPushMode(starting.mode)}` }
}

/** What decides the next step of the branch now, from the stores; the node of its row reads them too (`useLandingNode`) */
export function getNextStepFacts(branch: Branch, options: StartStepOptions | null = null): NextStepFacts {
  const name = branch.name
  const available = isPatronusAvailable()
  const robot = getBranchRobot(name)
  const needsReview = robot !== undefined && isRobotNeedingReview(robot.id)
  const failing = robot !== undefined && isRobotFailing(robot.id)
  const localTip = branch.commits[0]?.hash
  const landedBy = getLandedBy(name)
  const slot = getSafePushSlot({
    robot,
    localTip,
    remote: branch.remoteStatus,
    needsReview,
    failing,
    canOffer: available && !branch.hasError && branch.commitCount > 0,
    landed: isLandedByOwnRobot(landedBy, robot),
  })
  const start = canStartRobot(branch, robot, getLandingAncestor(name), getLandingSpan(name))
  const dryRun = canStartRobot(branch, robot)
  const target = getReadinessTarget()
  const pullRequest = getPullRequest(name)
  const restart =
    robot === undefined ? null : (getRobotActions(robot, branch, needsReview, failing).find(it => it.id === "restart-robot" || it.id === "push-and-restart-robot") ?? null)
  const readiness = options === null ? getReviewReadiness(name) : (freshReadiness(getReviewReadiness(name), name) ?? undefined)
  const landedIn = getLandedIn(name)
  // all of it landed; one with commits after the landed ones has them to land
  const landedWith = isLandedWhole(landedBy) ? formatLandedReason(landedBy) : null
  const pullRequestFact = pullRequestFactOf(branch, target)
  return {
    branchName: name,
    subject: name,
    patronusAvailable: available,
    syncing: branch.status === "Syncing",
    hasError: branch.hasError,
    commitCount: branch.commitCount,
    busy: busyFact(name, options),
    blocked: branch.status === "Blocked",
    startBlocker: start.ok ? null : start.reason,
    dryRunStartBlocker: dryRun.ok ? null : dryRun.reason,
    held: getPushHold(name) !== null,
    parked: getParkedState(name),
    landedIn,
    landedWith,
    robot: slot,
    robotPullRequest: robot?.pullRequest?.number ?? null,
    restart,
    pullRequestsAvailable: getPullRequestAvailability()?.status === "available",
    pullRequest: pullRequestFact,
    target,
    // the ancestors a merged Safe Push landed are in the target: the robot lands the others
    landsBranches: getLandingBranchCount(name),
    startDescription: null,
    readiness: readiness === undefined ? null : { readiness: readiness.readiness, stale: readiness.stale },
    stackPullRequest: stackPullRequestFactOf(name),
    actions: {
      openSafePush: () => void requestSafePush(name, { mode: "rebaseAutosquash", via: "click" }),
      openPullRequest: () => {
        const current = getPullRequest(name) ?? pullRequest
        if (current !== undefined) {
          openPullRequest(current)
        }
      },
      retarget: () => void retargetToTarget(name),
      // nothing of a branch that landed is left to review: the step decides it, so the item reads its facts here
      createPullRequest: getCreatePullRequestItem(branch, formatLandedState({ landedIn, landedWith, robot: slot, pullRequest: pullRequestFact })),
      requestReview: getRequestReviewItem(pullRequest, readiness?.readiness ?? null, () => openRequestReview(name)),
    },
  }
}

/**
 * The next step of the branch now (`getBranchNextStep`), for menus built on open, keys, toasts and the rows; null without
 * the branch. Computed once per change of the stores it reads (`nextStepSources`): six subscriptions of a row, the readiness
 * scan and the focus ask it, they share the answer.
 */
export function getBranchNextStepNow(name: string): BranchNextStep | null {
  const sources = nextStepSources()
  const shown = shownSteps.get(name)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.step
  }
  const branch = getBranch(name)
  const step = branch === undefined ? null : getBranchNextStepOf(branch)
  const key = stepKey(step, name)
  // the same object while the step stays: a row renders again only when it changes
  const kept = shown?.key === key ? shown.step : step
  shownSteps.set(name, { sources, key, step: kept })
  return kept
}

/** The states of every store the next step reads: a change of any is a new step (of a branch, of a whole stack) */
export function nextStepSources(): readonly unknown[] {
  return [
    branchSyncStore.getState(),
    patronusStore.getState(),
    pullRequestsStore.getState(),
    reviewReadinessStore.getState(),
    landedStore.getState(),
    operationsStore.getState(),
    safePushStore.getState(),
    repositorySettingsStore.getState(),
  ]
}

/** The next step of `branch` as it is given (a menu of the branch it shows), the rest from the stores; `options` for a start */
export function getBranchNextStepOf(branch: Branch, options: StartStepOptions | null = null): BranchNextStep {
  return getBranchNextStep(getNextStepFacts(branch, options))
}

/** What a surface renders of a step: one object while it stays, so a row renders again only when its step changes */
function stepKey(step: BranchNextStep | null, name: string): string {
  if (step === null) {
    return ""
  }
  const item = step.item
  const pullRequest = getPullRequest(name)
  return JSON.stringify([
    step.kind,
    step.tone,
    step.safePushBlocker,
    step.dryRunBlocker,
    step.needsReadiness,
    step.landed,
    item?.id,
    item?.label,
    item?.disabled,
    item?.description,
    getBranchRobot(name)?.id,
    pullRequest?.number,
    pullRequest?.url,
  ])
}

/** By branch: the step shown, its key, and the store states it was computed for */
const shownSteps = new Map<string, { sources: readonly unknown[]; key: string; step: BranchNextStep | null }>()

/**
 * The next step of the branch, re-rendering when it changes: it depends on the branches, the robots, the pull
 * requests, the readiness, the landed commits and the operations on the row, so each of their stores is watched.
 * The component shows the step: the readiness store asks for the readiness of the branch while it does
 * (`useShowReviewReadiness`), never for a row a collapse hides; of the root of a stack that forks, the readiness of its
 * whole stack too (`stackReadinessKey`), whose Safe Push its menu and its popover offer, with or without an island.
 */
export function useBranchNextStep(name: string): BranchNextStep | null {
  useShowReviewReadiness(name)
  const headsWholeStack = useBranchSync(state => (state.stackBranches[name]?.members.length ?? 0) > 0)
  useShowReviewReadiness(headsWholeStack ? stackReadinessKey(name) : "")
  return useNextStepStores(() => getBranchNextStepNow(name))
}

/** Renders again with `select` when a store the next step reads changes (of a branch, of a whole stack, of a node) */
export function useNextStepStores<T>(select: () => T): T {
  useStore(branchSyncStore, select)
  useStore(patronusStore, select)
  useStore(pullRequestsStore, select)
  useStore(reviewReadinessStore, select)
  useStore(landedStore, select)
  useStore(safePushStore, select)
  return useStore(operationsStore, select)
}

/** Whether a row waits for an approval (Awaiting): a focus asks GitHub for the pull requests at once then */
export function isAwaitingApproval(name: string): boolean {
  return getBranchNextStepNow(name)?.kind === "awaiting"
}

/**
 * Tells that the approvals of a branch arrived (`findApprovalNews`, the readiness store of the main window): a toast
 * "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform" with "Safe Push" first (the start of the row, the row
 * revealed; decided again at the click: a reason that came since is an `info` toast) and "Open Pull Request"; in the
 * background the OS notification of "Approvals of my pull requests" and the toast on the next focus; an Activity entry
 */
export function tellApprovalNews(news: ApprovalNews): void {
  announceApproval(news, getPullRequest(news.branchName), {
    label: "Safe Push",
    onClick: () => void requestSafePush(news.branchName, { mode: "rebaseAutosquash", via: "click" }),
  })
}

/**
 * The toast, the OS notification and the Activity entry of approvals that arrived in `pullRequest` (`describeApprovalNews`),
 * the start of what they count for first, then "Open Pull Request": of a branch, of the whole stack (`tellStackApprovalNews`)
 */
export function announceApproval(news: ApprovalNews, pullRequest: PullRequest | undefined, start: { readonly label: string; readonly onClick: () => void }): void {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined || pullRequest === undefined) {
    return
  }
  const { title, description } = describeApprovalNews(news)
  recordApproval(repositoryPath, title, description, pullRequest.url)
  announce("approvals", { title, body: description }, () => {
    const id = toast.add({
      type: "success",
      title,
      description,
      timeout: ACTION_TOAST_TIMEOUT_MS,
      actionProps: {
        children: start.label,
        onClick: () => {
          toast.close(id)
          start.onClick()
        },
      },
      data: { secondaryAction: { label: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } },
    })
  })
}

/** The next steps that wait for approvals: a restart of a robot that failed for them would fail again */
const APPROVAL_STEPS: ReadonlySet<BranchNextStep["kind"]> = new Set(["createPullRequest", "changesRequested", "requestReview", "awaiting"])

/**
 * The actions of a robot (`getRobotActions`) for every surface that shows them (the branch menu, the robot details, the
 * landing overview): its restart is disabled with the reason of the next step while the approvals that count are still
 * missing (a restart of a robot that failed for them fails again)
 */
export function getGatedRobotActions(robot: PatronusRobot, branch: Branch | null, needsReview?: boolean, failing?: boolean, operations?: Operations): RobotAction[] {
  const actions = getRobotActions(robot, branch, needsReview, failing, operations)
  if (branch === null) {
    return actions
  }
  const step = getBranchNextStepOf(branch)
  const reason = APPROVAL_STEPS.has(step.kind) ? step.safePushBlocker : null
  return reason === null
    ? actions
    : actions.map(item => (item.id === "restart-robot" || item.id === "push-and-restart-robot" ? { ...item, disabled: true, description: reason } : item))
}
