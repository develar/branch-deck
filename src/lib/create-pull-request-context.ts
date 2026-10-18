import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import type { CreatedPullRequest, PullRequest, PullRequestAvailability } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { PullRequestIncludes } from "@/lib/create-pull-request"
import { formatCreatedToast } from "@/lib/create-pull-request"
import { formatNewPullRequestBlocker, getBranchesGitHubRepository, getPullRequestBase, openNewPullRequest } from "@/lib/git/new-pull-request"
import { pushForAction } from "@/lib/git/push"
import type { MenuItem } from "@/lib/menu"
import type { CompareFields, GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl } from "@/lib/pull-request"
import { needsPush, stackAncestors, stackPushOrder } from "@/lib/stack"
import type { ToastMessage } from "@/lib/toast-message"
import { recordPullRequestCreated } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"
import { getPushHold } from "@/stores/patronus"
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
  /** The base: the baseline as the remote names it (`master`), for every branch, stacked or not */
  readonly base: string
  /** The branch, or an ancestor, has commits the remote lacks: Create and "Continue on GitHub" push them first */
  readonly pushNeeded: boolean
  /**
   * A stacked branch: the branches it is built on, root first, whose commits the pull request holds with its own, and
   * the commits of them all; null for a branch built on the baseline
   */
  readonly includes: PullRequestIncludes | null
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
  const pushNeeded = needsPush(branch) || stackPushOrder(branch.name, getBranch, { stopAtConfirm: true, isHeld: name => getPushHold(name) !== null }).length > 1
  const ancestors = stackAncestors(branch.name, getBranch)
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const withoutReview = facts.withoutReview
  return {
    repositoryPath: facts.repositoryPath,
    branchName: branch.name,
    head,
    base: getPullRequestBase(),
    pushNeeded,
    includes: ancestors.length === 0 ? null : { ancestors: ancestors.map(it => it.name), commitCount: ancestors.reduce((sum, it) => sum + it.commitCount, branch.commitCount) },
    newPullRequestBlocker: formatNewPullRequestBlocker(branch.name, existing),
    existing,
    availability: facts.availability,
    repository: getBranchesGitHubRepository(),
    patronusAvailable: facts.patronusAvailable,
    review: facts.review,
    withoutReview: withoutReview === null ? null : { label: withoutReview.label, disabled: withoutReview.disabled === true, description: withoutReview.description ?? "" },
  }
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

/** The push the creation starts with (`pushForAction`). Throws why nothing was pushed. */
async function pushForPullRequest(context: CreatePullRequestContext): Promise<void> {
  if (!context.pushNeeded) {
    return
  }
  const pushed = await pushForAction(context.branchName)
  if (!pushed.pushed) {
    throw new Error(pushed.reason)
  }
}

/**
 * Pushes what the pull request needs, then creates it through the GitHub API and records it in Activity: the toast to
 * show, with the pull request it opens. Right after the push GitHub may not have the branch yet (a Space mirror copies
 * it a moment later): the creation waits for it (`waitForMirror`). Main window only (the push, the store of the pull
 * requests, Activity). Throws why nothing was pushed, or what GitHub said.
 */
export async function submitPullRequest(
  context: CreatePullRequestContext,
  input: CreatePullRequestInput,
): Promise<{ toast: Omit<ToastMessage, "action">; pullRequest: PullRequest }> {
  await pushForPullRequest(context)
  const request = {
    branchName: context.branchName,
    head: context.head,
    base: context.base,
    title: input.title,
    body: input.body,
    reviewers: [...input.reviewers],
    assignSelf: input.assignSelf,
    waitForMirror: context.pushNeeded,
  }
  const result: CreatedPullRequest = await createPullRequest(request)
  const toast = formatCreatedToast(context.branchName, context.base, input.viewerLogin, result)
  if (result.created) {
    recordPullRequestCreated(toast.title, toast.description ?? "", result.pullRequest.url, toast.type === "warning")
  }
  return { toast, pullRequest: result.pullRequest }
}

/**
 * "Continue on GitHub": pushes what the pull request needs, waits until GitHub has the branch (a mirror copies it a
 * moment later; GitHub's form of a branch it doesn't have shows nothing), then opens GitHub's form with `fields`. Main
 * window only. Throws why nothing was pushed, or that GitHub still doesn't have the branch.
 */
export async function continueOnGitHub(context: CreatePullRequestContext, fields: CompareFields): Promise<void> {
  await pushForPullRequest(context)
  if (context.repository === null || context.newPullRequestBlocker !== null) {
    throw new Error(context.newPullRequestBlocker ?? "The GitHub repository of the branches is unknown")
  }
  if (context.pushNeeded) {
    const result = await commands.waitForGithubBranch({ repositoryPath: context.repositoryPath, branch: context.head })
    if (result.status === "error") {
      throw new Error(result.error.message)
    }
    if (!result.data) {
      throw new Error(`GitHub doesn't have ${context.head} yet: a mirror copies a pushed branch a moment later; try again`)
    }
  }
  openNewPullRequest(getCompareUrl(context.repository, context.base, context.head, fields))
}
