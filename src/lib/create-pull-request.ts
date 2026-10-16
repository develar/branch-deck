import type { CreatedPullRequest, GitHubUser, PatronusReviewRequirement } from "@/lib/bindings"
import type { ToastMessage } from "@/lib/toast-message"

/** A code owner Patronus suggests for the review of the branch, by group */
export interface SuggestedReviewer {
  readonly group: string
  readonly fullName: string
  /** The GitHub login Patronus knows; null while it doesn't say (the user looks the reviewer up) */
  readonly login: string | null
}

/**
 * The suggested and available reviewers of each group whose approval is missing, as `formatReviewerGroup` names them,
 * without the user (GitHub refuses a review request of the author); none when no approval is missing
 */
export function suggestedReviewers(requirement: PatronusReviewRequirement, viewerLogin: string | null): SuggestedReviewer[] {
  if (!requirement.required) {
    return []
  }
  return requirement.groups.flatMap(group =>
    group.reviewers
      .filter(reviewer => reviewer.suggested && reviewer.available)
      .filter(reviewer => !isLogin(reviewer.githubLogin, viewerLogin))
      .map(reviewer => ({ group: group.name, fullName: reviewer.fullName, login: reviewer.githubLogin })),
  )
}

/**
 * The reviewers the dialog starts with: the first suggested reviewer with a login of each group, once (a group whose
 * suggestion is already chosen for another group is covered); the user sees them before creating and may remove them
 */
export function preselectedReviewers(suggestions: readonly SuggestedReviewer[]): GitHubUser[] {
  const chosen: GitHubUser[] = []
  const coveredGroups = new Set<string>()
  for (const suggestion of suggestions) {
    if (suggestion.login === null || coveredGroups.has(suggestion.group)) {
      continue
    }
    coveredGroups.add(suggestion.group)
    if (!chosen.some(user => isLogin(user.login, suggestion.login))) {
      chosen.push({ login: suggestion.login, name: suggestion.fullName })
    }
  }
  return chosen
}

/** GitHub logins are case-insensitive */
export function isLogin(login: string | null | undefined, other: string | null | undefined): boolean {
  return login != null && other != null && login.toLowerCase() === other.toLowerCase()
}

/** The users a search found that are not chosen yet, without the user */
export function searchResults(users: readonly GitHubUser[], chosen: readonly GitHubUser[], viewerLogin: string | null): GitHubUser[] {
  return users.filter(user => !isLogin(user.login, viewerLogin) && !chosen.some(it => isLogin(it.login, user.login)))
}

/** "alice", "alice and bob", "alice, bob and carol" */
export function joinLogins(logins: readonly string[]): string {
  return logins.length <= 1 ? (logins[0] ?? "") : `${logins.slice(0, -1).join(", ")} and ${logins.at(-1)}`
}

/**
 * The toast of a creation: "Created pull request #123" / "feature-auth → master · review requested from alice ·
 * assigned to you", a warning naming what GitHub refused; "feature-auth already has pull request #7" for one that
 * existed, which got nothing of the dialog
 */
export function formatCreatedToast(branchName: string, baseName: string, viewerLogin: string | null, result: CreatedPullRequest): Omit<ToastMessage, "action"> {
  const { pullRequest } = result
  if (!result.created) {
    return {
      type: "info",
      title: `${branchName} already has pull request #${pullRequest.number}`,
      description: "The title, the reviewers and the assignee weren't applied to it",
    }
  }
  const parts = [`${branchName} → ${baseName}`]
  const requested = pullRequest.reviewers.filter(reviewer => reviewer.state === "requested").map(reviewer => reviewer.login)
  if (requested.length > 0) {
    parts.push(`review requested from ${joinLogins(requested)}`)
  }
  if (result.assignees.some(login => isLogin(login, viewerLogin)) || (viewerLogin === null && result.assignees.length > 0)) {
    parts.push("assigned to you")
  }
  parts.push(...result.warnings)
  return {
    type: result.warnings.length > 0 ? "warning" : "success",
    title: `Created pull request #${pullRequest.number}`,
    description: parts.join(" · "),
  }
}

/**
 * Hands out a number per request and tells whether an answer is of the latest one: a search answers out of order, and
 * an older answer must not replace the newer one
 */
export function createLatestOnly(): { next: () => number; isLatest: (id: number) => boolean } {
  let latest = 0
  return {
    next: () => ++latest,
    isLatest: id => id === latest,
  }
}
