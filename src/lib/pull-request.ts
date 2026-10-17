import { openUrl } from "@tauri-apps/plugin-opener"
import type { ChecksSummary, GitHubAuthStatus, PullRequest, PullRequestAvailability, Reviewer, ReviewerState, TokenSource } from "@/lib/bindings"
import { formatCheckCounts } from "@/lib/check-counts"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { ReviewCommitState } from "@/lib/review-commit"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import type { UIColor } from "@/lib/ui-types"
import type { RemoteStatus } from "@/stores/branch-sync"
import { setSettingsDialogOpen } from "@/stores/settings-dialog"

/** Kind of the chip icon, as GitHub shows a pull request */
export type PullRequestIconKind = "open" | "draft" | "merged" | "closed"

export interface PullRequestChipState {
  icon: PullRequestIconKind
  /** Hue of the icon and the number */
  color: Extract<UIColor, "success" | "error" | "info" | "neutral">
  /** "Approved", "Changes requested", "Review required", "Draft", "Merged", "Closed", "Open" */
  label: string
  /** The head checks failed: a `destructive` dot */
  checksFailing: boolean
  /** How the head relates to the branch now; `current` for a merged or closed pull request, which keeps its last head */
  commit: ReviewCommitState
  /** The branch changed since the head was pushed (not only rebuilt on a newer baseline): a `warning` dot */
  outdated: boolean
}

/**
 * How the chip shows a pull request. The review decision colours an open one; a draft, a merged and a closed one show
 * their state instead. `localTip` is the commit of the local virtual branch, undefined while unknown; `remote` its remote
 * status, null until loaded.
 */
export function getPullRequestChipState(pullRequest: PullRequest, localTip: string | undefined, remote: RemoteStatus | null): PullRequestChipState {
  const open = pullRequest.state === "open"
  const commit = open ? getReviewCommitState(pullRequest.headRefOid, localTip, remote) : "current"
  return {
    ...describeState(pullRequest),
    checksFailing: open && pullRequest.checks?.state === "failing",
    commit,
    outdated: isReviewCommitOutdated(commit),
  }
}

/**
 * What the details say about the commit the pull request shows, null when it shows the branch as it is: a quiet line for
 * a branch only rebuilt on a newer baseline, a push for a changed one, and the wait for GitHub when the remote branch
 * has the change already.
 */
export function formatPullRequestCommitNotice(pullRequest: PullRequest, chip: Pick<PullRequestChipState, "commit">, remoteTip: string | null): string | null {
  const shows = `Shows ${formatShortHash(pullRequest.headRefOid)}`
  switch (chip.commit) {
    case "current":
      return null
    case "rebased":
      return "Shows an earlier build of the same commits."
    case "changedLocally":
      return `${shows}; the branch has changed since: push to update the pull request.`
    case "changedOnRemote":
      return `${shows}; GitHub doesn't show the pushed ${remoteTip === null ? "commit" : formatShortHash(remoteTip)} yet.`
  }
}

function describeState(pullRequest: PullRequest): Pick<PullRequestChipState, "icon" | "color" | "label"> {
  if (pullRequest.state === "merged") {
    return { icon: "merged", color: "info", label: "Merged" }
  }
  if (pullRequest.state === "closed") {
    return { icon: "closed", color: "neutral", label: "Closed" }
  }
  if (pullRequest.isDraft) {
    return { icon: "draft", color: "neutral", label: "Draft" }
  }
  switch (pullRequest.reviewDecision) {
    case "approved":
      return { icon: "open", color: "success", label: "Approved" }
    case "changesRequested":
      return { icon: "open", color: "error", label: "Changes requested" }
    case "reviewRequired":
      return { icon: "open", color: "neutral", label: "Review required" }
    case null:
      return { icon: "open", color: "neutral", label: "Open" }
  }
}

/**
 * What an open pull request waits for from its author: "changes requested", "failing checks" or both; null when it
 * waits for nothing of theirs
 */
export function formatAuthorAttention(pullRequest: PullRequest): string | null {
  if (pullRequest.state !== "open") {
    return null
  }
  const reasons: string[] = []
  if (pullRequest.reviewDecision === "changesRequested") {
    reasons.push("changes requested")
  }
  if (pullRequest.checks?.state === "failing") {
    reasons.push("failing checks")
  }
  return reasons.length === 0 ? null : reasons.join(", ")
}

/** "1 failing (tests), 2 running, 9 passed"; "All 4 passed" */
export function formatChecks(checks: ChecksSummary): string {
  return formatCheckCounts({ failed: checks.failed, failedNames: checks.failedNames, running: checks.pending, passed: checks.passed })
}

const reviewerStateText: Readonly<Record<ReviewerState, string>> = {
  approved: "approved",
  changesRequested: "requested changes",
  commented: "commented",
  requested: "review requested",
}

/** "alice approved", "bob review requested", "carol approved · re-requested": asked again after the review */
export function formatReviewer(reviewer: Reviewer): string {
  const text = `${reviewer.login} ${reviewerStateText[reviewer.state]}`
  return reviewer.requested && reviewer.state !== "requested" ? `${text} · re-requested` : text
}

