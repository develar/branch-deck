// The pull request of a branch as the page offers it in words and links (spec/branch/pull-requests.spec.md): the base
// every pull request of a virtual branch goes into, the opener of GitHub's form of a new one ("Continue on GitHub"), and
// the `gh` command that retargets one into a virtual branch. Rust decides where a pull request goes
// (`domain_policy::pull_request`, `PullRequestView.base`) and whether a new one is offered (`create-pull-request` of the
// actions); the retarget before a push is a step of the plan of the push (`operations::push`, D3a). What the stores have
// of a branch now (its pull request, the repository on GitHub) its caller reads (`src/lib/pull-request-now.ts`).
import { openUrl } from "@tauri-apps/plugin-opener"
import type { PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { baselineOnRemote } from "@/lib/stack"

/**
 * The base of every pull request of a virtual branch, stacked or not: the baseline `baselineBranch` of the view (the
 * caller's) as the remote names it (`master`). The pull request holds everything the Safe Push of the branch lands, the
 * commits of the branches it is built on too, so its approvals are the ones Patronus counts
 * ([Landing](docs/design/landing.md)).
 */
export function pullRequestBaseOf(baselineBranch: string | null): string {
  return baselineOnRemote(baselineBranch ?? "")
}

/** The open pull request of a branch whose base is a virtual branch, and the base it goes into instead */
export interface PullRequestRetarget {
  readonly pullRequest: PullRequest
  /** The virtual branch it goes into, by its short name: `auth` of `user/virtual/auth` */
  readonly from: string
  /** The baseline as the remote names it (`pullRequestBaseOf`) */
  readonly base: string
}

/**
 * The `gh` command that retargets the open pull request of a branch whose base is a virtual branch (`retarget`); ` --repo
 * owner/name` for branches mirrored to GitHub from Space (`ghRepository`, `RepositoryPatronus.githubRepository`), where
 * `gh` finds no GitHub repository among the remotes. A push retargets it first (the `retarget` effect of its plan): the
 * command is for a retarget that failed, and for the branch menu before a push.
 */
export function formatRetargetCommand(retarget: PullRequestRetarget, ghRepository: string | null): string {
  return `gh pr edit ${retarget.pullRequest.number}${ghRepository === null ? "" : ` --repo ${ghRepository}`} --base ${retarget.base}`
}

/** A pull request a push retargeted first, as its toast names it */
export interface Retargeted {
  number: number
  /** The baseline as the remote names it */
  base: string
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
