import { GitPullRequestCreateIcon } from "lucide-react"
import type { StartStepOptions } from "@/hooks/useBranchNextStep"
import { announceApproval, getBranchNextStepNow, getLandedIn, nextStepSources, tellApprovalNews, useBranchNextStep, useNextStepStores } from "@/hooks/useBranchNextStep"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import { formatBranchCount } from "@/lib/branch"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { getBranchesGitHubRepository } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import { canStartStackRobot, formatLandingSubject, getSafePushSlot, isPullRequestIntoTarget } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { changesRequestedBy, openPullRequest, waitingReviewers } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import type { StackLandedPart } from "@/lib/whole-stack"
import { describeWholeStackLanding, formatWholeStack, getStackBranchBlocker, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import type { StackBranch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getStackBranch, stackMainLineIn, useBranchSync } from "@/stores/branch-sync"
import { openStackPullRequestDialog } from "@/stores/create-pull-request-dialog"
import { isObjectBusy } from "@/stores/operations"
import {
  getLandedBy,
  getStackLandedReason,
  getStackLandingMember,
  getStackPushHold,
  getStackRobot,
  isPatronusAvailable,
  isRobotFailing,
  isRobotNeedingReview,
} from "@/stores/patronus"
import { getPullRequestAvailability, getStackPullRequest } from "@/stores/pull-requests"
import { openRequestReview } from "@/stores/request-review-popover"
import { freshReadiness, getReadinessTarget, getReviewReadiness, useShowReviewReadiness } from "@/stores/review-readiness"
import { getSafePushStarting } from "@/stores/safe-push"

/**
 * The next step of the whole stack of a root ([Next Step](../../spec/branch/next-step.spec.md#the-whole-stack)): of its
 * stack branch `{prefix}/stack/<root>` while the stack forks, whose one robot lands every branch of it, side branches
 * too; of its last branch for a straight chain, which has no stack branch: that branch's Safe Push lands the whole stack
 */
export interface StackNextStep {
  /** The last branch of a straight chain, whose step it is; null: the step of the stack branch */
  readonly leaf: string | null
  readonly step: BranchNextStep
}

/**
 * The branches of the whole stack as a sentence names them: its main line first (`stackMainLineIn`), then the side
 * branches, which only the whole stack lands, each in the order of the stack
 */
export function getWholeStackMembers(stack: StackBranch): string[] {
  const mainLine = stackMainLineIn(branchSyncStore.getState(), stack.root).filter(name => stack.members.includes(name))
  return [...mainLine, ...stack.members.filter(name => !mainLine.includes(name))]
}

/**
 * What the Safe Push of the whole stack lands now (`getWholeStackRest`): its branches less those merged Safe Pushes
 * landed whole (`getLandedBy`), each with its commits, one landed with commits after it by those commits only
 */
export interface WholeStackRest {
  /** The branches left, in the order of a sentence (`getWholeStackMembers`) */
  readonly members: readonly string[]
  /** Their commits, of a branch landed with commits after it those only */
  readonly commitCount: number
  /** The branches landed whole and who landed them; null while none is */
  readonly landed: StackLandedPart | null
  /** The robot of the whole stack itself landed every branch of it */
  readonly byOwnRobot: boolean
}

/**
 * What is left of the whole stack after merged Safe Pushes of pieces of it or of the whole stack landed branches of it:
 * its stack branch still carries their commits (Patronus finds them in the target and lands the rest), so the start
 * names, counts and says only the rest; nothing left is the step of a landed branch (row 1)
 */
export function getWholeStackRest(stack: StackBranch): WholeStackRest {
  const members: string[] = []
  const landedNames: string[] = []
  const by: string[] = []
  let commitCount = 0
  let byOwnRobot = true
  for (const name of getWholeStackMembers(stack)) {
    const landed = getLandedBy(name)
    // a branch landed with commits after it lands those
    if (landed !== null && landed.after.length === 0) {
      landedNames.push(name)
      const subject = formatLandingSubject(landed)
      if (!by.includes(subject)) {
        by.push(subject)
      }
      byOwnRobot &&= landed.wholeStack === true && landed.branch === stack.root
      continue
    }
    members.push(name)
    commitCount += landed === null ? (getBranch(name)?.commitCount ?? 0) : landed.after.length
    byOwnRobot = false
  }
  // nothing landed: the commit count of the stack branch, which the sync counted
  return landedNames.length === 0
    ? { members, commitCount: stack.commitCount, landed: null, byOwnRobot: false }
    : { members, commitCount, landed: { names: landedNames, by }, byOwnRobot }
}

/** The stack forks: its whole stack is its stack branch, which the sync builds (`StackBranch.members`) */
export function isForkingStack(stack: StackBranch | undefined): stack is StackBranch {
  return stack !== undefined && stack.members.length > 0
}

/**
 * "Create Pull Request of the Stack…": the pull request of the stack branch of a stack that forks into the baseline,
 * disabled with the reason while the sync didn't build it, and once merged Safe Pushes landed every branch of it
 * (`getStackLandedReason`: nothing of it is left to review or land); null for a stack that doesn't fork or without the
 * GitHub repository of the branches
 */
export function getCreateStackPullRequestItem(root: string): MenuItem | null {
  const stack = getStackBranch(root)
  if (stack === undefined || stack.members.length === 0 || getBranchesGitHubRepository() === null) {
    return null
  }
  const blocker = getStackBranchBlocker(root, stack) ?? getStackLandedReason(root)
  return {
    id: "create-stack-pull-request",
    label: "Create Pull Request of the Stack…",
    icon: GitPullRequestCreateIcon,
    disabled: blocker !== null,
    description:
      blocker ?? `The pull request of ${formatWholeStack(root)} into ${getReadinessTarget()}: the commits of ${formatBranchCount(stack.members.length)}, which one Safe Push lands`,
    onSelect: () => openStackPullRequestDialog(root),
  }
}

/** The pull request of the whole stack as the next step needs it: never retargeted, so never "a virtual branch" */
function stackPullRequestFactOf(stack: StackBranch, target: string): NextStepPullRequest | null {
  const pullRequest = getStackPullRequest(stack.root)
  if (pullRequest === undefined) {
    return null
  }
  const mergedCurrent = pullRequest.state === "merged" && !isReviewCommitOutdated(getReviewCommitState(pullRequest.headRefOid, stack.tip ?? undefined, stack.remoteStatus))
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base: pullRequest.baseRefName ?? "",
    intoTarget: isPullRequestIntoTarget(pullRequest, target),
    mergedCurrent,
    baseIsVirtualBranch: false,
    waitingFor: waitingReviewers(pullRequest),
    changesRequestedBy: changesRequestedBy(pullRequest),
    checksFailing: pullRequest.checks?.state === "failing",
    changesRequested: pullRequest.reviewDecision === "changesRequested",
  }
}

