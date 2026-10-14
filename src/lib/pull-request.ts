import { openUrl } from "@tauri-apps/plugin-opener"
import type { ChecksSummary, PullRequest, PullRequestAvailability, Reviewer, ReviewerState } from "@/lib/bindings"
import { formatCheckCounts } from "@/lib/check-counts"
import { notifyError } from "@/lib/errors"
import type { UIColor } from "@/lib/ui-types"

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
  /** The pull request does not show the local branch tip yet: a `warning` dot */
  outdated: boolean
}

/**
 * How the chip shows a pull request. The review decision colours an open one; a draft, a merged and a closed one show
 * their state instead. `localTip` is the commit of the local virtual branch, undefined while unknown.
 */
export function getPullRequestChipState(pullRequest: PullRequest, localTip: string | undefined): PullRequestChipState {
  const open = pullRequest.state === "open"
  return {
    ...describeState(pullRequest),
    checksFailing: open && pullRequest.checks?.state === "failing",
    outdated: isPullRequestOutdated(pullRequest, localTip),
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

/** An open pull request whose head is not the local tip: the branch changed since the last push */
export function isPullRequestOutdated(pullRequest: PullRequest, localTip: string | undefined): boolean {
  return pullRequest.state === "open" && localTip !== undefined && localTip !== "" && pullRequest.headRefOid !== localTip
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

export function formatReviewer(reviewer: Reviewer): string {
  return `${reviewer.login} ${reviewerStateText[reviewer.state]}`
}

/**
 * A quiet hint why pull requests are not shown, null when they are (or the repository is not on GitHub, which needs no
 * hint). Never a toast: GitHub status is an extra.
 */
export function formatAvailabilityHint(availability: PullRequestAvailability | null): string | null {
  switch (availability?.status) {
    case "ghMissing":
      return "Install the GitHub CLI (gh) to see pull requests"
    case "notAuthenticated":
      return "Run gh auth login to see pull requests"
    case "error":
      return `GitHub: ${availability.message}`
    default:
      return null
  }
}

/** Opens the pull request in the browser */
export function openPullRequest(pullRequest: PullRequest): void {
  openUrl(pullRequest.url).catch((error: unknown) => notifyError("Cannot open the pull request", error))
}
