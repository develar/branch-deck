import type { StepInputs } from "@/hooks/useBranchNextStep"
import {
  announceApproval,
  busyOf,
  getBranchNextStep,
  getStepInputs,
  stepActionsOf,
  tellApprovalNews,
  useBranchNextStep,
  useBranchStepWait,
  useStepInputs,
} from "@/hooks/useBranchNextStep"
import { useOperationWords } from "@/hooks/useOperationWords"
import { requestSafePush } from "@/hooks/useSafePush"
import type { ApprovalNews } from "@/lib/approval-news"
import type { ActionId, ActionState, HostedStack, NextStepView, WholeStackRest as RestOfStack } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import type { MenuItem } from "@/lib/menu"
import { formatLandingSubject } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import { sameJson } from "@/lib/record"
import type { StackLandedPart } from "@/lib/whole-stack"
import { describeWholeStackLanding, formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart, useRowPart } from "@/model/store"
import { presentAction } from "@/presenters/actions"
import type { BranchNextStep, StepContext, StepWait } from "@/presenters/next-step"
import { presentNextStep } from "@/presenters/next-step"
import type { OperationWords } from "@/presenters/operations"
import type { StackBranch } from "@/stores/branch-sync"
import { getStackBranch, getStackIslandRoots, getWholeStack, useStackBranch, useWholeStack } from "@/stores/branch-sync"
import { openStackPullRequestDialog } from "@/stores/create-pull-request-dialog"
import { getOperationWords } from "@/stores/indicators"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"

/**
 * The next step of the whole stack of a root ([Next Step](../../openspec/specs/branch/next-step/spec.md#the-whole-stack)): of its
 * stack branch `{prefix}/stack/<root>` while the stack forks, whose one robot lands every branch of it, side branches
 * too; of its last branch for a straight chain, which has no stack branch: that branch's Safe Push lands the whole stack
 */
export interface StackNextStep {
  /** The last branch of a straight chain, whose step it is; null: the step of the stack branch */
  readonly leaf: string | null
  readonly step: BranchNextStep
}

/** What the Safe Push of the whole stack lands now, as Rust decides it of the table as it is (`NextStepView.rest`, `whole_stack_rest`) */
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
 * (`rest` of the next step of the whole stack, of the table as it is): its stack branch still carries their commits
 * (Patronus finds them in the target and lands the rest), so the start names, counts and says only the rest. Before the
 * view has the step, every branch of its stack branch (`stack`)
 */
