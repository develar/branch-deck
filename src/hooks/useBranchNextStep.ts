import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import type { ApprovalNews } from "@/lib/approval-news"
import { describeApprovalNews } from "@/lib/approval-news"
import type { PatronusRobot } from "@/lib/bindings"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { notifyError } from "@/lib/errors"
import { getPullRequestBase } from "@/lib/git/new-pull-request"
import { isFullyLanded } from "@/lib/landed"
import { announce } from "@/lib/os-notification"
import { canStartRobot, getSafePushSlot, isPullRequestIntoTarget } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { getRobotActions } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { stackAncestors } from "@/lib/stack"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { recordApproval } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { inlineRowActionStore, isProcessing, openInline } from "@/stores/inline-row-action"
import { landedStore } from "@/stores/landed"
import { getBranchRobot, getLandingAncestor, getLandingSpan, getPushHold, isPatronusAvailable, isRobotFailing, isRobotNeedingReview, patronusStore } from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, pullRequestsStore, retargetPullRequest } from "@/stores/pull-requests"
import { getFullBranchName, getSelectedProject } from "@/stores/repository"
import { openRequestReview } from "@/stores/request-review-popover"
import { getReadinessTarget, getReviewReadiness, reviewReadinessStore } from "@/stores/review-readiness"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/**
 * The baseline ("origin/master") while the fetched baseline has every commit of the branch, before the update archives
 * it (as `useLandedIn`); null otherwise
 */
function getLandedIn(name: string): string | null {
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
    baseIsFormerParent: isFormerParent(pullRequest.baseRefName ?? ""),
  }
}

/**
 * A base that is the virtual branch of a parent no longer in the table (it landed, was archived or deleted): the
 * pushed name `{prefix}/virtual/{name}` of no active branch
 */
function isFormerParent(base: string): boolean {
  const namespace = getFullBranchName("")
  return base.startsWith(namespace) && base.length > namespace.length && getBranch(base.slice(namespace.length)) === undefined
}

/** Retargets the open pull request of the branch to its base, the target, and says so */
async function retarget(branchName: string): Promise<void> {
  const branch = getBranch(branchName)
  const pullRequest = getPullRequest(branchName)
  if (branch === undefined || pullRequest === undefined) {
    return
  }
  const base = getPullRequestBase(branch)
  try {
    await retargetPullRequest(branchName, base)
  } catch (error) {
    notifyError(`Cannot retarget pull request #${pullRequest.number}`, error)
    return
  }
  showToast({
    title: `Retargeted #${pullRequest.number} to ${base}`,
    description: `${branchName} · its approvals count for a Safe Push now`,
    action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) },
  })
}

/** The Safe Push form of the root of a stack: the island shows its row first, also from a collapsed one */
function openSafePushOf(name: string): void {
  requestReveal({ branchName: name, commitIds: [] })
  openInline("safe-push", name)
}

/** What decides the next step of the branch now, from the stores */
function factsOf(branch: Branch): NextStepFacts {
  const name = branch.name
  const available = isPatronusAvailable()
  const robot = getBranchRobot(name)
  const needsReview = robot !== undefined && isRobotNeedingReview(robot.id)
  const failing = robot !== undefined && isRobotFailing(robot.id)
  const localTip = branch.commits[0]?.hash
  const slot = getSafePushSlot({ robot, localTip, remote: branch.remoteStatus, needsReview, failing, canOffer: available && !branch.hasError && branch.commitCount > 0 })
  const start = canStartRobot(branch, robot, getLandingAncestor(name), getLandingSpan(name))
  const dryRun = canStartRobot(branch, robot)
  const target = getReadinessTarget()
  const pullRequest = getPullRequest(name)
  const restart =
    robot === undefined ? null : (getRobotActions(robot, branch, needsReview, failing).find(it => it.id === "restart-robot" || it.id === "push-and-restart-robot") ?? null)
  const root = stackAncestors(name, getBranch)[0]
  const readiness = getReviewReadiness(name)
  return {
    branchName: name,
    patronusAvailable: available,
    syncing: branch.status === "Syncing",
    hasError: branch.hasError,
    commitCount: branch.commitCount,
    processing: isProcessing(name),
    blocked: branch.status === "Blocked",
    startBlocker: start.ok ? null : start.reason,
    dryRunStartBlocker: dryRun.ok ? null : dryRun.reason,
    held: getPushHold(name) !== null,
    landedIn: getLandedIn(name),
    robot: slot,
    robotPullRequest: robot?.pullRequest?.number ?? null,
    restart,
    pullRequestsAvailable: getPullRequestAvailability()?.status === "available",
    pullRequest: pullRequestFactOf(branch, target),
    target,
    // the root has no root: one level
    stackRoot: root === undefined ? null : { name: root.name, blocker: getBranchNextStepNow(root.name)?.safePushBlocker ?? null },
    readiness: readiness === undefined ? null : { readiness: readiness.readiness, stale: readiness.stale },
    actions: {
      openSafePush: () => openInline("safe-push", name),
      openRootSafePush: () => {
        if (root !== undefined) {
          openSafePushOf(root.name)
        }
      },
      openPullRequest: () => {
        const current = getPullRequest(name) ?? pullRequest
        if (current !== undefined) {
          openPullRequest(current)
        }
      },
      retarget: () => void retarget(name),
      createPullRequest: getCreatePullRequestItem(branch),
      requestReview: getRequestReviewItem(branch, pullRequest, readiness?.readiness ?? null, () => openRequestReview(name)),
    },
  }
}

