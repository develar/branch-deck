// The page's words of a review after landing (show of ship/show/ask, openspec/specs/branch/shown-reviews/spec.md) where
// the user acts on it: the popover of Safe Push and Request Review… (what the start opens, whom it asks by default), the
// confirmation of Close Review… by what the review waits for (`ShownState`, Rust's), and the toasts of a request and a
// close. It decides nothing: Rust offers, plans and refuses each.
import type { ShownReview, ShownState, ShowReviewers } from "@/lib/bindings"
import { joinNames } from "@/lib/text"

/** What a Safe Push and Request Review… does beside the robot, a line of its popover */
export function describeShowStart(branch: string, target: string): string {
  return `${branch} lands now; the reviewers are asked once it is in ${target}. The review closes when they all approve; it can never land itself.`
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
  const closes = `#${number} closes on GitHub with a comment and its branches are deleted; the change stays in master.`
  return { title: `Close Review #${number}?`, description: `${closeStateWords(review.state, number, branch)} ${closes}` }
}

/** What the review waits for as the close says it: who approved it, or what closing now leaves without a review */
function closeStateWords(state: ShownState, number: number, branch: string): string {
  switch (state.kind) {
    case "approved":
      return `${joinNames(state.by)} approved #${number}.`
    case "inReview":
      return state.waiting.length === 0
        ? `#${number} asked nobody yet; closing now ends it without a review.`
        : `${joinNames(state.waiting)} ${state.waiting.length === 1 ? "has" : "have"} not approved #${number} yet; closing now ends the review without that approval.`
    case "changesRequested":
      return `${joinNames(state.by)} requested changes in #${number}; closing now ends the review without a fix.`
    case "extending":
      return `A fix of ${branch} is still landing and is not in #${number} yet; closing now leaves it unreviewed.`
    case "draft":
      return `#${number} is a draft that asked nobody yet; closing now ends it without a review.`
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
  hint: "Already in master: your review does not block landing. The review closes when every requested reviewer approves.",
} as const

/** The hover of the badge of the author's own review after landing a reviewer handed back ("Returned to you") */
export function formatOwnShownHint(branch: string, number: number): string {
  return `Already in master: reviewed after landing. A fix of ${branch} goes into #${number} by its own Safe Push.`
}
