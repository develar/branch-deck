import { GitPullRequestCreateIcon } from "lucide-react"
import type { StartStepOptions } from "@/hooks/useBranchNextStep"
import {
  announceApproval,
  busyFact,
  fixLanding,
  getBranchNextStepNow,
  getLandedIn,
  nextStepSources,
  pendingFactOf,
  tellApprovalNews,
  unavailableFactOf,
  useBranchNextStep,
  useNextStepStores,
} from "@/hooks/useBranchNextStep"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import { formatBranchCount } from "@/lib/branch"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { getBranchesGitHubRepository } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import { stackParkedState } from "@/lib/parked"
import { formatLandingSubject, getSafePushSlot } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { changesRequestedBy, formatAvailabilityHint, openPullRequest, waitingReviewers } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import type { StackLandedPart } from "@/lib/whole-stack"
import { describeWholeStackLanding, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { formatReasonOrNull } from "@/presenters/reasons"
import type { StackBranch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getOpenPullRequest, getStackBranch, getStackNotBuilt, getWholeStack, isForkingRoot, useBranchSync } from "@/stores/branch-sync"
import { openStackPullRequestDialog } from "@/stores/create-pull-request-dialog"
import {
  getStackLandedReason,
  getStackLanding,
  getStackLandingMember,
  getStackPushHold,
  getStackRobot,
  isPatronusAvailable,
  isRobotFailing,
  isRobotNeedingReview,
} from "@/stores/patronus"
import { getPullRequestAvailability, getPullRequestListing, getStackPullRequest } from "@/stores/pull-requests"
import { openRequestReview } from "@/stores/request-review-popover"
import { freshReadiness, getReadinessTarget, getReviewReadiness } from "@/stores/review-readiness"

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

/** What the Safe Push of the whole stack lands now, as Rust decides it (`StackLanding.rest`, `whole_stack_rest`) */
export interface WholeStackRest {
  /** The branches left, as a sentence names them: the main line first, then the side branches */
  readonly members: readonly string[]
  /** Their commits, of a branch landed with commits after it those only */
  readonly commitCount: number
  /** The branches merged Safe Pushes landed whole and who landed them; null while none is */
  readonly landed: StackLandedPart | null
  /** The robot of the whole stack itself landed every branch of it */
  readonly byOwnRobot: boolean
}

/**
 * What is left of the whole stack of `root` after merged Safe Pushes of pieces of it or of the whole stack landed
 * branches of it (`StackLanding.rest`): its stack branch still carries their commits (Patronus finds them in the target
 * and lands the rest), so the start names, counts and says only the rest; nothing left is the step of a landed branch
 * (row 1). Before the view of the robots has it, every branch of its stack branch
 */
export function wholeStackRestOf(root: string): WholeStackRest {
  const rest = getStackLanding(root).rest
  if (rest.members.length === 0 && rest.landed.length === 0) {
    const stack = getStackBranch(root)
    return { members: stack?.members ?? [], commitCount: stack?.commitCount ?? 0, landed: null, byOwnRobot: false }
  }
  return {
    members: rest.members,
    commitCount: rest.commitCount,
    landed: rest.landed.length === 0 ? null : { names: rest.landed, by: rest.by.map(formatLandingSubject) },
    byOwnRobot: rest.byOwnRobot,
  }
}

/**
 * "Create Pull Request of the Stack…": the pull request of the stack branch of a stack that forks into the baseline,
 * disabled with the reason while the sync didn't build it, and once merged Safe Pushes landed every branch of it
 * (`getStackLandedReason`: nothing of it is left to review or land); null for a stack that doesn't fork or without the
 * GitHub repository of the branches
 */
export function getCreateStackPullRequestItem(root: string): MenuItem | null {
  const stack = getStackBranch(root)
  if (stack === undefined || !isForkingRoot(branchSyncStore.getState(), root) || getBranchesGitHubRepository() === null) {
    return null
  }
  const blocker = formatReasonOrNull(getStackNotBuilt(root)) ?? getStackLandedReason(root)
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
function stackPullRequestFactOf(stack: StackBranch): NextStepPullRequest | null {
  const pullRequest = getStackPullRequest(stack.root)
  if (pullRequest === undefined) {
    return null
  }
  const base = getOpenPullRequest(stackReadinessKey(stack.root))?.base
  const mergedCurrent = pullRequest.state === "merged" && !isReviewCommitOutdated(getReviewCommitState(pullRequest.headRefOid, stack.tip ?? undefined, stack.remoteStatus))
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base: pullRequest.baseRefName ?? "",
    // where an open one goes, as Rust reads it (`pull_request::base_of`); one into a virtual branch is no retarget of
    // the whole stack, another base
    intoTarget: base === undefined || base.kind === "target",
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
 * its state: only a built one lands, else the reason, `StackLanding.start`), the robot of the whole stack, the robot
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
  const rest = wholeStackRestOf(root)
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
  const landingMember = getStackLandingMember(root)
  // why its starts can't, as Rust decides it of the view: its stack branch not built, a robot on its commit, one of a
  // branch of it (`domain_policy::land::start_refusal`)
  const landing = getStackLanding(root)
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
    errorReason: null,
    commitCount: stack.commitCount,
    // its own running start is no reason against the start it runs
    busy: busyFact(key, options),
    blocked: false,
    startBlocker: formatReasonOrNull(landing.start),
    dryRunStartBlocker: formatReasonOrNull(landing.dryRun),
    landingPart: landingMember,
    held: getStackPushHold(root) !== null,
    parked: stackParkedState(getBranch(stack.root)?.structure.stackParked ?? null),
    landedIn,
    // merged Safe Pushes, of pieces of it or its own, landed every branch of it: nothing is left to land
    landedWith: getStackLandedReason(root),
    robot: slot,
    robotPullRequest: robot?.pullRequest?.number ?? null,
    restart,
    // a refresh that failed for a while keeps the last listing: its pull requests are known as of then
    pullRequestsAvailable: getPullRequestListing() === "listed",
    pullRequestsUnlisted: getPullRequestListing() === "unlisted" ? formatAvailabilityHint(getPullRequestAvailability()) : null,
    pullRequest: stackPullRequestFactOf(stack),
    target,
    // what merged Safe Pushes landed of it is in the target: the robot lands the rest
    landsBranches: rest.members.length,
    startDescription: describeWholeStackLanding(rest.members, rest.commitCount, target, "rebaseAutosquash", rest.landed),
    // the repository view computes them of the branches, not of the whole stack: its start's gate checks it
    landingNeeds: null,
    readiness: readiness === undefined ? null : { readiness: readiness.readiness, stale: readiness.stale },
    pending: pendingFactOf(robot, null),
    unavailable: unavailableFactOf(robot),
    forStart: options !== null,
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
      // in the column of steps of the island header, which is the whole stack's
      createPullRequest: stackStepItem(getCreateStackPullRequestItem(root)),
      requestReview: getRequestReviewItem(getOpenPullRequest(key), () => openRequestReview(key)),
      fixLanding,
    },
  }
}

