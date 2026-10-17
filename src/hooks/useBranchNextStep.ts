import { useStore } from "zustand"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { notifyError } from "@/lib/errors"
import { getPullRequestBase } from "@/lib/git/new-pull-request"
import { isFullyLanded } from "@/lib/landed"
import { canStartRobot, getSafePushSlot, isPullRequestIntoTarget } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { getPushAction } from "@/lib/push-summary"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { stackAncestors } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { inlineRowActionStore, isProcessing, openInline } from "@/stores/inline-row-action"
import { landedStore } from "@/stores/landed"
import { getBranchRobot, getLandingAncestor, getLandingSpan, getPushHold, isPatronusAvailable, isRobotFailing, isRobotNeedingReview, patronusStore } from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, pullRequestsStore, retargetPullRequest } from "@/stores/pull-requests"
import { getSelectedProject } from "@/stores/repository"
import { getReadinessTarget, getReviewReadiness, reviewReadinessStore } from "@/stores/review-readiness"

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
  }
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
  const pushAction = getPushAction(branch)
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
    pushed: branch.remoteStatus?.exists === true,
    pushPending: pushAction !== null && pushAction !== "upToDate",
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
      // the review request of the pull request is asked there until its own form lands
      requestReview: () => {
        const current = getPullRequest(name) ?? pullRequest
        if (current !== undefined) {
          openPullRequest(current)
        }
      },
    },
  }
}

/** The next step of the branch now (`getBranchNextStep`), for menus built on open, keys and toasts; null without the branch */
export function getBranchNextStepNow(name: string): BranchNextStep | null {
  const branch = getBranch(name)
  return branch === undefined ? null : getBranchNextStepOf(branch)
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

const shownSteps = new Map<string, { key: string; step: BranchNextStep | null }>()

function selectNextStep(name: string): BranchNextStep | null {
  const step = getBranchNextStepNow(name)
  const key = stepKey(step, name)
  const shown = shownSteps.get(name)
  if (shown?.key === key) {
    return shown.step
  }
  shownSteps.set(name, { key, step })
  return step
}

/**
 * The next step of the branch, re-rendering when it changes: it depends on the branches, the robots, the pull
 * requests, the readiness, the landed commits and the inline action of the row, so each of their stores is watched
 */
export function useBranchNextStep(name: string): BranchNextStep | null {
  const select = () => selectNextStep(name)
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
  const reason = step?.safePushBlocker ?? null
  if (step === null || reason === null) {
    return
  }
  const pullRequest = getPullRequest(name)
  const opensPullRequest = pullRequest !== undefined && step.tone !== "default"
  showToast({
    type: "info",
    title: reason,
    description: `Safe Push of ${name}`,
    ...(opensPullRequest && { action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } }),
  })
}