/**
 * What decides the next step of the whole stack of a stack that forks, from the stores: its stack branch (its commits,
 * its state: only a built one lands, else the reason, `getStackBranchBlocker`), the robot of the whole stack, the robot
 * of a branch of it that lands a part of the stack (its start waits for it), its pull request, and the readiness of the
 * whole stack (`stackReadinessKey`). A branch of it that syncs is the sync of the whole stack.
 */
export function stackFactsOf(stack: StackBranch, options: StartStepOptions | null = null): NextStepFacts {
  const { root } = stack
  const key = stackReadinessKey(root)
  const available = isPatronusAvailable()
  const robot = getStackRobot(root)
  const needsReview = robot !== undefined && isRobotNeedingReview(robot.id)
  const failing = robot !== undefined && isRobotFailing(robot.id)
  const tip = stack.tip ?? undefined
  const rest = getWholeStackRest(stack)
  const slot = getSafePushSlot({
    robot,
    localTip: tip,
    remote: stack.remoteStatus,
    needsReview,
    failing,
    canOffer: available && stack.commitCount > 0,
    // a rebuild of the stack branch after its robot landed all of it: new hashes, not new work
    landed: rest.byOwnRobot && rest.members.length === 0,
  })
  const blocker = getStackBranchBlocker(root, stack)
  const landingMember = getStackLandingMember(root)
  const start = canStartStackRobot(blocker, robot, tip, stack.remoteStatus, landingMember)
  const dryRun = canStartStackRobot(blocker, robot, tip, stack.remoteStatus, landingMember, true)
  const target = getReadinessTarget()
  const pullRequest = getStackPullRequest(root)
  const restart =
    robot === undefined ? null : (getRobotActions(robot, null, needsReview, failing).find(it => it.id === "restart-robot" || it.id === "push-and-restart-robot") ?? null)
  const readiness = options === null ? getReviewReadiness(key) : (freshReadiness(getReviewReadiness(key), key) ?? undefined)
  const landedIn = stack.members.every(name => getLandedIn(name) !== null) ? getLandedIn(root) : null
  return {
    branchName: key,
    subject: formatWholeStack(root),
    patronusAvailable: available,
    syncing: stack.members.some(name => getBranch(name)?.status === "Syncing"),
    hasError: false,
    commitCount: stack.commitCount,
    // its own running start is no reason against the start it runs
    processing: isObjectBusy(key) || (options === null && getSafePushStarting(key) !== null),
    blocked: false,
    startBlocker: start.ok ? null : start.reason,
    dryRunStartBlocker: dryRun.ok ? null : dryRun.reason,
    held: getStackPushHold(root) !== null,
    landedIn,
    // merged Safe Pushes, of pieces of it or its own, landed every branch of it: nothing is left to land
    landedWith: getStackLandedReason(root),
    robot: slot,
    robotPullRequest: robot?.pullRequest?.number ?? null,
    restart,
    pullRequestsAvailable: getPullRequestAvailability()?.status === "available",
    pullRequest: stackPullRequestFactOf(stack, target),
    target,
    // what merged Safe Pushes landed of it is in the target: the robot lands the rest
    landsBranches: rest.members.length,
    startDescription: describeWholeStackLanding(rest.members, rest.commitCount, target, "rebaseAutosquash", rest.landed),
    readiness: readiness === undefined ? null : { readiness: readiness.readiness, stale: readiness.stale },
    stackPullRequest: null,
    actions: {
      openSafePush: () => void requestSafePush(key, { mode: "rebaseAutosquash", via: "click" }),
      openPullRequest: () => {
        const current = getStackPullRequest(root) ?? pullRequest
        if (current !== undefined) {
          openPullRequest(current)
        }
      },
      // the pull request of the whole stack goes into the target by construction: nothing to retarget
      retarget: () => {},
      createPullRequest: getCreateStackPullRequestItem(root),
      requestReview: getRequestReviewItem(pullRequest, readiness?.readiness ?? null, () => openRequestReview(key)),
    },
  }
}