/** "Create Pull Request…" in a column of steps, at the header of the whole stack, which needs no "of the Stack" */
function stackStepItem(item: MenuItem | null): MenuItem | null {
  return item === null ? null : { ...item, compactLabel: "Create Pull Request…" }
}

/** By root: the step shown, its key, and the store states it was computed for */
const shownSteps = new Map<string, { sources: readonly unknown[]; key: string; step: StackNextStep | null }>()

/**
 * What a surface renders of the step of the stack branch: one object while it stays. The step of the last branch of a
 * straight chain is that branch's own, one object while it stays (`getBranchNextStepNow`), so it is compared as it is
 */
function stackStepKey(next: StackNextStep | null, root: string): string {
  if (next === null || next.leaf !== null) {
    return ""
  }
  const { step } = next
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
  const same = next?.leaf == null ? shown?.key === key && shown.step?.leaf == null : shown?.step?.leaf === next.leaf && shown.step.step === next.step
  const kept = shown !== undefined && same ? shown.step : next
  shownSteps.set(root, { sources, key, step: kept })
  return kept
}

function computeStackNextStep(root: string): StackNextStep | null {
  const stack = getStackBranch(root)
  const whole = getWholeStack(root)
  if (whole?.kind === "forking" && stack !== undefined) {
    return { leaf: null, step: getBranchNextStep(stackFactsOf(stack)) }
  }
  // a straight chain: its last branch that is not parked lands the whole stack (`WholeStackView.chain`); the parked
  // branches at its end stay local, the island has them in the Parked card
  if (whole?.kind !== "chain") {
    return null
  }
  const step = getBranchNextStepNow(whole.leaf)
  return step === null ? null : { leaf: whole.leaf, step }
}

/**
 * The next step of the whole stack of `root`, re-rendering when it changes (the header of its island, the root's menu
 * and preview); the readiness of the whole stack is asked while it shows
 */
export function useStackNextStep(root: string): StackNextStep | null {
  const forks = useBranchSync(state => isForkingRoot(state, root))
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
 * Tells that approvals arrived (the news of the session): of a branch (`tellApprovalNews`), or in the pull request of the
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
