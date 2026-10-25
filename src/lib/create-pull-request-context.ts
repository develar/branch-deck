import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import type { CreatedPullRequest, PullRequest, PullRequestAvailability, StackMemberSubjects } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import type { PullRequestIncludes } from "@/lib/create-pull-request"
import { formatCreatedToast, formatPullRequestIncludes } from "@/lib/create-pull-request"
import { commandFailure, getErrorDetails } from "@/lib/errors"
import { formatNewPullRequestBlocker, getBranchesGitHubRepository, getPullRequestBase, openNewPullRequest } from "@/lib/git/new-pull-request"
import { pushForAction } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import type { MenuItem } from "@/lib/menu"
import type { CompareFields, GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl } from "@/lib/pull-request"
import type { RunningMove } from "@/lib/running-move"
import { stackAncestors, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import type { ToastMessage } from "@/lib/toast-message"
import { formatWholeStackStart, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import { recordPullRequestCreated } from "@/stores/activity"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"
import { forgetLandedStackRoot } from "@/stores/landed-stacks"
import { branchSubject, runOperation } from "@/stores/operations"
import { getPushHold } from "@/stores/patronus"
import { createPullRequest } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getFullBranchName, getFullStackBranchName, getRepositorySwitchReason } from "@/stores/repository"

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
  /** The branch, or an ancestor, has commits the remote lacks: Create and "Continue on GitHub" push them first */
  readonly pushNeeded: boolean
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
  // the remote lacks work of the branch (Rust's action of its remote status), or of an ancestor the push takes along
  const pushNeeded =
    (branch.remoteStatus?.action.kind ?? "upToDate") !== "upToDate" ||
    stackPushOrder(branch.name, getBranch, { stopAtConfirm: true, isHeld: name => getPushHold(name) !== null }).length > 1
  const ancestors = stackAncestors(branch.name, getBranch)
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
    pushNeeded,
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
 * "Continue on GitHub" push the stack branch first, and only a stack branch the sync built (`getStackBranchBlocker`).
 * The code owners it suggests are those of the whole stack, which its Safe Push needs: the readiness of its stack branch.
 */
export function toStackPullRequestContext(root: string, stack: StackBranch | undefined, facts: StackPullRequestFacts): CreatePullRequestContext {
  const members = (stack?.members ?? []).flatMap(name => {
    const branch = facts.branchOf(name)
    return branch === undefined ? [] : [{ branch, parent: name === root ? null : (branch.stack?.effectiveBase ?? null) }]
  })
  const existing = facts.pullRequest !== undefined && facts.pullRequest.state !== "closed" ? facts.pullRequest : null
  const label = formatWholeStackStart(root)
  const remote = stack?.remoteStatus ?? null
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
    pushNeeded: remote === null || !remote.exists || remote.remoteTip !== stack?.tip,
    includes: null,
    includesText: names.length === 0 ? null : `Holds the commits of every branch of the stack: ${joinNames(names)}: ${formatBranchesAndCommits(names.length, commitCount)}`,
    newPullRequestBlocker: getStackBranchBlocker(root, stack) ?? (existing === null ? null : formatNewPullRequestBlocker(label, existing)),
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

/**
 * The push the creation starts with (`pushForAction`; the stack branch of the whole stack, `pushWholeStack`). Throws why
 * nothing was pushed.
 */
async function pushForPullRequest(context: CreatePullRequestContext): Promise<void> {
  if (!context.pushNeeded) {
    return
  }
  if (context.wholeStack !== null) {
    // the push reads the stack branch of the table, which is of the selected repository
    const switched = getRepositorySwitchReason(context.repositoryPath)
    if (switched !== null) {
      throw new Error(switched)
    }
    let reason: string | null = null
    const pushed = await pushWholeStack(context.branchName, {
      report: (_title, error) => {
        reason ??= getErrorDetails(error)
      },
    })
    if (!pushed) {
      throw new Error(reason ?? `Cannot push the stack branch of ${context.branchName}`)
    }
    return
  }
  const pushed = await pushForAction(context.branchName, { repositoryPath: context.repositoryPath })
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
export function submitPullRequest(context: CreatePullRequestContext, input: CreatePullRequestInput): Promise<{ toast: Omit<ToastMessage, "action">; pullRequest: PullRequest }> {
  const label = `Creating the pull request of ${context.label}`
  // the move of the row: the reviewers or Patronus answer next, from the click on (the push and the mirror included)
  const move = { key: context.wholeStack === null ? context.branchName : stackReadinessKey(context.branchName), lands: false, label }
  return runPullRequestOperation(context, "create-pull-request", label, () => createPullRequestOf(context, input), move)
}

/** The ids of the operations of the pull request dialog: its creation and "Continue on GitHub", which push first */
export const PULL_REQUEST_DIALOG_ACTIONS: readonly string[] = ["create-pull-request", "continue-on-github"]

/** The subject of the pull request of a branch, or of the whole stack of a root (`wholeStack`) */
export function pullRequestSubject(branchName: string, wholeStack: boolean): string {
  return wholeStack ? `stack:${branchName}` : branchSubject(branchName)
}

/**
 * Runs an action of the dialog as one operation of its pull request (`runOperation`): a second submit joins it; one that
 * must wait (another remote action of the branch) throws why. `move`: the action is the move of the row (the creation;
 * "Continue on GitHub" is not, the user creates it on GitHub)
 */
async function runPullRequestOperation<T>(context: CreatePullRequestContext, id: string, label: string, run: () => Promise<T>, move?: RunningMove): Promise<T> {
  let blocked: string | null = null
  const result = await runOperation(
    {
      id,
      subject: pullRequestSubject(context.branchName, context.wholeStack !== null),
      objects: [context.branchName],
      kind: "remote",
      label,
      repositoryPath: context.repositoryPath,
      ...(move === undefined ? null : { move }),
    },
    run,
    reason => (blocked = reason),
  )
  if (blocked !== null) {
    throw new Error(blocked)
  }
  return result as T
}

async function createPullRequestOf(context: CreatePullRequestContext, input: CreatePullRequestInput): Promise<{ toast: Omit<ToastMessage, "action">; pullRequest: PullRequest }> {
  await pushForPullRequest(context)
  const request = {
    repositoryPath: context.repositoryPath,
    branchName: context.branchName,
    wholeStack: context.wholeStack !== null,
    head: context.head,
    base: context.base,
    title: input.title,
    body: input.body,
    reviewers: [...input.reviewers],
    assignSelf: input.assignSelf,
    waitForMirror: context.pushNeeded,
  }
  const result: CreatedPullRequest = await createPullRequest(request)
  if (context.wholeStack !== null) {
    // a new pull request of the stack matches the stack as it is
    forgetLandedStackRoot(context.branchName, context.repositoryPath, context.branchPrefix)
  }
  const toast = formatCreatedToast(context.label, context.base, input.viewerLogin, result)
  if (result.created) {
    recordPullRequestCreated(toast.title, toast.description ?? "", result.pullRequest.url, toast.type === "warning", context.repositoryPath)
  }
  return { toast, pullRequest: result.pullRequest }
}

/**
 * "Continue on GitHub": pushes what the pull request needs, waits until GitHub has the branch (a mirror copies it a
 * moment later; GitHub's form of a branch it doesn't have shows nothing), then opens GitHub's form with `fields`. Main
 * window only. Throws why nothing was pushed, or that GitHub still doesn't have the branch.
 */
export function continueOnGitHub(context: CreatePullRequestContext, fields: CompareFields): Promise<void> {
  return runPullRequestOperation(context, "continue-on-github", `Opening the pull request of ${context.label} on GitHub`, () => continueOnGitHubOf(context, fields))
}

async function continueOnGitHubOf(context: CreatePullRequestContext, fields: CompareFields): Promise<void> {
  await pushForPullRequest(context)
  if (context.repository === null || context.newPullRequestBlocker !== null) {
    throw new Error(context.newPullRequestBlocker ?? "The GitHub repository of the branches is unknown")
  }
  if (context.pushNeeded) {
    const result = await commands.waitForGithubBranch({ repositoryPath: context.repositoryPath, branch: context.head })
    if (result.status === "error") {
      throw commandFailure(result.error)
    }
    if (!result.data) {
      throw new Error(`GitHub doesn't have ${context.head} yet: a mirror copies a pushed branch a moment later; try again`)
    }
  }
  openNewPullRequest(getCompareUrl(context.repository, context.base, context.head, fields))
}
