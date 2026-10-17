import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import type { CreatedPullRequest, PullRequest, PullRequestAvailability } from "@/lib/bindings"
import { formatCreatedToast } from "@/lib/create-pull-request"
import { formatNewPullRequestBlocker, getBranchesGitHubRepository, getPullRequestBase } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import type { CompareFields, GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl } from "@/lib/pull-request"
import type { ToastMessage } from "@/lib/toast-message"
import { recordPullRequestCreated } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { createPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/**
 * What the "Create pull request" dialog of a branch shows, from the main window, which owns the branches: the dialog
 * of the main window builds it from its stores, the one of the review window gets it over the link of the window
 * (`BranchReviewWindowState.createPullRequest`). Plain data: it crosses the window boundary as JSON.
 */
export interface CreatePullRequestContext {
  readonly repositoryPath: string
  readonly branchName: string
  /** The remote branch, as GitHub takes it */
  readonly head: string
  /** The base as GitHub takes it: the parent's pushed branch, or the baseline as the remote names it */
  readonly base: string
  /** The base for the title line: the parent's virtual name, never its ref */
  readonly baseName: string
  /** Why neither GitHub's form nor the creation can open the pull request now (`formatNewPullRequestBlocker`) */
  readonly newPullRequestBlocker: string | null
  /** The live pull request of the branch (open or merged), one that appeared meanwhile */
  readonly existing: PullRequest | null
  readonly availability: PullRequestAvailability | null
  /** The GitHub repository of the branches, for GitHub's form; null when unknown */
  readonly repository: GitHubRepository | null
  readonly patronusAvailable: boolean
  /** The code owners of the changes (`usePatronusReviewRequirement`), whatever a pull request would count */
  readonly review: ReviewRequirementState
  /** "Safe Push Without Review…" of row 7 (`getSafePushWithoutReviewItem`), null on another step */
  readonly withoutReview: { readonly label: string; readonly disabled: boolean; readonly description: string } | null
}

/** The facts of the main window a context is built of, read by hooks (the dialog) or from the stores (the link) */
export interface CreatePullRequestFacts {
  readonly repositoryPath: string
  readonly pullRequest: PullRequest | undefined
  readonly availability: PullRequestAvailability | null
  readonly patronusAvailable: boolean
  readonly review: ReviewRequirementState
  readonly withoutReview: MenuItem | null
}

export function toCreatePullRequestContext(branch: Branch, facts: CreatePullRequestFacts): CreatePullRequestContext {
  const head = getFullBranchName(branch.name)
  const base = getPullRequestBase(branch)
  const parent = branch.stack?.effectiveBase ?? null
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const withoutReview = facts.withoutReview
  return {
    repositoryPath: facts.repositoryPath,
    branchName: branch.name,
    head,
    base,
    baseName: parent !== null && base === getFullBranchName(parent) ? parent : base,
    newPullRequestBlocker: formatNewPullRequestBlocker(branch.name, existing, branch.remoteStatus?.exists === true),
    existing,
    availability: facts.availability,
    repository: getBranchesGitHubRepository(),
    patronusAvailable: facts.patronusAvailable,
    review: facts.review,
    withoutReview: withoutReview === null ? null : { label: withoutReview.label, disabled: withoutReview.disabled === true, description: withoutReview.description ?? "" },
  }
}

/** GitHub's form of the new pull request with `fields` filled in; null when it can't open one now */
export function getContextPullRequestUrl(context: CreatePullRequestContext, fields: CompareFields): string | null {
  return context.repository === null || context.newPullRequestBlocker !== null ? null : getCompareUrl(context.repository, context.base, context.head, fields)
}

/** What the dialog sends to create the pull request: the text, the reviewers, whether the user is its assignee */
export interface CreatePullRequestInput {
  readonly title: string
  readonly body: string
  readonly reviewers: readonly string[]
  readonly assignSelf: boolean
  /** For the toast: "assigned to you" */
  readonly viewerLogin: string | null
}

/**
 * Creates the pull request of the context through the GitHub API and records it in Activity: the toast to show, with
 * the pull request it opens. Main window only (the store of the pull requests, Activity). Throws what GitHub said.
 */
export async function submitPullRequest(
  context: CreatePullRequestContext,
  input: CreatePullRequestInput,
): Promise<{ toast: Omit<ToastMessage, "action">; pullRequest: PullRequest }> {
  const request = {
    branchName: context.branchName,
    head: context.head,
    base: context.base,
    title: input.title,
    body: input.body,
    reviewers: [...input.reviewers],
    assignSelf: input.assignSelf,
  }
  const result: CreatedPullRequest = await createPullRequest(request)
  const toast = formatCreatedToast(context.branchName, context.baseName, input.viewerLogin, result)
  if (result.created) {
    recordPullRequestCreated(toast.title, toast.description ?? "", result.pullRequest.url, toast.type === "warning")
  }
  return { toast, pullRequest: result.pullRequest }
}
