// The words and the menu item of Request Review… (spec/branch/pull-requests.spec.md, "Request Review"): whom it offers,
// what it says of the code owners while Patronus can't name them, and the toast of whom GitHub asked; the request is
// the intent of the engine (`requestReview` of `pull-request-run.ts`), the candidates Rust's (`review::candidates`).
import type { ActionState, PullRequest, PullRequestView, RequestedReviewers, ReviewCandidate, ReviewReadiness } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { joinNames } from "@/lib/text"
import type { ToastMessage } from "@/lib/toast-message"
import type { BlockerWords } from "@/presenters/actions"
import { presentAction } from "@/presenters/actions"

/** "Ilya Korennoy", else the login */
export function formatCandidate(candidate: Pick<ReviewCandidate, "login" | "fullName">): string {
  return candidate.fullName ?? candidate.login
}

/** What "Request Review…" of a subject (a branch, `stack:<root>`) is built of, as its caller selected them */
export interface RequestReviewParts {
  /** Its open pull request (`RepositoryView.openPullRequests` of the subject); none: no item */
  readonly pullRequest: PullRequestView | undefined
  /** Its actions (`RepositoryView.actions` of the subject), `request-review` among them while Rust offers it */
  readonly actions: readonly ActionState[] | undefined
  /** Opens its popover (`openRequestReview` of the subject) */
  readonly open: () => void
}

/**
 * "Request Review…" of the open pull request of a subject, as Rust offers it (none without an open pull request,
 * disabled for a draft): its popover asks the code owners Patronus suggests (the readiness of the subject), the push
 * first, running in the words of its operation (`words.operations`); null without one
 */
export function getRequestReviewItem(parts: RequestReviewParts, words: BlockerWords): MenuItem | null {
  const { pullRequest } = parts
  if (pullRequest === undefined) {
    return null
  }
  return presentAction(parts.actions, "request-review", { description: describeRequestReview(pullRequest), onSelect: parts.open }, words)
}

/**
 * What "Request Review…" of the open pull request `pullRequest` does (`PullRequestView`), whose availability Rust decides
 * (`domain_policy::actions`: a draft asks nobody): the code owners Patronus preselects, else a review of it; work the
 * remote lacks is no reason, the request pushes it first (its plan's `Push`), so the reviewers see the commits
 */
export function describeRequestReview(pullRequest: PullRequestView): string {
  const chosen = pullRequest.candidates.filter(candidate => candidate.preselected)
  return chosen.length > 0 ? `Ask ${joinNames(chosen.map(formatCandidate))} to review #${pullRequest.number}` : `Ask for a review of #${pullRequest.number}`
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
