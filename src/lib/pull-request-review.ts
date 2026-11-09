import { showEventToast } from "@/lib/event-toast"
import { openRobotUrl } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"

/*
 * The review of a pull request in the review window (openspec/specs/repository/review-requests/spec.md#review-window): the list
 * it walks and the toasts of its writes.
 */

/**
 * Where the review of a pull request was opened from: its ↑/↓ walk the rows of that list, its title names it ("Your
 * Turn › 2 of 5", "Reviews › 3 of 14")
 */
export type ReviewSource = "yourTurn" | "reviews"

/** What a toast about a pull request opens */
export type ReviewToastTarget = { readonly kind: "pullRequest"; readonly url: string } | { readonly kind: "robot"; readonly url: string }

/** A toast of a write: "Approved #101" / its title, with "Open Pull Request" or "Open in Patronus" */
export interface ReviewToast {
  /** The toast of the pull request (`toastKeys.review`): its next state, the start after an approval, takes its place */
  readonly key: string
  readonly title: string
  readonly description: string
  readonly target: ReviewToastTarget
}

/** "Open Pull Request", "Open in Patronus" */
export function reviewToastActionLabel(target: ReviewToastTarget): string {
  return target.kind === "robot" ? "Open in Patronus" : "Open Pull Request"
}

/** Says how a write on a pull request ended: in the window that asked for it */
export interface ReviewFeedback {
  readonly toast: (toast: ReviewToast) => void
  readonly error: (title: string, error: unknown) => void
}

/**
 * The toast of a write of the review window, with "Open in Patronus" or "Open Pull Request"; one toast per pull request
 * (the approval, then the Safe Push started after it)
 */
export function showReviewToast({ key, title, description, target }: ReviewToast): void {
  showEventToast({
    key,
    render: () => ({
      type: "success",
      title,
      description,
      action: { label: reviewToastActionLabel(target), onClick: () => (target.kind === "robot" ? openRobotUrl(target.url) : openPullRequestUrl(target.url)) },
    }),
  })
}
