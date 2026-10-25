import { UserPlusIcon } from "lucide-react"
import type { PullRequest, PullRequestView, RequestedReviewers, ReviewCandidate, ReviewReadiness } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { joinNames } from "@/lib/text"
import type { ToastMessage } from "@/lib/toast-message"

/** "Ilya Korennoy", else the login */
export function formatCandidate(candidate: Pick<ReviewCandidate, "login" | "fullName">): string {
  return candidate.fullName ?? candidate.login
}

/**
 * "Request Review…" of a branch, which opens the popover of its open pull request (`open`), as Rust reads it
 * (`PullRequestView`): disabled with the reason while it is a draft; work the remote lacks is no reason, the request
 * pushes it first (`pushForAction`), so the reviewers see the commits. Null without an open pull request, where it
 * never applies (Create Pull Request… asks the reviewers then).
 */
export function getRequestReviewItem(pullRequest: PullRequestView | undefined, open: () => void): MenuItem | null {
  if (pullRequest === undefined) {
    return null
  }
  const reason = pullRequest.draft ? formatDraftReason(pullRequest.number) : null
  const chosen = pullRequest.candidates.filter(candidate => candidate.preselected)
  return {
    id: "request-review",
    label: "Request Review…",
    icon: UserPlusIcon,
    disabled: reason !== null,
    description: reason ?? (chosen.length > 0 ? `Ask ${joinNames(chosen.map(formatCandidate))} to review #${pullRequest.number}` : `Ask for a review of #${pullRequest.number}`),
    onSelect: open,
  }
}

/** Why nobody is asked to review a draft (`PullRequestView.draft`) */
export function formatDraftReason(number: number): string {
  return `#${number} is a draft: mark it ready for review on GitHub first`
}

/**
 * What the popover says of the code owners while Patronus can't name them: asked and not answered yet (`null`, where
 * Patronus answers for the repository), a check that failed (`unavailable`), one that doesn't tell yet (`unknown`);
 * null once it named them, where none is needed, and off Space
 */
export function describeCodeOwnersFact(readiness: ReviewReadiness | null, patronus: boolean): string | null {
  if (readiness === null) {
    return patronus ? "Asking Patronus which code owners must approve…" : null
  }
  switch (readiness.kind) {
    case "unavailable":
      return `Patronus didn't answer which code owners must approve (${readiness.message}): ask the reviewers yourself.`
    case "unknown":
      return "Patronus doesn't tell yet which code owners must approve: ask the reviewers yourself."
    default:
      return null
  }
}

/**
 * The toast of a request GitHub asked someone in: "Requested a review from Ilya Korennoy on #1078", a `warning` whose
 * description names whom GitHub refused ("GitHub refused Bob: …"); null when it asked nobody (an error, `formatRefused`
 * says why). `nameOf` gives the full name of a login the popover knows.
 */
export function formatRequestedToast(pullRequest: Pick<PullRequest, "number">, result: RequestedReviewers, nameOf: (login: string) => string): Omit<ToastMessage, "action"> | null {
  if (result.requested.length === 0) {
    return null
  }
  const title = `Requested a review from ${joinNames(result.requested.map(nameOf))} on #${pullRequest.number}`
  if (result.refused.length === 0) {
    return { title }
  }
  return { type: "warning", title, description: result.refused.map(refused => `GitHub refused ${nameOf(refused.login)}: ${refused.reason}`).join(" · ") }
}

/** Whom GitHub refused, with its reason: "Bob: Review cannot be requested from pull request author." */
export function formatRefused(result: RequestedReviewers, nameOf: (login: string) => string): string {
  return result.refused.map(refused => `${nameOf(refused.login)}: ${refused.reason}`).join("; ")
}
