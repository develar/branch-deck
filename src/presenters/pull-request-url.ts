// The pull request of a branch as the page offers it in words and links (spec/branch/pull-requests.spec.md): the base
// every pull request of a virtual branch goes into, the opener of GitHub's form of a new one ("Continue on GitHub"), the
// `gh` command that retargets one into a virtual branch, and the pull request action of a toast. Rust decides where a pull request
// goes (`domain_policy::pull_request`, `PullRequestView.base`) and whether a new one is offered (`create-pull-request`
// of the actions); the retarget before a push is a step of the plan of the push (`operations::push`, D3a).
import { openUrl } from "@tauri-apps/plugin-opener"
import type { PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { GitHubRepository } from "@/lib/pull-request"
import { openPullRequest } from "@/lib/pull-request"
import { baselineOnRemote } from "@/lib/stack"
import type { ToastActionProps } from "@/lib/toast-message"
import { getRowPart } from "@/model/store"
import { findAction } from "@/presenters/actions"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getOpenPullRequest } from "@/stores/branch-sync"
import { openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { getPatronusGitHubRepository } from "@/stores/patronus"
import { getGitHubRepository, getPullRequest } from "@/stores/pull-requests"

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
 * (`getPullRequestRetarget`); null if there is nothing to retarget. A push retargets it first (the `retarget` effect of
 * its plan): the command is for a retarget that failed, and for the branch menu before a push.
 */
export function getRetargetCommand(branch: Branch): string | null {
  const retarget = getPullRequestRetarget(branch.name)
  return retarget === null ? null : `gh pr edit ${retarget.pullRequest.number}${ghRepositoryOption()} --base ${retarget.base}`
}

/** A pull request a push retargeted first, as its toast names it */
export interface Retargeted {
  number: number
  /** The baseline as the remote names it */
  base: string
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
 * remote doesn't matter: the creation pushes it first (the `push` effect of its plan).
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
