// The page's words of a review after landing (show of ship/show/ask, openspec/specs/branch/shown-reviews/spec.md) where
// the user acts on it: the popover of Safe Push and Request Review… (what the start opens, whom it asks by default), the
// confirmation of Close Review… by what the review waits for (`ShownState`, Rust's), and the toasts of a request and a
// close. It decides nothing: Rust offers, plans and refuses each.
import type { ShownReview, ShownState, ShowReviewers } from "@/lib/bindings"
import { joinNames } from "@/lib/text"

/** What a Safe Push and Request Review… opens beside the robot, a line of its popover */
export function describeShowStart(target: string): string {
  return `A draft pull request into the fork point opens now and asks the reviewers once the Safe Push merged into ${target}; it closes once they all approved.`
}

/**
 * Whom the picker starts with, under it: the owner groups it asks one available code owner of, those of which nobody can
 * be asked, or why it starts empty; null while the code owners are read
 */
export function describeShowCandidates(
  candidates: { readonly kind: "loading" } | { readonly kind: "loaded"; readonly value: ShowReviewers } | { readonly kind: "failed"; readonly message: string },
): string | null {
  switch (candidates.kind) {
    case "loading":
      return null
    case "failed":
      return `Cannot read the code owners of the changes: ${candidates.message}. Add the reviewers by their GitHub login.`
    case "loaded": {
      const { groups, reviewers, withoutReviewer } = candidates.value
      if (groups.length === 0) {
        return "No OWNERSHIP file names an owner of the changes: add the reviewers by their GitHub login."
      }
      const asked = groups.filter(group => !withoutReviewer.includes(group))
      const parts = [
        ...(reviewers.length > 0 ? [`One available code owner of ${joinNames(asked)}`] : []),
        ...(withoutReviewer.length > 0 ? [`nobody of ${joinNames(withoutReviewer)} can be asked`] : []),
      ]
      const text = parts.join(" · ")
      return `${text.charAt(0).toUpperCase()}${text.slice(1)}.`
    }
  }
}

/** The confirmation of Close Review… of the review after landing `review` of `branch`: its title and what the close does */
export function describeCloseShownReview(review: ShownReview, branch: string): { readonly title: string; readonly description: string } {
  const number = review.pullRequest.number
  const title = `Close review after landing #${number}`
  const deletes = "Closes it on GitHub with a comment and deletes its branches; the change stays in master."
  return { title, description: `${closeStateWords(review.state, number, branch)} ${deletes}` }
}

/** What the review waits for as the close says it: closed approved, or closed anyway without what it waits for */
function closeStateWords(state: ShownState, number: number, branch: string): string {
  switch (state.kind) {
    case "approved":
      return `${joinNames(state.by)} approved #${number}.`
    case "inReview":
      return state.waiting.length === 0
        ? `#${number} asked nobody yet: it closes without a review.`
        : `${joinNames(state.waiting)} has not approved #${number}: it closes without that approval.`
    case "changesRequested":
      return `${joinNames(state.by)} requested changes in #${number}: it closes without a fix.`
    case "extending":
      return `A fix of ${branch} lands, which #${number} has not taken yet: it closes before its review.`
    case "draft":
      return `#${number} is a draft that asked nobody yet: it closes without a review.`
  }
}

/** The toast of a close: "Closed review after landing #12 of auth" */
export function formatShownClosedTitle(number: number, branch: string): string {
  return `Closed review after landing #${number} of ${branch}`
}

/**
 * The toast of a request: "Asked alice and bob to review #12 after landing"; of a draft, whose open asks them once its
 * Safe Push merged, "#12 asks alice once the Safe Push merged"
 */
export function formatShownRequestedTitle(number: number, reviewers: readonly string[], draft: boolean): string {
  return draft ? `#${number} asks ${joinNames(reviewers)} once the Safe Push merged` : `Asked ${joinNames(reviewers)} to review #${number} after landing`
}

/** The badge of a review after landing to review (Your Turn, Reviews, the review window): its label and its hover */
export const SHOWN_BADGE = {
  label: "Shown",
  hint: "In master already: your review gates no Safe Push, and the approval of every reviewer it asked closes it",
} as const