function wholeStackRestIn(rest: RestOfStack | null | undefined, stack: StackBranch | undefined): WholeStackRest {
  if (rest == null || (rest.members.length === 0 && rest.landed.length === 0)) {
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
  return wholeStackRestIn(getRowPart(stackReadinessKey(root), "landing")?.rest, getStackBranch(root))
}

/**
 * "Create Pull Request of the Stack…": the pull request of the stack branch into the baseline, as Rust offers it of the
 * whole stack (`actions`, `stack:<root>` of `RepositoryView.actions`: of a stack that forks on GitHub without a live
 * pull request of it, disabled while the sync didn't build it and once all of it landed); null while it doesn't
 */
function stackPullRequestItemOf(root: string, actions: readonly ActionState[] | undefined, stack: StackBranch | undefined, target: string, words: OperationWords): MenuItem | null {
  const members = stack?.members.length ?? 0
  return presentAction(
    actions,
    "create-stack-pull-request",
    {
      description: `The pull request of ${formatWholeStack(root)} into ${target}: the commits of ${formatBranchCount(members)}, which one Safe Push lands`,
      onSelect: () => openStackPullRequestDialog(root),
    },
    { ...words, subject: root },
  )
}

/** "Create Pull Request of the Stack…" of `root` now (`stackPullRequestItemOf`), for the menus */
export function getCreateStackPullRequestItem(root: string): MenuItem | null {
  return stackPullRequestItemOf(root, getRowPart(stackReadinessKey(root), "actions"), getStackBranch(root), getReadinessTarget(), getOperationWords())
}

/** The action of a whole stack whose words name the operation it runs as */
const CREATE_STACK_PULL_REQUEST: readonly ActionId[] = ["create-stack-pull-request"]

/** What the words of the step of a whole stack read beside those of a branch (`StepInputs`): what its start lands, its pull request */
interface StackInputs extends StepInputs {
  readonly rest: RestOfStack | null | undefined
  readonly stack: StackBranch | undefined
  /** The actions Rust offers of the whole stack: its Create Pull Request of the Stack… */
  readonly actions: readonly ActionState[] | undefined
  /** The words of the operations of the session its Create Pull Request of the Stack… names while it runs */
  readonly stackWords: OperationWords
}

/**
 * The context of the step of the whole stack of `root` over `inputs`; `byPush`: its step is a push of its stack branch
 * into the target (Push to master), which its words say
 */
function stackContextOf(root: string, inputs: StackInputs, forStart: boolean, byPush: boolean): StepContext {
  const key = stackReadinessKey(root)
  const rest = wholeStackRestIn(inputs.rest, inputs.stack)
  const create = stackPullRequestItemOf(root, inputs.actions, inputs.stack, inputs.target, inputs.stackWords)
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
 * The state the step of the whole stack of `root` waits in (`useStackNextStep(root)?.step.wait`), null while it waits
 * in none: of the parts of the whole stack only a step that waits, what its start lands and its Create Pull Request of
 * the Stack…, each kept while it says the same (`sameJson`), the leaf's wait for a straight chain, so the patches of
 * every next step after a sync (the landing needs) render no wait that stays; the slot of the header asks for its
 * readiness
 */
export function useStackStepWait(root: string): StepWait | null {
  const whole = useWholeStack(root)
  const forks = whole?.kind === "forking"
  const key = forks ? stackReadinessKey(root) : ""
  const step = useRowPart(key, "landing", view => (view?.step.wait === true ? view.step : undefined), sameJson)
  const rest = useRowPart(key, "landing", view => (view?.step.wait === true ? view.rest : undefined), sameJson)
  const actions = useRowPart(key, "actions", all => all?.filter(action => CREATE_STACK_PULL_REQUEST.includes(action.id)), sameJson)
  const inputs: StackInputs = {
    ...useStepInputs(key, step),
    rest,
    stack: useStackBranch(forks ? root : ""),
    actions,
    stackWords: useOperationWords(actions, CREATE_STACK_PULL_REQUEST),
  }
  const leaf = whole?.kind === "chain" ? whole.leaf : null
  const leafWait = useBranchStepWait(leaf ?? "")
  if (leaf !== null) {
    return leafWait
  }
  return step === undefined ? null : presentNextStep(step, stackContextOf(root, inputs, false, step.kind.kind === "pushToBaseline")).wait
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
  const inputs: StackInputs = {
    ...getStepInputs(key, view.step),
    rest: view.rest,
    stack: getStackBranch(root),
    actions: getRowPart(key, "actions"),
    stackWords: getOperationWords(),
  }
  return { leaf: null, step: presentNextStep(view.step, stackContextOf(root, inputs, options !== undefined, view.step.kind.kind === "pushToBaseline")) }
}

/**
 * The row of `name` hosts the whole stack of the stack that forks it heads, as Rust decides it
 * (`NextStepView.hosted`, `domain_policy::landing_plan::lands_as_whole_stack`): the whole stack has no row of its
 * own, so the slot, S and the first start of the "⋯" of its root's row are the whole stack's, the root's own start
 * "Safe Push Root Only" in its "⋯"; false of every other branch
 */
export function useHostsWholeStack(name: string): boolean {
  return useRowPart(name, "landing", view => view?.hosted != null)
}

/** `useHostsWholeStack` now, for menus built on open, keys and starts */
export function hostsWholeStackNow(name: string): boolean {
  return getRowPart(name, "landing")?.hosted != null
}

/**
 * The row of `name` shows the whole stack it hosts (`NextStepView.hosted`): its node and its Status cell are the whole
 * stack's, unless an island header shows that whole stack right above it, where the row keeps the root's own
 */
function showsHosted(name: string, view: NextStepView | undefined): view is NextStepView & { hosted: HostedStack } {
  return view?.hosted != null && !getStackIslandRoots().has(name)
}

/** The node the row of the subject `key` shows of its part of the view: the whole stack's it hosts (`showsHosted`), else its own */
export function rowNodeOf(key: string, view: NextStepView | undefined): NextStepView["node"] | undefined {
  return showsHosted(key, view) ? view.hosted.node : view?.node
}

/** The Status cell the row of `name` shows of its part of the view: of the whole stack it hosts (`showsHosted`), else its own */
export function rowStatusOf(name: string, view: NextStepView | undefined): NextStepView["status"] | undefined {
  return showsHosted(name, view) ? view.hosted.status : view?.status
}

/**
 * The state the step of the slot of the row of `name` waits in, which its Status cell and its preview say: of the whole
 * stack the row of a root hosts and shows (`showsHosted`, `useStackStepWait`), of the branch otherwise (`useBranchStepWait`)
 */
export function useRowStepWait(name: string): StepWait | null {
  const hosts = useRowPart(name, "landing", view => showsHosted(name, view))
  const own = useBranchStepWait(hosts ? "" : name)
  const stack = useStackStepWait(hosts ? name : "")
  return hosts ? stack : own
}

/**
 * The subject the slot of the row of `name` starts, as S, the preview and the first start of its "⋯" do: the whole stack
 * of the root whose row hosts it (`stackReadinessKey`), the branch otherwise
 */
export function slotSubjectOf(name: string): string {
  return hostsWholeStackNow(name) ? stackReadinessKey(name) : name
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
  const actions = useRowPart(key, "actions")
  const inputs: StackInputs = {
    ...useStepInputs(key, view?.step),
    rest: view?.rest,
    stack: useStackBranch(forks ? root : ""),
    actions,
    // the words of its Create Pull Request of the Stack… while it runs or waits
    stackWords: useOperationWords(actions, CREATE_STACK_PULL_REQUEST),
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
