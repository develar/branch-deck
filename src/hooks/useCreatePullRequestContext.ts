import { getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { getReviewRequirementState, usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { CreatePullRequestContext } from "@/lib/pull-request-dialog"
import { toCreatePullRequestContext, toStackPullRequestContext } from "@/lib/pull-request-dialog"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { getBranch, getStackBranch, useBranchSync, useStackBranch } from "@/stores/branch-sync"
import { isPatronusAvailable, usePatronusAvailable } from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, getStackPullRequest, usePullRequest, usePullRequestAvailability, useStackPullRequest } from "@/stores/pull-requests"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"

/**
 * The context of the "Create pull request" dialog of the main window, re-rendering when a fact of it changes; it asks
 * for the readiness of the branch (the code owners) while the dialog is open
 */
export function useCreatePullRequestContext(branch: Branch): CreatePullRequestContext {
  const patronusAvailable = usePatronusAvailable()
  const nextStep = useBranchNextStep(branch.name)
  return toCreatePullRequestContext(branch, {
    repositoryPath: useSelectedProjectPath() ?? "",
    pullRequest: usePullRequest(branch.name),
    availability: usePullRequestAvailability(),
    patronusAvailable,
    review: usePatronusReviewRequirement(branch.name, patronusAvailable).state,
    withoutReview: nextStep === null ? null : getSafePushWithoutReviewItem(branch, nextStep),
  })
}

/**
 * The same context from the stores as they are, for the dialog of the review window (the main window sends it over the
 * link while the dialog is open, and asks for the readiness meanwhile)
 */
export function getCreatePullRequestContext(branch: Branch): CreatePullRequestContext {
  return toCreatePullRequestContext(branch, {
    repositoryPath: getSelectedProject()?.path ?? "",
    pullRequest: getPullRequest(branch.name),
    availability: getPullRequestAvailability(),
    patronusAvailable: isPatronusAvailable(),
    review: getReviewRequirementState(branch.name),
    withoutReview: getSafePushWithoutReviewItem(branch),
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
    pullRequest: useStackPullRequest(root),
    availability: usePullRequestAvailability(),
    branchOf: name => branches[name],
    patronusAvailable,
    review: usePatronusReviewRequirement(stackReadinessKey(root), patronusAvailable).state,
  })
}

/** The same context from the stores as they are, for a request of the review window */
export function getStackPullRequestContext(root: string): CreatePullRequestContext {
  const stack = getStackBranch(root)
  return toStackPullRequestContext(root, stack, {
    repositoryPath: getSelectedProject()?.path ?? "",
    pullRequest: getStackPullRequest(root),
    availability: getPullRequestAvailability(),
    branchOf: getBranch,
    patronusAvailable: isPatronusAvailable() && stack?.state === "Built",
    review: getReviewRequirementState(stackReadinessKey(root)),
  })
}
