// What the "Create pull request" dialog shows of a branch or of a whole stack (`CreatePullRequestContext`), built from
// the stores of the main window (`useCreatePullRequestContext`) and sent to the dialog of the review window over the
// link of the window; what it sends to create the pull request (`CreatePullRequestInput`). The creation itself is the
// intent of the engine (`pull-request-run.ts`).
import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import type { PullRequest, PullRequestAvailability, StackMemberSubjects } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import type { PullRequestIncludes } from "@/lib/create-pull-request"
import { formatPullRequestIncludes } from "@/lib/create-pull-request"
import { formatNewPullRequestBlocker, getBranchesGitHubRepository, getPullRequestBase } from "@/lib/git/new-pull-request"
import type { MenuItem } from "@/lib/menu"
import type { GitHubRepository } from "@/lib/pull-request"
import { joinNames } from "@/lib/text"
import { formatWholeStackStart } from "@/lib/whole-stack"
import { formatReasonOrNull } from "@/presenters/reasons"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { getBranch, getStackAncestors, getStackNotBuilt } from "@/stores/branch-sync"
import { getEffectiveBranchPrefix, getFullBranchName, getFullStackBranchName } from "@/stores/repository"

/**
 * What the "Create pull request" dialog of a branch shows, from the main window, which owns the branches: the dialog
 * of the main window builds it from its stores, the one of the review window gets it over the link of the window
 * (`BranchReviewWindowState.createPullRequest`). Plain data: it crosses the window boundary as JSON.
 */
export interface CreatePullRequestContext {
  /**
   * The repository the dialog was opened in: the push, the pull request and Activity go there, also once the main window
   * shows another repository (whose branches of the same name are other branches: the push refuses). With the prefix
   * the page showed then, which keys the stacks it remembers landed (`forgetLandedStackRoot`); Rust resolves the
   * prefix of every command itself
   */
  readonly repositoryPath: string
  readonly branchPrefix: string
  /** The branch, or the root of the whole stack (`wholeStack`) */
  readonly branchName: string
  /** How the dialog names what the pull request is of: the branch, or "Whole air stack" */
  readonly label: string
  /**
   * The pull request of the whole stack: its stack branch `{prefix}/stack/<root>` holds the commits of every branch of
   * the stack (`members`, as their rows show them); null for a branch
   */
  readonly wholeStack: { readonly members: readonly StackMemberSubjects[] } | null
  /** The remote branch, as GitHub takes it */
  readonly head: string
  /** The base: the baseline as the remote names it (`master`), for every branch, stacked or not */
  readonly base: string
  /**
   * The remote branches of the branches it is built on, root first (`user/virtual/auth`): the text of the pull request
   * names them and leaves their commits out of its own; empty for a branch built on the baseline and the whole stack
   */
  readonly stackAncestors: readonly string[]
  /**
   * A stacked branch: the branches it is built on, root first, whose commits the pull request holds with its own, and
   * the commits of them all; null for a branch built on the baseline
   */
  readonly includes: PullRequestIncludes | null
  /** What the pull request holds besides the branch's own commits, under the title: `formatPullRequestIncludes` */
  readonly includesText: string | null
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
  const ancestors = getStackAncestors(branch.name).flatMap(name => getBranch(name) ?? [])
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const withoutReview = facts.withoutReview
  const includes = ancestors.length === 0 ? null : { ancestors: ancestors.map(it => it.name), commitCount: ancestors.reduce((sum, it) => sum + it.commitCount, branch.commitCount) }
  return {
    repositoryPath: facts.repositoryPath,
    branchPrefix: getEffectiveBranchPrefix(),
    branchName: branch.name,
    label: branch.name,
    wholeStack: null,
    head,
    base: getPullRequestBase(),
    stackAncestors: ancestors.map(it => getFullBranchName(it.name)),
    includes,
    includesText: includes === null ? null : formatPullRequestIncludes(branch.name, includes),
    newPullRequestBlocker: formatNewPullRequestBlocker(branch.name, existing),
    existing,
    availability: facts.availability,
    repository: getBranchesGitHubRepository(),
    patronusAvailable: facts.patronusAvailable,
    review: facts.review,
    withoutReview: withoutReview === null ? null : { label: withoutReview.label, disabled: withoutReview.disabled === true, description: withoutReview.description ?? "" },
  }
}

/** The facts of the main window the context of the whole stack's pull request is built of */
export interface StackPullRequestFacts {
  readonly repositoryPath: string
  readonly pullRequest: PullRequest | undefined
  readonly availability: PullRequestAvailability | null
  readonly branchOf: (name: string) => Branch | undefined
  /** Patronus is available and the sync built the stack branch: Patronus is asked about it */
  readonly patronusAvailable: boolean
  /** The code owners of the whole stack, its readiness (`stack:<root>`, `stackReadinessKey`) */
  readonly review: ReviewRequirementState
}

/**
 * The context of the "Create pull request" dialog of the whole stack of `root`: the pull request of its stack branch
 * into the baseline, whose approvals the one robot of the whole stack counts. It holds every commit of the stack
 * ("Holds the commits of every branch of the stack: air, auth and 5 more: 7 branches · 14 commits"); Create and
 * "Continue on GitHub" push the stack branch first (the plan of `CreatePullRequest`), only one the sync built
 * (`getStackNotBuilt`).
 * The code owners it suggests are those of the whole stack, which its Safe Push needs: the readiness of its stack branch.
 */
export function toStackPullRequestContext(root: string, stack: StackBranch | undefined, facts: StackPullRequestFacts): CreatePullRequestContext {
  const members = (stack?.members ?? []).flatMap(name => {
    const branch = facts.branchOf(name)
    return branch === undefined ? [] : [{ branch, parent: name === root ? null : (branch.stack?.effectiveBase ?? null) }]
  })
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const label = formatWholeStackStart(root)
  const commitCount = stack?.commitCount ?? 0
  const names = members.map(it => it.branch.name)
  return {
    repositoryPath: facts.repositoryPath,
    branchPrefix: getEffectiveBranchPrefix(),
    branchName: root,
    label,
    wholeStack: {
      members: members.map(({ branch, parent }) => ({
        name: branch.name,
        parent,
        // oldest first, as the stack branch has them
        subjects: [...branch.commits].reverse().map(commit => commit.strippedSubject),
      })),
    },
    head: getFullStackBranchName(root),
    base: getPullRequestBase(),
    stackAncestors: [],
    includes: null,
    includesText: names.length === 0 ? null : `Holds the commits of every branch of the stack: ${joinNames(names)}: ${formatBranchesAndCommits(names.length, commitCount)}`,
    newPullRequestBlocker: formatReasonOrNull(getStackNotBuilt(root)) ?? (existing === null ? null : formatNewPullRequestBlocker(label, existing)),
    existing,
    availability: facts.availability,
    repository: getBranchesGitHubRepository(),
    patronusAvailable: facts.patronusAvailable,
    review: facts.review,
    withoutReview: null,
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
