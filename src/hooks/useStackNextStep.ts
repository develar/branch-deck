import type { StepInputs } from "@/hooks/useBranchNextStep"
import { announceApproval, busyOf, getBranchNextStep, getStepInputs, stepActionsOf, tellApprovalNews, useBranchNextStep, useStepInputs } from "@/hooks/useBranchNextStep"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import type { ActionState, StackLanding } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import type { MenuItem } from "@/lib/menu"
import { formatLandingSubject } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import type { StackLandedPart } from "@/lib/whole-stack"
import { describeWholeStackLanding, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart, useRowPart } from "@/model/store"
import { presentAction } from "@/presenters/actions"
import type { BranchNextStep, StepContext } from "@/presenters/next-step"
import { presentNextStep } from "@/presenters/next-step"
import type { StackBranch } from "@/stores/branch-sync"
import { getStackBranch, getWholeStack, useStackBranch, useWholeStack } from "@/stores/branch-sync"
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

/**
 * "Create Pull Request of the Stack…": the pull request of the stack branch into the baseline, as Rust offers it of the
 * whole stack (`actions`, `stack:<root>` of `RepositoryView.actions`: of a stack that forks on GitHub without a live
 * pull request of it, disabled while the sync didn't build it and once all of it landed); null while it doesn't
 */
function stackPullRequestItemOf(root: string, actions: readonly ActionState[] | undefined, stack: StackBranch | undefined, target: string): MenuItem | null {
  const members = stack?.members.length ?? 0
  return presentAction(
    actions,
    "create-stack-pull-request",
    {
      description: `The pull request of ${formatWholeStack(root)} into ${target}: the commits of ${formatBranchCount(members)}, which one Safe Push lands`,
      onSelect: () => openStackPullRequestDialog(root),
    },
    { subject: root },
  )
}

/** "Create Pull Request of the Stack…" of `root` now (`stackPullRequestItemOf`), for the menus */
export function getCreateStackPullRequestItem(root: string): MenuItem | null {
  return stackPullRequestItemOf(root, getRowPart(stackReadinessKey(root), "actions"), getStackBranch(root), getReadinessTarget())
}

/** What the words of the step of a whole stack read beside those of a branch (`StepInputs`): what its start lands, its pull request */
interface StackInputs extends StepInputs {
  readonly rest: StackLanding["rest"]
  readonly stack: StackBranch | undefined
  /** The actions Rust offers of the whole stack: its Create Pull Request of the Stack… */
  readonly actions: readonly ActionState[] | undefined
}

/**
 * The context of the step of the whole stack of `root` over `inputs`; `byPush`: its step is a push of its stack branch
 * into the target (Push to master), which its words say
 */
function stackContextOf(root: string, inputs: StackInputs, forStart: boolean, byPush: boolean): StepContext {
  const key = stackReadinessKey(root)
  const rest = wholeStackRestIn(inputs.rest, inputs.stack)
  const create = stackPullRequestItemOf(root, inputs.actions, inputs.stack, inputs.target)
  return {
    subject: formatWholeStack(root),
    target: inputs.target,
    startDescription: describeWholeStackLanding(rest.members, rest.commitCount, inputs.target, "rebaseAutosquash", rest.landed, byPush),
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
  const inputs: StackInputs = { ...getStepInputs(key, view.step), rest: getStackLanding(root).rest, stack: getStackBranch(root), actions: getRowPart(key, "actions") }
  return { leaf: null, step: presentNextStep(view.step, stackContextOf(root, inputs, options !== undefined, view.step.kind.kind === "pushToBaseline")) }
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
    actions: useRowPart(key, "actions"),
  }
  const leaf = whole?.kind === "chain" ? whole.leaf : null
  const leafStep = useBranchNextStep(leaf ?? "")
  if (leaf !== null) {
    return leafStep === null ? null : { leaf, step: leafStep }
  }
  return view === undefined ? null : { leaf: null, step: presentNextStep(view.step, stackContextOf(root, inputs, false, view.step.kind.kind === "pushToBaseline")) }
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
