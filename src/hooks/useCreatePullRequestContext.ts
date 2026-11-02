import { getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { CreatePullRequestContext } from "@/lib/pull-request-dialog"
import { toCreatePullRequestContext, toStackPullRequestContext } from "@/lib/pull-request-dialog"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useStackBranch } from "@/stores/branch-sync"
import { usePatronusAvailable } from "@/stores/patronus"
import { usePullRequest, usePullRequestAvailability, useStackPullRequest } from "@/stores/pull-requests"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * The context of the "Create pull request" dialog of a window, re-rendering when a fact of it changes; it asks
 * for the readiness of the branch (the code owners) while the dialog is open
 */
export function useCreatePullRequestContext(branch: Branch): CreatePullRequestContext {
  const patronusAvailable = usePatronusAvailable()
  const nextStep = useBranchNextStep(branch.name)
  return toCreatePullRequestContext(branch, {
    repositoryPath: useSelectedProjectPath() ?? "",
    baselineBranch: useBranchSync(state => state.baselineBranch),
    pullRequest: usePullRequest(branch.name),
    landed: (nextStep?.landed ?? null) !== null,
    availability: usePullRequestAvailability(),
    patronusAvailable,
    review: usePatronusReviewRequirement(branch.name, patronusAvailable).state,
    withoutReview: nextStep === null ? null : getSafePushWithoutReviewItem(branch, nextStep),
    layer: useBranchSync(state => state.githubLayers[branch.name] ?? null),
  })
}

/**
 * The context of the dialog of the whole stack of `root` (`toStackPullRequestContext`), re-rendering when its stack
 * branch, its pull request or a branch of the stack changes
 */
export function useStackPullRequestContext(root: string): CreatePullRequestContext {
  const stack = useStackBranch(root)
  // the branches of the stack as one record, so the dialog renders again when one of them changes
  const branches = useBranchSync(state => state.branches)
  const available = usePatronusAvailable()
  // Patronus is asked about the stack branch the sync built, and only while the dialog is open
  const patronusAvailable = available && stack?.state === "Built"
  return toStackPullRequestContext(root, stack, {
    repositoryPath: useSelectedProjectPath() ?? "",
    baselineBranch: useBranchSync(state => state.baselineBranch),
    pullRequest: useStackPullRequest(root),
    availability: usePullRequestAvailability(),
    branchOf: name => branches[name],
    patronusAvailable,
    review: usePatronusReviewRequirement(stackReadinessKey(root), patronusAvailable).state,
  })
}
