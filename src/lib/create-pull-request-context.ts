import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import type { CreatedPullRequest, PullRequest, PullRequestAvailability } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatCreatedToast } from "@/lib/create-pull-request"
import { formatNewPullRequestBlocker, getBranchesGitHubRepository, getPullRequestBase, openNewPullRequest } from "@/lib/git/new-pull-request"
import { pushForAction } from "@/lib/git/push"
import type { MenuItem } from "@/lib/menu"
import type { CompareFields, GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl } from "@/lib/pull-request"
import { needsPush, stackPushOrder } from "@/lib/stack"
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
  /** The base as GitHub takes it: the parent's pushed branch, or the baseline as the remote names it */
  readonly base: string
  /** The branch, or an ancestor, has commits the remote lacks: Create and "Continue on GitHub" push them first */
  readonly pushNeeded: boolean
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
  const pushNeeded = needsPush(branch) || stackPushOrder(branch.name, getBranch, { stopAtConfirm: true, isHeld: name => getPushHold(name) !== null }).length > 1
  const parent = branch.stack?.effectiveBase ?? null
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const withoutReview = facts.withoutReview
  return {
    repositoryPath: facts.repositoryPath,
    branchName: branch.name,
    head,
    base,
    pushNeeded,
    baseName: parent !== null && base === getFullBranchName(parent) ? parent : base,
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

/**
 * The push the creation starts with (`pushForAction`), then the base as it is after it (`getPullRequestBase`). Throws
 * why nothing was pushed.
 */
async function pushForPullRequest(context: CreatePullRequestContext): Promise<string> {
  if (!context.pushNeeded) {
    return context.base
  }
  const pushed = await pushForAction(context.branchName)
  if (!pushed.pushed) {
    throw new Error(pushed.reason)
  }
  const branch = getBranch(context.branchName)
  return branch === undefined ? context.base : getPullRequestBase(branch)
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
  const base = await pushForPullRequest(context)
  const request = {
    branchName: context.branchName,
    head: context.head,
    base,
    title: input.title,
    body: input.body,
    reviewers: [...input.reviewers],
    assignSelf: input.assignSelf,
    waitForMirror: context.pushNeeded,
  }
  const result: CreatedPullRequest = await createPullRequest(request)
  const toast = formatCreatedToast(context.branchName, context.baseName, input.viewerLogin, result)
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
  const base = await pushForPullRequest(context)
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
  openNewPullRequest(getCompareUrl(context.repository, base, context.head, fields))
}
