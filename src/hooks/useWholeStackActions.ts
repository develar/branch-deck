import { FileDiffIcon, FlaskConicalIcon, GitPullRequestClosedIcon, GitPullRequestIcon, ShieldCheckIcon } from "lucide-react"
import { getPushWithChildrenItem, getSafePushBlocker } from "@/hooks/useBranchContextActions"
import { requestSafePush } from "@/hooks/useSafePush"
import type { StackNextStep } from "@/hooks/useStackNextStep"
import { getCreateStackPullRequestItem, getStackNextStepNow, getWholeStackMembers, isForkingStack } from "@/hooks/useStackNextStep"
import type { PatronusPushMode } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow, openWholeStackReviewWindow } from "@/lib/branch-review-window"
import type { MenuItem } from "@/lib/menu"
import { describeStart } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { describeOrphanedStackPullRequest, describeWholeStackLanding, formatWholeStack, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import { branchSyncStore, getBranch, getStackBranch, stackMainLineIn } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { openInline } from "@/stores/inline-row-action"
import { getStackLandedBy } from "@/stores/landed-stacks"
import { getStackRobot, isPatronusAvailable } from "@/stores/patronus"
import { getStackPullRequest } from "@/stores/pull-requests"
import { openRequestReview } from "@/stores/request-review-popover"
import { getReadinessTarget, getReviewReadiness } from "@/stores/review-readiness"

/** Where the actions of the whole stack render */
export interface WholeStackMenuOptions {
  /**
   * Push with Children of the root (`getPushWithChildrenItem`): in the "⋯" of the island; the root's own menu has it
   * beside its push
   */
  readonly withPush: boolean
  /**
   * The Safe Push of the last branch of the main line while the sync didn't build the stack branch, so the stack lands in
   * pieces (`getStackBranchBlocker`): in the "⋯" of the island, at its header; the root's menu has the branches' rows
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
  const forks = isForkingStack(stack)
  const items: MenuItem[] = getWholeStackStartItems(root)
  if (options.landInPieces === true) {
    const piece = getLandInPiecesItem(root)
    if (piece !== null) {
      items.push(piece)
    }
  }
  const pullRequest = getStackPullRequest(root)
  const orphaned = describeOrphanedStackPullRequest(root, pullRequest, stack, getStackLandedBy(root))
  if (pullRequest !== undefined && pullRequest.state !== "closed") {
    items.push({
      id: "open-stack-pull-request",
      label: `Open Pull Request #${pullRequest.number}`,
      icon: GitPullRequestIcon,
      description: orphaned ?? (pullRequest.state === "merged" ? `#${pullRequest.number} is merged` : `The pull request of ${formatWholeStack(root)}`),
      onSelect: () => openPullRequest(pullRequest),
    })
  }
  const requestReview = orphaned === null ? getWholeStackRequestReviewItem(root) : null
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
  } else if (pullRequest === undefined || pullRequest.state === "closed") {
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
  const review = getWholeStackReviewItem(root, forks)
  if (review !== null) {
    items.push(review)
  }
  const robot = getStackRobot(root)
  if (robot !== undefined) {
    // its pull request is the stack's one, which "Open Pull Request #12" already opens
    items.push(...getRobotActions(robot, null).filter(item => item.id !== "open-robot-pull-request" || robot.pullRequest?.url !== pullRequest?.url))
  }
  return items
}

/**
 * "Safe Push Whole Stack" and "Dry Run Whole Stack": the start of the whole stack (`requestSafePush` of its key,
 * `stackReadinessKey`), from its stack branch while the stack forks, of the last branch of a straight chain otherwise;
 * at the header of its island, else at the root's row. Disabled with the reasons of the next step of the whole stack
 * (`next`, `getStackNextStepNow`: its stack branch not built, a robot of a branch of it, its approvals); none where
 * Patronus doesn't answer or for a branch that heads no stack. The menus of the whole stack and the Safe Push popover of
 * a branch of it render them (a component passes the step it subscribed to, `useStackNextStep`).
 */
export function getWholeStackStartItems(root: string, next: StackNextStep | null = getStackNextStepNow(root)): MenuItem[] {
  if (!isPatronusAvailable() || next === null || next.step.kind === "hidden") {
    return []
  }
  const { step, leaf } = next
  const key = stackReadinessKey(root)
  const lands = (mode: PatronusPushMode): string => {
    const stack = getStackBranch(root)
    const target = getReadinessTarget()
    if (leaf === null && stack !== undefined) {
      return describeWholeStackLanding(getWholeStackMembers(stack), stack.commitCount, target, mode)
    }
    return describeStart(mode, leaf ?? root, stackMainLineIn(branchSyncStore.getState(), root).length, target)
  }
  return [
    {
      id: "safe-push-whole-stack",
      label: "Safe Push Whole Stack",
      icon: ShieldCheckIcon,
      disabled: step.safePushBlocker !== null,
      description: step.safePushBlocker ?? lands("rebaseAutosquash"),
      onSelect: () => void requestSafePush(key, { mode: "rebaseAutosquash", via: "click" }),
    },
    {
      id: "dry-run-whole-stack",
      label: "Dry Run Whole Stack",
      icon: FlaskConicalIcon,
      disabled: step.dryRunBlocker !== null,
      description: step.dryRunBlocker ?? lands("dryRun"),
      onSelect: () => void requestSafePush(key, { mode: "dryRun", via: "click" }),
    },
  ]
}

/**
 * "Request Review…" of the open pull request of the whole stack of `root`: the step of the whole stack when it is the
 * one (it says whose approval is missing, or that nobody of a group can approve), else the item of
 * `getRequestReviewItem` with the code owners Patronus suggests for the stack branch (the readiness of its key); its
 * popover opens at the header of the island, else at the root's row (`openRequestReview` of the key). Null without an
 * open pull request.
 */
function getWholeStackRequestReviewItem(root: string): MenuItem | null {
  const key = stackReadinessKey(root)
  const next = getStackNextStepNow(root)
  if (next?.leaf === null && next.step.kind === "requestReview" && next.step.item !== null) {
    return next.step.item
  }
  return getRequestReviewItem(getStackPullRequest(root), getReviewReadiness(key)?.readiness ?? null, () => openRequestReview(key))
}

/**
 * "Safe Push deploy": while the sync didn't build the stack branch of a stack that forks (`getStackBranchBlocker`), the
 * stack lands in pieces, the Safe Push of the last branch of its main line the first, its popover at the island's
 * header; null otherwise
 */
function getLandInPiecesItem(root: string): MenuItem | null {
  const stack = getStackBranch(root)
  if (!isPatronusAvailable() || !isForkingStack(stack) || getStackBranchBlocker(root, stack) === null) {
    return null
  }
  const mainLine = stackMainLineIn(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1)
  const branch = leaf === undefined ? undefined : getBranch(leaf)
  if (leaf === undefined || branch === undefined) {
    return null
  }
  const blocker = getSafePushBlocker(branch)
  return {
    id: "safe-push-main-line",
    label: `Safe Push ${leaf}`,
    icon: ShieldCheckIcon,
    disabled: blocker !== null,
    description:
      blocker ?? `Lands the stack in pieces, its main line first: ${mainLine.length} of ${formatBranchCount(stack.members.length)} into ${getReadinessTarget()} in one Safe Push`,
    onSelect: () => void requestSafePush(leaf, { mode: "rebaseAutosquash", via: "click", placement: "stack-header" }),
  }
}

/**
 * "Review Whole Stack": the review window of the stack branch of a stack that forks (one part, every commit of the
 * stack); of a straight chain, the review of its last branch from the root (`range: "all"`), which holds them all; null
 * for a branch that heads no stack
 */
function getWholeStackReviewItem(root: string, forks: boolean): MenuItem | null {
  const stack = getStackBranch(root)
  if (forks) {
    const tip = stack?.tip ?? null
    return {
      id: "review-whole-stack",
      label: "Review Whole Stack",
      icon: FileDiffIcon,
      disabled: tip === null,
      description:
        tip === null
          ? (getStackBranchBlocker(root, stack) ?? "The stack branch is not built yet: sync first")
          : `The diff of every commit of ${formatWholeStack(root)}, in a window`,
      onSelect: () => void openWholeStackReviewWindow(root),
    }
  }
  const mainLine = stackMainLineIn(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1)
  if (leaf === undefined || mainLine.length < 2) {
    return null
  }
  // as the branch menu's: a leaf without commits, failed or blocked has nothing to review
  const branch = getBranch(leaf)
  const blocker = branch === undefined ? "The branch is not in the table" : branchReviewBlocker(branch)
  return {
    id: "review-whole-stack",
    label: "Review Whole Stack",
    icon: FileDiffIcon,
    disabled: blocker !== null,
    description: blocker ?? `The diff of ${formatBranchCount(mainLine.length)} from ${root} to ${leaf}, the files of each code owner, in a window`,
    onSelect: () => void openBranchReviewWindow(leaf, { range: "all" }),
  }
}
