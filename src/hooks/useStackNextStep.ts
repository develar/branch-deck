import { GitPullRequestCreateIcon } from "lucide-react"
import type { StepInputs } from "@/hooks/useBranchNextStep"
import { announceApproval, busyOf, getBranchNextStep, getStepInputs, stepActionsOf, tellApprovalNews, useBranchNextStep, useStepInputs } from "@/hooks/useBranchNextStep"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import type { Landed, Reason, StackLanding } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { getBranchesGitHubRepository } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import { formatLandingSubject } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import type { StackLandedPart } from "@/lib/whole-stack"
import { describeWholeStackLanding, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart, useRowPart } from "@/model/store"
import type { BranchNextStep, StepContext } from "@/presenters/next-step"
import { presentNextStep } from "@/presenters/next-step"
import { formatLandedWhole, formatReasonOrNull } from "@/presenters/reasons"
import type { StackBranch } from "@/stores/branch-sync"
import { getLandedState, getStackBranch, getStackNotBuilt, getWholeStack, useForkingStackNotBuilt, useStackBranch, useWholeStack } from "@/stores/branch-sync"
import { openStackPullRequestDialog } from "@/stores/create-pull-request-dialog"
import { getStackLanding, useStackLanding } from "@/stores/patronus"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"

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
 * What is left of the whole stack after merged Safe Pushes of pieces of it or of the whole stack landed branches of it
 * (`rest` of its `StackLanding`): its stack branch still carries their commits (Patronus finds them in the target and
 * lands the rest), so the start names, counts and says only the rest. Before the view of the robots has it, every
 * branch of its stack branch (`stack`)
 */
function wholeStackRestIn(rest: StackLanding["rest"], stack: StackBranch | undefined): WholeStackRest {
  if (rest.members.length === 0 && rest.landed.length === 0) {
    return { members: stack?.members ?? [], commitCount: stack?.commitCount ?? 0, landed: null, byOwnRobot: false }
  }
  return {
    members: rest.members,
    commitCount: rest.commitCount,
    landed: rest.landed.length === 0 ? null : { names: rest.landed, by: rest.by.map(formatLandingSubject) },
    byOwnRobot: rest.byOwnRobot,
  }
}

/** `wholeStackRestIn` of the whole stack of `root` now */
export function wholeStackRestOf(root: string): WholeStackRest {
  return wholeStackRestIn(getStackLanding(root).rest, getStackBranch(root))
}

/** What "Create Pull Request of the Stack…" of a whole stack that forks reads */
interface StackPullRequestFacts {
  readonly stack: StackBranch | undefined
  /** Why the sync didn't build its stack branch; null when it did */
  readonly notBuilt: Reason | null
  /** What landed all of it */
  readonly landed: Landed | null | undefined
  readonly target: string
}

/**
 * "Create Pull Request of the Stack…": the pull request of the stack branch into the baseline, disabled with the reason
 * while the sync didn't build it, and once all of it landed (nothing of it is left to review or land)
 */
function stackPullRequestItemOf(root: string, facts: StackPullRequestFacts): MenuItem | null {
  const { stack, target } = facts
  if (stack === undefined) {
    return null
  }
  const blocker = formatReasonOrNull(facts.notBuilt) ?? formatLandedWhole(facts.landed)
  return {
    id: "create-stack-pull-request",
    label: "Create Pull Request of the Stack…",
    icon: GitPullRequestCreateIcon,
    disabled: blocker !== null,
    description: blocker ?? `The pull request of ${formatWholeStack(root)} into ${target}: the commits of ${formatBranchCount(stack.members.length)}, which one Safe Push lands`,
    onSelect: () => openStackPullRequestDialog(root),
  }
}

/**
 * "Create Pull Request of the Stack…" of `root` now (`stackPullRequestItemOf`), for the menus; null for a stack that
 * doesn't fork or without the GitHub repository of the branches
 */
