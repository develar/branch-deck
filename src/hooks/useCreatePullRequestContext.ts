import { getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { getReviewRequirementState, usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { CreatePullRequestContext } from "@/lib/create-pull-request-context"
import { toCreatePullRequestContext } from "@/lib/create-pull-request-context"
import type { Branch } from "@/stores/branch-sync"
import { isPatronusAvailable, usePatronusAvailable } from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, usePullRequest, usePullRequestAvailability } from "@/stores/pull-requests"
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
