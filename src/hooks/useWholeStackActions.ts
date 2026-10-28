import { GitPullRequestClosedIcon } from "lucide-react"
import { actionsOf, getPushWithChildrenItem, withStepReason } from "@/hooks/useBranchContextActions"
import { getBranchNextStep, getRobotItems } from "@/hooks/useBranchNextStep"
import { requestSafePush } from "@/hooks/useSafePush"
import type { StackNextStep } from "@/hooks/useStackNextStep"
import { getCreateStackPullRequestItem, getStackNextStep, wholeStackRestOf } from "@/hooks/useStackNextStep"
import type { ActionId, PatronusPushMode } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { openBranchReviewWindow, openWholeStackReviewWindow } from "@/lib/branch-review-window"
import type { MenuItem } from "@/lib/menu"
import { describeStart } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import { describePushToBaseline, formatPushToBaselineLabel } from "@/lib/push-to-baseline"
import { describeOrphanedStackPullRequest, describeWholeStackLanding, formatWholeStack, stackReadinessKey } from "@/lib/whole-stack"
import type { ActionItem } from "@/presenters/actions"
import { presentAction } from "@/presenters/actions"
import { NO_NEXT_STEP } from "@/presenters/next-step"
import { getRequestReviewItem } from "@/presenters/review"
import { branchSyncStore, getStackBranch, isForkingRoot, mainLineOf } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { openInline } from "@/stores/inline-row-action"
import { getStackLandedBy } from "@/stores/landed-stacks"
import { getStackRobot } from "@/stores/patronus"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"
import type { SafePushMode } from "@/stores/safe-push"

/** Where the actions of the whole stack render */
export interface WholeStackMenuOptions {
  /**
   * Push with Children of the root (`getPushWithChildrenItem`): in the "⋯" of the island; the root's own menu has it
   * beside its push
   */
  readonly withPush: boolean
  /**
   * The Safe Push of the last branch of the main line while the sync didn't build the stack branch, so the stack lands in
   * pieces (`getStackNotBuilt`): in the "⋯" of the island, at its header; the root's menu has the branches' rows
   */
  readonly landInPieces?: boolean
}

/**
 * The actions of the whole stack of `root`, one list for every surface that shows them (the "⋯" of the stack island,
 * the "Whole Stack" submenu of the root's menu, so the root's preview too), in this order:
 * - "Safe Push Whole Stack" and "Dry Run Whole Stack" (`getWholeStackStartItems`), then, in the island while the stack
 *   branch is not built, the Safe Push of the main line (`landInPieces`);
 * - the pull request of the whole stack: "Open Pull Request #12" and, while it is open, "Request Review…" of it (the
 *   code owners of the whole stack, its stack branch pushed first; the popover at the header of the island, else at the
 *   root's row), else "Create Pull Request of the Stack…" (its stack branch `{prefix}/stack/<root>` into the baseline;
 *   disabled with the reason while the sync didn't build it); one that no longer matches the stack
 *   (`describeOrphanedStackPullRequest`) says so and offers "Close Pull Request #12…", asked in the root's row, never
 *   closed by itself;
 * - Push with Children of the root (`withPush`);
 * - "Review Whole Stack": the review window of the stack branch; of a straight chain, which has none, the review of its
 *   last branch from the root;
 * - the actions of the robot of the whole stack (`getRobotActions`: restart, cancel, the prompt, Patronus, its pull
 *   request unless it is the stack's).
 * Empty for a branch that heads no stack.
 */