export function getCreateStackPullRequestItem(root: string): MenuItem | null {
  if (getWholeStack(root)?.kind !== "forking" || getBranchesGitHubRepository() === null) {
    return null
  }
  return stackPullRequestItemOf(root, {
    stack: getStackBranch(root),
    notBuilt: getStackNotBuilt(root),
    landed: getLandedState(stackReadinessKey(root)),
    target: getReadinessTarget(),
  })
}

/** What the words of the step of a whole stack read beside those of a branch (`StepInputs`): what its start lands, its pull request */
interface StackInputs extends StepInputs {
  readonly rest: StackLanding["rest"]
  readonly stack: StackBranch | undefined
  readonly notBuilt: Reason | null
}

/** The context of the step of the whole stack of `root` over `inputs`; `landed` what landed all of it */
function stackContextOf(root: string, inputs: StackInputs, landed: Landed | null, forStart: boolean): StepContext {
  const key = stackReadinessKey(root)
  const rest = wholeStackRestIn(inputs.rest, inputs.stack)
  const create = stackPullRequestItemOf(root, { stack: inputs.stack, notBuilt: inputs.notBuilt, landed, target: inputs.target })
  return {
    subject: formatWholeStack(root),
    target: inputs.target,
    startDescription: describeWholeStackLanding(rest.members, rest.commitCount, inputs.target, "rebaseAutosquash", rest.landed),
    busy: busyOf(inputs, forStart),
    forStart,
    actions: stepActionsOf(key, inputs, {
      openPullRequest: () => {
        const pullRequest = getStackPullRequest(root)
        if (pullRequest !== undefined) {
          openPullRequest(pullRequest)
        }
      },
      // the pull request of the whole stack goes into the target by construction: nothing to retarget
      retarget: () => {},
      // in the column of steps of the island header, which is the whole stack's: no "of the Stack" there
      createPullRequest: create === null ? null : { ...create, compactLabel: "Create Pull Request…" },
    }),
  }
}

/**
 * The step of the whole stack of `root` now (`StackNextStep`), for menus built on open, keys, toasts and a start
 * (`forStart`); null for a branch that heads no stack
 */
export function getStackNextStep(root: string, options?: { readonly forStart: true }): StackNextStep | null {
  const whole = getWholeStack(root)
  if (whole?.kind === "chain") {
    const step = getBranchNextStep(whole.leaf, options)
    return step === null ? null : { leaf: whole.leaf, step }
  }
  const key = stackReadinessKey(root)
  const view = whole?.kind === "forking" ? getRowPart(key, "landing") : undefined
  if (view === undefined) {
    return null
  }
  const inputs: StackInputs = { ...getStepInputs(key, view.step), rest: getStackLanding(root).rest, stack: getStackBranch(root), notBuilt: getStackNotBuilt(root) }
  return { leaf: null, step: presentNextStep(view.step, stackContextOf(root, inputs, view.step.landed, options !== undefined)) }
}

/**
 * The next step of the whole stack of `root`, re-rendering when it changes (the header of its island, the root's menu
 * and preview): of its stack branch while it forks, the readiness of the whole stack asked while it shows; of the last
 * branch of a straight chain, that branch's own
 */
export function useStackNextStep(root: string): StackNextStep | null {
  const whole = useWholeStack(root)
  const forks = whole?.kind === "forking"
  const key = forks ? stackReadinessKey(root) : ""
  useShowReviewReadiness(key)
  const view = useRowPart(key, "landing")
  const inputs: StackInputs = {
    ...useStepInputs(key, view?.step),
    rest: useStackLanding(forks ? root : "").rest,
    stack: useStackBranch(forks ? root : ""),
    notBuilt: useForkingStackNotBuilt(root),
  }
  const leaf = whole?.kind === "chain" ? whole.leaf : null
  const leafStep = useBranchNextStep(leaf ?? "")
  if (leaf !== null) {
    return leafStep === null ? null : { leaf, step: leafStep }
  }
  return view === undefined ? null : { leaf: null, step: presentNextStep(view.step, stackContextOf(root, inputs, view.step.landed, false)) }
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