/** By root: the step shown, its key, and the store states it was computed for */
const shownSteps = new Map<string, { sources: readonly unknown[]; key: string; step: StackNextStep | null }>()

/** What a surface renders of the step of the whole stack: one object while it stays */
function stackStepKey(next: StackNextStep | null, root: string): string {
  if (next === null) {
    return ""
  }
  const { step, leaf } = next
  if (leaf !== null) {
    // the step of the branch is one object while it stays
    return JSON.stringify([leaf, getBranchNextStepNow(leaf) === step])
  }
  const item = step.item
  const pullRequest = getStackPullRequest(root)
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
    getStackRobot(root)?.id,
    pullRequest?.number,
    pullRequest?.url,
    pullRequest?.state,
  ])
}

/**
 * The next step of the whole stack of `root` now (`StackNextStep`), for menus built on open, keys and toasts; null for a
 * branch that heads no stack. Computed once per change of the stores it reads, the same object while it stays.
 */
export function getStackNextStepNow(root: string): StackNextStep | null {
  const sources = nextStepSources()
  const shown = shownSteps.get(root)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.step
  }
  const next = computeStackNextStep(root)
  const key = stackStepKey(next, root)
  const kept = shown?.key === key ? shown.step : next
  shownSteps.set(root, { sources, key, step: kept })
  return kept
}

function computeStackNextStep(root: string): StackNextStep | null {
  const stack = getStackBranch(root)
  if (isForkingStack(stack)) {
    return { leaf: null, step: getBranchNextStep(stackFactsOf(stack)) }
  }
  // a straight chain: its last branch lands the whole stack
  const mainLine = stackMainLineIn(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1)
  if (leaf === undefined || mainLine.length < 2) {
    return null
  }
  const step = getBranchNextStepNow(leaf)
  return step === null ? null : { leaf, step }
}

/**
 * The next step of the whole stack of `root`, re-rendering when it changes (the header of its island, the root's menu
 * and preview); the readiness of the whole stack is asked while it shows
 */
export function useStackNextStep(root: string): StackNextStep | null {
  const forks = useBranchSync(state => isForkingStack(state.stackBranches[root]))
  useShowReviewReadiness(forks ? stackReadinessKey(root) : "")
  return useNextStepStores(() => getStackNextStepNow(root))
}

/**
 * The next step of a start, re-rendering when it changes: of the branch, or of the whole stack of `root` while it forks
 * (the step of the last branch of a straight chain is that branch's own: its button is the branch's)
 */
export function useStartStep(branchName: string, root: string | undefined): BranchNextStep | null {
  const branchStep = useBranchNextStep(root === undefined ? branchName : "")
  const stackStep = useStackNextStep(root ?? "")
  if (root === undefined) {
    return branchStep
  }
  return stackStep?.leaf === null ? stackStep.step : null
}

/**
 * Of the shown readiness keys (`askedOrder`: branches and whole stacks, `stackReadinessKey`), those whose next step
 * depends on the readiness: the store asks Patronus for them
 */
export function readinessKeysNeedingAnswer(order: readonly string[]): string[] {
  return order.filter(key => {
    const root = stackRootOfReadinessKey(key)
    if (root === null) {
      return getBranchNextStepNow(key)?.needsReadiness === true
    }
    const next = getStackNextStepNow(root)
    return next !== null && next.leaf === null && next.step.needsReadiness
  })
}

/** Whether the header of a whole stack waits for an approval (Awaiting): a focus asks GitHub for the pull requests at once */
export function isStackAwaitingApproval(): boolean {
  return Object.keys(branchSyncStore.getState().stackBranches).some(root => {
    const next = getStackNextStepNow(root)
    return next?.leaf === null && next.step.kind === "awaiting"
  })
}

/**
 * Tells that approvals arrived (`findApprovalNews`): of a branch (`tellApprovalNews`), or in the pull request of the
 * whole stack, "Ilya Korennoy approved the whole air stack" / "#12 · IJPL Code Platform", with "Safe Push Whole Stack"
 * first (the start of the whole stack, decided again at the click) and "Open Pull Request"
 */
export function tellReadinessNews(news: ApprovalNews): void {
  const root = stackRootOfReadinessKey(news.branchName)
  if (root === null) {
    tellApprovalNews(news)
    return
  }
  announceApproval(news, getStackPullRequest(root), {
    label: "Safe Push Whole Stack",
    onClick: () => void requestSafePush(news.branchName, { mode: "rebaseAutosquash", via: "click" }),
  })
}