/**
 * The next step of the branch now (`getBranchNextStep`), for menus built on open, keys, toasts and the rows; null without
 * the branch. Computed once per change of the stores it reads (`stepSources`): six subscriptions of a row, the readiness
 * scan and the focus ask it, they share the answer.
 */
export function getBranchNextStepNow(name: string): BranchNextStep | null {
  const sources = stepSources()
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

/** The states of every store the next step reads: a change of any is a new step */
function stepSources(): readonly unknown[] {
  return [
    branchSyncStore.getState(),
    patronusStore.getState(),
    pullRequestsStore.getState(),
    reviewReadinessStore.getState(),
    landedStore.getState(),
    inlineRowActionStore.getState(),
    repositorySettingsStore.getState(),
  ]
}

/** The next step of `branch` as it is given (a menu of the branch it shows), the rest from the stores */
export function getBranchNextStepOf(branch: Branch): BranchNextStep {
  return getBranchNextStep(factsOf(branch))
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
 * requests, the readiness, the landed commits and the inline action of the row, so each of their stores is watched
 */
export function useBranchNextStep(name: string): BranchNextStep | null {
  const select = () => getBranchNextStepNow(name)
  useStore(branchSyncStore, select)
  useStore(patronusStore, select)
  useStore(pullRequestsStore, select)
  useStore(reviewReadinessStore, select)
  useStore(landedStore, select)
  return useStore(inlineRowActionStore, select)
}

/** The branches of `order` whose next step depends on their readiness: the store asks Patronus for them */
export function branchesNeedingReadiness(order: readonly string[]): string[] {
  return order.filter(name => getBranchNextStepNow(name)?.needsReadiness === true)
}

/** Whether a row waits for an approval (Awaiting): a focus asks GitHub for the pull requests at once then */
export function isAwaitingApproval(name: string): boolean {
  return getBranchNextStepNow(name)?.kind === "awaiting"
}

/**
 * S on a branch whose "Safe Push…" is disabled: the reason as an `info` toast (a precondition), with "Open Pull Request"
 * while the step waits in the pull request (the approvals, a draft)
 */
export function explainSafePushBlocker(name: string): void {
  const step = getBranchNextStepNow(name)
  const notice = step?.notice ?? null
  if (step === null || notice === null) {
    return
  }
  const pullRequest = getPullRequest(name)
  const opensPullRequest = pullRequest !== undefined && step.tone !== "default"
  showToast({
    type: "info",
    title: notice.title,
    description: notice.description,
    ...(opensPullRequest && { action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } }),
  })
}

/**
 * Tells that the approvals of a branch arrived (`findApprovalNews`, the readiness store of the main window): a toast
 * "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform" with "Safe Push…" first (the row's form, the row
 * revealed; decided again at the click: a reason that came since is an `info` toast) and "Open Pull Request"; in the
 * background the OS notification of "Approvals of my pull requests" and the toast on the next focus; an Activity entry
 */
export function tellApprovalNews(news: ApprovalNews): void {
  const repositoryPath = getSelectedProject()?.path
  const pullRequest = getPullRequest(news.branchName)
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
        children: "Safe Push…",
        onClick: () => {
          toast.close(id)
          const branch = getBranch(news.branchName)
          if (branch !== undefined && getBranchNextStepOf(branch).safePushBlocker === null) {
            openSafePushOf(news.branchName)
          } else {
            explainSafePushBlocker(news.branchName)
          }
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
 * robots indicator): its restart is disabled with the reason of the next step while the approvals that count are still
 * missing (a restart of a robot that failed for them fails again)
 */
export function getGatedRobotActions(robot: PatronusRobot, branch: Branch | null, needsReview?: boolean, failing?: boolean): RobotAction[] {
  const actions = getRobotActions(robot, branch, needsReview, failing)
  if (branch === null) {
    return actions
  }
  const step = getBranchNextStepOf(branch)
  const reason = APPROVAL_STEPS.has(step.kind) ? step.safePushBlocker : null
  return reason === null
    ? actions
    : actions.map(item => (item.id === "restart-robot" || item.id === "push-and-restart-robot" ? { ...item, disabled: true, description: reason } : item))
}
