import { openUrl } from "@tauri-apps/plugin-opener"
import type { PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl, openPullRequest } from "@/lib/pull-request"
import { pullRequestBaseOf } from "@/lib/stack"
import type { ToastActionProps } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { getPatronusGitHubRepository } from "@/stores/patronus"
import { getGitHubRepository, getPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/**
 * The pull request base of a branch, as `gh` and GitHub take it: the parent's pushed branch, or the baseline as the
 * remote names it, also while the parent isn't on the remote (`pullRequestBaseOf`)
 */
export function getPullRequestBase(branch: Branch): string {
  return pullRequestBaseOf(branch, branchSyncStore.getState().baselineBranch ?? "", getFullBranchName, getBranch)
}

/**
 * ` --repo owner/name` for branches mirrored to GitHub from Space (`RepositoryPatronus.githubRepository`): `gh` finds
 * no GitHub repository among the remotes; empty when `origin` is on GitHub
 */
function ghRepositoryOption(): string {
  const repository = getPatronusGitHubRepository()
  return repository === null ? "" : ` --repo ${repository}`
}

/**
 * The `gh` command that retargets the open pull request of a branch whose base is not the one it should have, e.g.
 * after its parent was integrated or pushed, or the branch was restacked; null if there is nothing to retarget. The app
 * writes nothing to GitHub but the `/patronus` comment: the user runs it.
 */
export function getRetargetCommand(branch: Branch): string | null {
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest === undefined || pullRequest.state !== "open" || !pullRequest.baseRefName) {
    return null
  }
  const base = getPullRequestBase(branch)
  return pullRequest.baseRefName === base ? null : `gh pr edit ${pullRequest.number}${ghRepositoryOption()} --base ${base}`
}

/**
 * The GitHub repository of the branches: the one `gh` lists pull requests of (`getGitHubRepository`), else the one
 * Patronus knows the branches are mirrored to (Space settings, no `gh` needed for a link)
 */
function getBranchesGitHubRepository(): GitHubRepository | null {
  const listed = getGitHubRepository()
  if (listed !== null) {
    return listed
  }
  const [owner, name] = getPatronusGitHubRepository()?.split("/") ?? []
  return owner === undefined || name === undefined ? null : { owner, name }
}

/** The pull request of a branch that keeps it from getting a new one: open (a draft too) or merged, not closed */
function getLivePullRequest(branchName: string): PullRequest | undefined {
  const pullRequest = getPullRequest(branchName)
  return pullRequest?.state === "closed" ? undefined : pullRequest
}

/**
 * The GitHub form of a new pull request of the pushed branch into its base (`getPullRequestBase`); null while the
 * branch is not on the remote, when it has an open or a merged pull request (a closed one doesn't count), and while
 * the GitHub repository is not known (`getBranchesGitHubRepository`)
 */
export function getNewPullRequestUrl(branch: Branch): string | null {
  const repository = getBranchesGitHubRepository()
  if (repository === null || branch.remoteStatus?.exists !== true || getLivePullRequest(branch.name) !== undefined) {
    return null
  }
  return getCompareUrl(repository, getPullRequestBase(branch), getFullBranchName(branch.name))
}

/** Opens the GitHub form of a new pull request in the browser; GitHub creates nothing until it is submitted */
export function openNewPullRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open GitHub", error))
}

/**
 * The pull request action of a branch for a toast: "Open Pull Request" for its pull request, "Create Pull Request"
 * without one or with only a closed one; null when there is neither (not on GitHub)
 */
export function getPullRequestToastAction(branch: Branch): ToastActionProps | null {
  const url = getNewPullRequestUrl(branch)
  if (url !== null) {
    return { children: "Create Pull Request", onClick: () => openNewPullRequest(url) }
  }
  const pullRequest = getPullRequest(branch.name)
  return pullRequest === undefined ? null : { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
}