export function getWholeStackMenuItems(root: string, options: WholeStackMenuOptions): MenuItem[] {
  const stack = getStackBranch(root)
  const items: MenuItem[] = getWholeStackStartItems(root)
  if (options.landInPieces === true) {
    const piece = getLandInPiecesItem(root)
    if (piece !== null) {
      items.push(piece)
    }
  }
  const pullRequest = getStackPullRequest(root)
  const orphaned = describeOrphanedStackPullRequest(root, pullRequest, stack, getStackLandedBy(root))
  if (pullRequest !== undefined) {
    const open = stackItem(root, "open-stack-pull-request", {
      label: `Open Pull Request #${pullRequest.number}`,
      description: orphaned ?? (pullRequest.state === "merged" ? `#${pullRequest.number} is merged` : `The pull request of ${formatWholeStack(root)}`),
      onSelect: () => openPullRequest(pullRequest),
    })
    if (open !== null) {
      items.push(open)
    }
  }
  // the code owners of the whole stack, its stack branch pushed first: the step of the whole stack when it is the one (it
  // says whose approval is missing, or that nobody of a group can approve); one that no longer matches the stack is closed
  const next = getStackNextStep(root)
  const stepItem = next?.leaf === null && next.step.kind === "requestReview" ? next.step.item : null
  const requestReview = orphaned === null ? (stepItem ?? getRequestReviewItem(stackReadinessKey(root))) : null
  if (requestReview !== null) {
    items.push(requestReview)
  }
  if (orphaned !== null && pullRequest !== undefined) {
    items.push({
      id: "close-stack-pull-request",
      label: `Close Pull Request #${pullRequest.number}…`,
      icon: GitPullRequestClosedIcon,
      description: orphaned,
      onSelect: () => {
        requestReveal({ branchName: root, commitIds: [] })
        openInline("close-stack-pull-request", root)
      },
    })
  } else {
    const create = getCreateStackPullRequestItem(root)
    if (create !== null) {
      items.push(create)
    }
  }
  if (options.withPush) {
    const push = getPushWithChildrenItem(root)
    if (push !== null) {
      items.push(push)
    }
  }
  const review = getWholeStackReviewItem(root)
  if (review !== null) {
    items.push(review)
  }
  const robot = getStackRobot(root)
  if (robot !== undefined) {
    // its pull request is the stack's one, which "Open Pull Request #12" already opens
    items.push(...getRobotItems(robot).filter(item => item.id !== "open-robot-pull-request" || robot.pullRequest?.url !== pullRequest?.url))
  }
  return items
}

/** The item of the action `id` of the whole stack of `root` as Rust offers it; null while it doesn't */
function stackItem(root: string, id: ActionId, item: ActionItem, step?: string | null): MenuItem | null {
  return presentAction(actionsOf(stackReadinessKey(root)), id, item, { subject: root, ...(step !== undefined && { step }) })
}

/**
 * "Safe Push Whole Stack" and "Dry Run Whole Stack": the start of the whole stack (`requestSafePush` of its key,
 * `stackReadinessKey`), from its stack branch while the stack forks, of the last branch of a straight chain otherwise;
 * at the header of its island, else at the root's row. Offered as Rust decides them of the whole stack (none off Space),
 * disabled with the reasons of the next step of the whole stack (`next`, `getStackNextStep`: its stack branch not built,
 * a robot of a branch of it, its approvals). The menus of the whole stack and the Safe Push popover of a branch of it
 * render them (a component passes the step it subscribed to, `useStackNextStep`).
 */
