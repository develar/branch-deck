// The pull request of a branch as the stores have it now (spec/branch/pull-requests.spec.md), for what is built on
// demand: a menu on open, a toast, a store selector. It reads the open pull request of the view (`openPullRequests`,
// `PullRequestView.base`), the pull requests GitHub listed, the actions Rust offers (`create-pull-request`) and the
// repository Patronus mirrors the branches to; the presenters word what it read (`src/presenters/pull-request-url.ts`,
// `src/presenters/push-button.ts`) and read no store themselves.
import type { GitHubRepository } from "@/lib/pull-request"
import { openPullRequest } from "@/lib/pull-request"
import type { ToastActionProps } from "@/lib/toast-message"
import { getRowPart } from "@/model/store"
import { findAction } from "@/presenters/actions"
import type { PullRequestRetarget } from "@/presenters/pull-request-url"
import { CREATE_PULL_REQUEST_LABEL, formatRetargetCommand } from "@/presenters/pull-request-url"
import type { PushRetarget } from "@/presenters/push-button"
import type { Branch } from "@/stores/branch-sync"
import { getOpenPullRequest } from "@/stores/branch-sync"
import { openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { getPatronusGitHubRepository } from "@/stores/patronus"
import { getGitHubRepository, getPullRequest } from "@/stores/pull-requests"

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

/** The `gh` command that retargets the open pull request of a branch into a virtual branch (`formatRetargetCommand`); null if there is nothing to retarget */
export function getRetargetCommand(branch: Branch): string | null {
  const retarget = getPullRequestRetarget(branch.name)
  return retarget === null ? null : formatRetargetCommand(retarget, getPatronusGitHubRepository())
}

/** The open pull requests into a virtual branch that a push of `branchNames` retargets first (the `retarget` effects of its plan) */
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
 * The pull request action of a branch for a toast: "Create Pull Request…" while Rust offers a new one
 * (`create-pull-request` of its actions, `domain_policy::actions`: no open or merged pull request, the GitHub repository
 * known; the creation pushes it first), else "Open Pull Request" for its pull request; null when there is neither
 */
export function getPullRequestToastAction(branch: Branch): ToastActionProps | null {
  if (findAction(getRowPart(branch.name, "actions"), "create-pull-request") !== undefined) {
    return { children: CREATE_PULL_REQUEST_LABEL, onClick: () => openCreatePullRequestDialog(branch.name) }
  }
  const pullRequest = getPullRequest(branch.name)
  return pullRequest === undefined ? null : { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
}
