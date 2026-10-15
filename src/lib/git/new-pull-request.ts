import { openUrl } from "@tauri-apps/plugin-opener"
import { notifyError } from "@/lib/errors"
import { getPullRequestBase } from "@/lib/git/stacked-push-toast"
import type { GitHubRepository } from "@/lib/pull-request"
import { getCompareUrl, openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { getPatronusGitHubRepository } from "@/stores/patronus"
import { getGitHubRepository, getPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/**
 * The GitHub repository of the branches: the one `gh` lists pull requests of (`getGitHubRepository`), else the one
 * Patronus knows the branches are mirrored to (`branchdeck.githubRepository`, no `gh` needed for a link)
 */
function getBranchesGitHubRepository(): GitHubRepository | null {
  const listed = getGitHubRepository()
  if (listed !== null) {
    return listed
  }
  const [owner, name] = getPatronusGitHubRepository()?.split("/") ?? []
  return owner === undefined || name === undefined ? null : { owner, name }
}

/**
 * The GitHub form of a new pull request of the pushed branch into its base (`getPullRequestBase`: the parent's pushed
 * branch, or the baseline as the remote names it); null while the branch is not on the remote, when it has a pull
 * request, and while the GitHub repository is not known (`getBranchesGitHubRepository`)
 */
export function getNewPullRequestUrl(branch: Branch): string | null {
  const repository = getBranchesGitHubRepository()
  if (repository === null || branch.remoteStatus?.exists !== true || getPullRequest(branch.name) !== undefined) {
    return null
  }
  return getCompareUrl(repository, getPullRequestBase(branch), getFullBranchName(branch.name))
}

/** Opens the GitHub form of a new pull request in the browser; GitHub creates nothing until it is submitted */
export function openNewPullRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open GitHub", error))
}

/**
 * The action of the toast of a push: "Open Pull Request" for the pull request of the branch, "Create Pull Request"
 * without one; null when there is neither (not on GitHub)
 */
export function getPullRequestToastAction(branch: Branch): { children: string, onClick: () => void } | null {
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest !== undefined) {
    return { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
  }
  const url = getNewPullRequestUrl(branch)
  return url === null ? null : { children: "Create Pull Request", onClick: () => openNewPullRequest(url) }
}
