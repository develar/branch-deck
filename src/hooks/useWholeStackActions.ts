import { FileDiffIcon, GitPullRequestClosedIcon, GitPullRequestCreateIcon, GitPullRequestIcon } from "lucide-react"
import { getPushWithChildrenItem } from "@/hooks/useBranchContextActions"
import { formatBranchCount } from "@/lib/branch"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow, openWholeStackReviewWindow } from "@/lib/branch-review-window"
import { getBranchesGitHubRepository } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import { getRobotActions } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { describeOrphanedStackPullRequest, formatWholeStack, getStackBranchBlocker } from "@/lib/whole-stack"
import { branchSyncStore, getBranch, getStackBranch, stackMainLineIn } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { openStackPullRequestDialog } from "@/stores/create-pull-request-dialog"
import { openInline } from "@/stores/inline-row-action"
import { getStackLandedBy } from "@/stores/landed-stacks"
import { getStackRobot } from "@/stores/patronus"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"

/** Where the actions of the whole stack render */
export interface WholeStackMenuOptions {
  /**
   * Push with Children of the root (`getPushWithChildrenItem`): in the "⋯" of the island; the root's own menu has it
   * beside its push
   */
  readonly withPush: boolean
}

/**
 * The actions of the whole stack of `root`, one list for every surface that shows them (the "⋯" of the stack island,
 * the "Whole Stack" submenu of the root's menu, so the root's preview too), in this order:
 * - the pull request of the whole stack: "Open Pull Request #12", else "Create Pull Request of the Stack…" (its stack
 *   branch `{prefix}/stack/<root>` into the baseline; disabled with the reason while the sync didn't build it); one that
 *   no longer matches the stack (`describeOrphanedStackPullRequest`) says so and offers "Close Pull Request #12…",
 *   asked in the root's row, never closed by itself;
 * - Push with Children of the root (`withPush`);
 * - "Review Whole Stack": the review window of the stack branch; of a straight chain, which has none, the review of its
 *   last branch from the root;
 * - the actions of the robot of the whole stack (`getRobotActions`: restart, cancel, the prompt, Patronus, its pull
 *   request unless it is the stack's).
 * The Safe Push of the whole stack and its Dry Run come with its form (spec/branch/whole-stack.spec.md). Empty for a
 * branch that heads no stack.
 */
export function getWholeStackMenuItems(root: string, options: WholeStackMenuOptions): MenuItem[] {
  const stack = getStackBranch(root)
  const forks = stack !== undefined && stack.members.length > 0
  const items: MenuItem[] = []
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
  } else if (forks && (pullRequest === undefined || pullRequest.state === "closed") && getBranchesGitHubRepository() !== null) {
    const blocker = getStackBranchBlocker(root, stack)
    items.push({
      id: "create-stack-pull-request",
      label: "Create Pull Request of the Stack…",
      icon: GitPullRequestCreateIcon,
      disabled: blocker !== null,
      description:
        blocker ??
        `The pull request of ${formatWholeStack(root)} into ${getReadinessTarget()}: the commits of ${formatBranchCount(stack.members.length)}, which one Safe Push lands`,
      onSelect: () => openStackPullRequestDialog(root),
    })
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
