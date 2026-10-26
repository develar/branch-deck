import { openUrl } from "@tauri-apps/plugin-opener"
import type { PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { CompareFields, GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl, openPullRequest } from "@/lib/pull-request"
import { baselineOnRemote } from "@/lib/stack"
import type { ToastActionProps } from "@/lib/toast-message"
import { getRowPart } from "@/model/store"
import { findAction } from "@/presenters/actions"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getOpenPullRequest } from "@/stores/branch-sync"
import { openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { getPatronusGitHubRepository } from "@/stores/patronus"
import { getGitHubRepository, getPullRequest, retargetPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/**
 * The base of every pull request of a virtual branch, stacked or not: the baseline as the remote names it (`master`).
 * The pull request holds everything the Safe Push of the branch lands, the commits of the branches it is built on too,
 * so its approvals are the ones Patronus counts ([Landing](docs/design/landing.md)).
 */
export function getPullRequestBase(): string {
  return baselineOnRemote(branchSyncStore.getState().baselineBranch ?? "")
}

/**
 * ` --repo owner/name` for branches mirrored to GitHub from Space (`RepositoryPatronus.githubRepository`): `gh` finds
 * no GitHub repository among the remotes; empty when `origin` is on GitHub
 */
function ghRepositoryOption(): string {
  const repository = getPatronusGitHubRepository()
  return repository === null ? "" : ` --repo ${repository}`
}

/** The open pull request of a branch whose base is a virtual branch, and the base it goes into instead */
export interface PullRequestRetarget {
  readonly pullRequest: PullRequest
  /** The virtual branch it goes into, by its short name: `auth` of `user/virtual/auth` */
  readonly from: string
  /** The baseline as the remote names it (`getPullRequestBase`) */
  readonly base: string
}

/**
 * The open pull request of a branch whose base is a virtual branch, e.g. its parent's or the one of a parent that landed,
 * with the base it goes into instead, as Rust reads it (`PullRequestView.base`, `pull_request::base_of`); null if there
 * is nothing to retarget. A pull request into a release branch is left as it is: that base may be meant.
 */
export function getPullRequestRetarget(branchName: string): PullRequestRetarget | null {
  const pullRequest = getPullRequest(branchName)
  const base = getOpenPullRequest(branchName)?.base
  if (pullRequest === undefined || base?.kind !== "virtual") {
    return null
  }
  return { pullRequest, from: base.from, base: base.retarget }
}

/**
 * The `gh` command that retargets the open pull request of a branch whose base is a virtual branch
 * (`getPullRequestRetarget`); null if there is nothing to retarget. A push retargets it first (`retargetBeforePush`):
 * the command is for a retarget that failed, and for the branch menu before a push.
 */
export function getRetargetCommand(branch: Branch): string | null {
  const retarget = getPullRequestRetarget(branch.name)
  return retarget === null ? null : `gh pr edit ${retarget.pullRequest.number}${ghRepositoryOption()} --base ${retarget.base}`
}

/** A pull request the push of a branch retargets first (`getPushRetargets`), as the push description says it */
export interface PushRetarget {
  readonly branchName: string
  readonly number: number
  /** The baseline as the remote names it */
  readonly base: string
  /** Someone approved it: GitHub may dismiss the approvals of a pull request whose base changes */
  readonly approved: boolean
}

/** The open pull requests into a virtual branch that a push of `branchNames` retargets first (`retargetBeforePush`) */
export function getPushRetargets(branchNames: readonly string[]): PushRetarget[] {
  return branchNames.flatMap(branchName => {
    const retarget = getPullRequestRetarget(branchName)
    if (retarget === null) {
      return []
    }
    const { number, reviewers } = retarget.pullRequest
    return [{ branchName, number, base: retarget.base, approved: reviewers.some(reviewer => reviewer.state === "approved") }]
  })
}

/**
 * What the description of a push says before it retargets pull requests (`getPushRetargets`): "Retargets #8 to master:
 * GitHub may dismiss its approvals." for one with approvals, a line per pull request, each named by its branch when the
 * push sends several (`ofBranches`); null without a retarget
 */
export function describePushRetargets(retargets: readonly PushRetarget[], ofBranches: boolean): { text: string; warning: boolean } | null {
  if (retargets.length === 0) {
    return null
  }
  const lines = retargets.map(
    retarget =>
      `Retargets #${retarget.number}${ofBranches ? ` of ${retarget.branchName}` : ""} to ${retarget.base}${retarget.approved ? ": GitHub may dismiss its approvals" : ""}.`,
  )
  return { text: lines.join("\n"), warning: retargets.some(retarget => retarget.approved) }
}

/** A pull request a push retargeted first */
export interface Retargeted {
  number: number
  /** The baseline as the remote names it */
  base: string
}

/**
 * Retargets the open pull request of a branch whose base is a virtual branch to the baseline before the branch is
 * pushed (`getPullRequestRetarget`), as the engine runs it (`retargetPullRequest`, Activity records it): the pull request
 * then shows what the Safe Push of the branch lands, and its approvals count. Null when there was nothing to retarget,
 * or the retarget failed (an error toast said why; the push still runs, its toast offers the command).
 */
export async function retargetBeforePush(branch: Branch): Promise<Retargeted | null> {
  const retarget = getPullRequestRetarget(branch.name)
  if (retarget === null) {
    return null
  }
  const { pullRequest, base } = retarget
  try {
    await retargetPullRequest(branch.name)
  } catch (error) {
    notifyError(`Cannot retarget pull request #${pullRequest.number}`, error)
    return null
  }
  return { number: pullRequest.number, base }
}

/**
 * The GitHub repository of the branches: the one GitHub lists pull requests of (`getGitHubRepository`), else the one
 * Patronus knows the branches are mirrored to (Space settings, no token needed for a link)
 */
export function getBranchesGitHubRepository(): GitHubRepository | null {
  const listed = getGitHubRepository()
  if (listed !== null) {
    return listed
  }
  const [owner, name] = getPatronusGitHubRepository()?.split("/") ?? []
  return owner === undefined || name === undefined ? null : { owner, name }
}

/**
 * Rust offers a new pull request of the branch (`create-pull-request` of its actions, `domain_policy::actions`): it has
 * no open or merged pull request (a closed one doesn't count), and the GitHub repository is known. Whether it is on the
 * remote doesn't matter: the creation pushes it first (`pushForAction`).
 */
function offersPullRequest(branchName: string): boolean {
  return findAction(getRowPart(branchName, "actions"), "create-pull-request") !== undefined
}

/**
 * Why the branch can't get a new pull request now, null when it can: its live pull request (open or merged, as the
 * dialog subscribes to it). "Continue on GitHub" and Create are disabled with it.
 */
export function formatNewPullRequestBlocker(branchName: string, live: PullRequest | null): string | null {
  if (live === null) {
    return null
  }
  return live.state === "merged" ? `Pull request #${live.number} of ${branchName} is merged` : `${branchName} already has pull request #${live.number}`
}

/**
 * The GitHub form of a new pull request of the pushed branch into the baseline (`getPullRequestBase`), with `fields`
 * filled in: "Continue on GitHub" of the dialog; null when the branch can't get one (`offersPullRequest`)
 */
export function getNewPullRequestUrl(branch: Branch, fields?: CompareFields): string | null {
  const repository = getBranchesGitHubRepository()
  if (repository === null || !offersPullRequest(branch.name)) {
    return null
  }
  return getCompareUrl(repository, getPullRequestBase(), getFullBranchName(branch.name), fields)
}

/** Opens the GitHub form of a new pull request in the browser; GitHub creates nothing until it is submitted */
export function openNewPullRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open GitHub", error))
}

/** The label of the action that opens the "Create pull request" dialog: it asks for more, hence the ellipsis */
export const CREATE_PULL_REQUEST_LABEL = "Create Pull Request…"

/**
 * The pull request action of a branch for a toast: "Open Pull Request" for its pull request, "Create Pull Request…"
 * without one or with only a closed one; null when there is neither (not on GitHub)
 */
export function getPullRequestToastAction(branch: Branch): ToastActionProps | null {
  if (offersPullRequest(branch.name)) {
    return { children: CREATE_PULL_REQUEST_LABEL, onClick: () => openCreatePullRequestDialog(branch.name) }
  }
  const pullRequest = getPullRequest(branch.name)
  return pullRequest === undefined ? null : { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
}