export function getWholeStackStartItems(root: string, next: StackNextStep | null = getStackNextStep(root)): MenuItem[] {
  const step = next?.step ?? NO_NEXT_STEP
  const leaf = next?.leaf ?? null
  const key = stackReadinessKey(root)
  const lands = (mode: PatronusPushMode): string => {
    const target = getReadinessTarget()
    if (leaf === null) {
      const rest = wholeStackRestOf(root)
      return describeWholeStackLanding(rest.members, rest.commitCount, target, mode, rest.landed)
    }
    return describeStart(mode, leaf, mainLineOf(branchSyncStore.getState(), root).length, target)
  }
  const start = (id: ActionId, mode: SafePushMode, blocker: string | null, item: Partial<ActionItem> = {}): MenuItem | null =>
    withStepReason(stackItem(root, id, { description: lands(mode), onSelect: () => void requestSafePush(key, { mode, via: "click" }), ...item }, blocker), blocker)
  return [
    start("safe-push-whole-stack", "rebaseAutosquash", step.safePushBlocker),
    start("dry-run-whole-stack", "dryRun", step.dryRunBlocker),
    // a repository that lands by a push of the copy into the target pushes its stack branch, in place of the robot
    pushToBaselineOf(root, leaf, step.safePushBlocker, start),
    // one that lands by a merge of the pull request merges the one of its stack branch, named as its step names it
    start("merge-pull-request", "rebaseAutosquash", step.safePushBlocker, {
      label: step.item?.id === "merge-pull-request" ? step.item.label : "Merge",
      ...(step.item?.id === "merge-pull-request" && step.item.description !== undefined && { description: step.item.description }),
    }),
  ].filter(item => item !== null)
}

/**
 * "Push to master · 7" of the whole stack of `root`, where the repository lands by a push of the copy into the target:
 * of its stack branch while it forks, of the last branch `leaf` of a straight chain otherwise; null while Rust offers
 * none
 */
function pushToBaselineOf(
  root: string,
  leaf: string | null,
  blocker: string | null,
  start: (id: ActionId, mode: SafePushMode, blocker: string | null, item: Partial<ActionItem>) => MenuItem | null,
): MenuItem | null {
  const target = getReadinessTarget()
  const rest = leaf === null ? wholeStackRestOf(root) : null
  const count = rest?.members.length ?? mainLineOf(branchSyncStore.getState(), root).length
  const description =
    rest === null ? describePushToBaseline(leaf ?? root, count, target) : describeWholeStackLanding(rest.members, rest.commitCount, target, "rebaseAutosquash", rest.landed, true)
  return start("push-to-baseline", "rebaseAutosquash", blocker, { label: formatPushToBaselineLabel(target, count), description })
}

/**
 * "Safe Push deploy": while the sync didn't build the stack branch of a stack that forks, the stack lands in pieces, the
 * Safe Push of the last branch of its main line the first, its popover at the island's header; as Rust offers it,
 * disabled with the reason of the next step of that branch; null otherwise
 */
function getLandInPiecesItem(root: string): MenuItem | null {
  const stack = getStackBranch(root)
  const mainLine = mainLineOf(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1)
  if (stack === undefined || leaf === undefined) {
    return null
  }
  const blocker = (getBranchNextStep(leaf) ?? NO_NEXT_STEP).safePushBlocker
  const item = stackItem(
    root,
    "safe-push-main-line",
    {
      label: `Safe Push ${leaf}`,
      description: `Lands the stack in pieces, its main line first: ${mainLine.length} of ${formatBranchCount(stack.members.length)} into ${getReadinessTarget()} in one Safe Push`,
      onSelect: () => void requestSafePush(leaf, { mode: "rebaseAutosquash", via: "click", placement: "stack-header" }),
    },
    blocker,
  )
  return withStepReason(item, blocker)
}

/**
 * "Review Whole Stack", as Rust offers it of the whole stack: the review window of the stack branch of a stack that forks
 * (one part, every commit of the stack), disabled while the sync didn't build it; of a straight chain, the review of its
 * last branch from the root (`range: "all"`), which holds them all, disabled as that branch's review; null for a branch
 * that heads no stack
 */
function getWholeStackReviewItem(root: string): MenuItem | null {
  const mainLine = mainLineOf(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1) ?? root
  const forks = isForkingRoot(branchSyncStore.getState(), root)
  return stackItem(root, "review-whole-stack", {
    description: forks
      ? `The diff of every commit of ${formatWholeStack(root)}, in a window`
      : `The diff of ${formatBranchCount(mainLine.length)} from ${root} to ${leaf}, the files of each code owner, in a window`,
    onSelect: () => void (forks ? openWholeStackReviewWindow(root) : openBranchReviewWindow(leaf, { range: "all" })),
  })
}