/** A repository on github.com: `github.com/<owner>/<name>` */
export interface GitHubRepository {
  readonly owner: string
  readonly name: string
}

/**
 * The repository that `origin` pushes to, while GitHub lists its pull requests; null otherwise (not on GitHub, no token,
 * a rejected one, a failed refresh, or not asked yet).
 */
export function gitHubRepositoryOf(availability: PullRequestAvailability | null): GitHubRepository | null {
  return availability?.status === "available" ? { owner: availability.owner, name: availability.name } : null
}

/** What GitHub's form of a new pull request starts with: its query parameters take no reviewers */
export interface CompareFields {
  title?: string
  body?: string
  /** A login */
  assignee?: string
}

/** A longer body is cut: a URL of more than about 8 KB is refused by GitHub's front end */
export const MAX_COMPARE_BODY_LENGTH = 6000

/**
 * The GitHub page that opens a pull request of `head` into `base` (branch names as the remote has them):
 * `https://github.com/JetBrains/ultimate/compare/master...develar/virtual/auth?expand=1`, the form already expanded,
 * with `fields` filled in
 */
export function getCompareUrl(repository: GitHubRepository, base: string, head: string, fields: CompareFields = {}): string {
  // a branch name keeps its slashes, the rest of it is encoded
  const ref = (name: string) => name.split("/").map(encodeURIComponent).join("/")
  const query = new URLSearchParams({ expand: "1" })
  if (fields.title) {
    query.set("title", fields.title)
  }
  if (fields.body) {
    query.set("body", fields.body.length > MAX_COMPARE_BODY_LENGTH ? `${fields.body.slice(0, MAX_COMPARE_BODY_LENGTH)}…` : fields.body)
  }
  if (fields.assignee) {
    query.set("assignees", fields.assignee)
  }
  return `https://github.com/${repository.owner}/${repository.name}/compare/${ref(base)}...${ref(head)}?${query.toString()}`
}

/**
 * A quiet hint why pull requests are not shown, null when they are (or the repository is not on GitHub, which needs no
 * hint). Never a toast: GitHub status is an extra.
 */
export function formatAvailabilityHint(availability: PullRequestAvailability | null): string | null {
  switch (availability?.status) {
    case "signedOut":
      return "No GitHub token to see pull requests"
    case "notAuthenticated":
      return `GitHub rejected the token from ${formatTokenSource(availability.source)}`
    case "rateLimited":
      return "GitHub: rate limit reached, asking again later"
    case "ssoRequired":
      return "GitHub: the organization requires SAML SSO, authorize the token for it"
    case "noAccess":
      return availability.source === "branchDeck"
        ? `GitHub: ${availability.message} (the token from the Branch Deck sign-in). ${ORGANIZATION_APPROVAL}: request it, or run gh auth login`
        : `GitHub: ${availability.message} (the token from ${formatTokenSource(availability.source)})`
    case "error":
      return `GitHub: ${availability.message}`
    default:
      return null
  }
}

/** Why the token of the app's own sign-in may not see a repository that the user can */
export const ORGANIZATION_APPROVAL = "An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck"

/**
 * What fixes an availability, for the button after the hint: no token or a rejected one opens Settings, where the app
 * signs in ("Sign In…") or, in a build without the sign-in, says what gives it a token ("Settings…"); the token of the
 * app's sign-in that can't see the repository opens the page of the app on GitHub, where the user asks an
 * organization to approve it ("Request Access…"); SAML SSO opens GitHub's page that authorizes the token. `auth` is
 * null until known: no button yet.
 */
export function getAvailabilityAction(
  availability: PullRequestAvailability | null,
  auth: Pick<GitHubAuthStatus, "signInAvailable" | "manageUrl"> | null,
): { label: string; onClick: () => void } | null {
  switch (availability?.status) {
    case "signedOut":
    case "notAuthenticated":
      return auth === null ? null : { label: auth.signInAvailable ? "Sign In…" : "Settings…", onClick: () => setSettingsDialogOpen(true) }
    case "noAccess":
      return availability.source === "branchDeck" ? openUrlAction("Request Access…", auth?.manageUrl ?? null) : null
    case "ssoRequired":
      return openUrlAction("Authorize…", availability.url)
    default:
      return null
  }
}

function openUrlAction(label: string, url: string | null): { label: string; onClick: () => void } | null {
  return url === null ? null : { label, onClick: () => void openUrl(url).catch((error: unknown) => notifyError("Cannot open GitHub", error)) }
}

/** Where a GitHub token came from, as `TokenSource::label` says it */
export function formatTokenSource(source: TokenSource): string {
  switch (source) {
    case "environment":
      return "GH_TOKEN"
    case "gh":
      return "gh"
    case "branchDeck":
      return "the Branch Deck sign-in"
    case "gitCredential":
      return "the git credentials"
  }
}

/** Opens the pull request in the browser */
export function openPullRequest(pullRequest: PullRequest): void {
  openPullRequestUrl(pullRequest.url)
}

/** Opens a pull request by its URL, e.g. the one a Patronus robot belongs to */
export function openPullRequestUrl(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the pull request", error))
}
