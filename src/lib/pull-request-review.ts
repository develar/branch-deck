import type { ActionState, ReviewInvolvement, ReviewRequest } from "@/lib/bindings"
import type { ReviewIntent } from "@/lib/review-request-actions"
import type { ReviewAction } from "@/stores/review-requests"

/*
 * The review window of a pull request (`pull-request-review`, spec/repository/review-requests.spec.md#review-window):
 * the main window owns the requests, the writes and Activity; the window shows a request, fetches it and asks.
 */

/**
 * The label of the review window: one window, reused for the next request. Its link (`@/lib/sub-window-link`) sends the
 * state of the requests of its repository (`ReviewWindowState`), and carries its writes and notices
 * (`ReviewWindowRequest`), their answers and their toasts or errors (`ReviewWindowFeedback`).
 */
export const PULL_REQUEST_REVIEW_WINDOW = "pull-request-review"

/**
 * Where the review window was opened from: its ↑/↓ walk the rows of that list, its title names it ("Your Turn › 2 of
 * 5", "Reviews › 3 of 14")
 */
export type ReviewSource = "yourTurn" | "reviews"

export interface ReviewWindowState {
  readonly repositoryPath: string
  /** The main window shows this repository: only then can it start a Safe Push (Patronus asks for the selected one) */
  readonly selected: boolean
  readonly source: ReviewSource
  /** The rows of the source: the requests waiting the longest first, as in Your Turn; the pull requests of Reviews in its order */
  readonly requests: readonly ReviewRequest[]
  /** How the user takes part in each pull request of Reviews, by number; none of a request by name */
  readonly involvements: Readonly<Record<number, ReviewInvolvement>>
  /** The actions of each pull request Rust decides (`pull:<number>` of the view), of the repository the main window shows */
  readonly actions: Readonly<Record<number, readonly ActionState[]>>
  readonly busy: Readonly<Record<number, ReviewAction>>
}

/** `__INIT_DATA__` of the review window (`openPullRequestReviewWindow`) */
export interface PullRequestReviewWindowData {
  /** The repository whose diffs the window reads (`getDiffRepositoryPath`) */
  readonly repositoryPath: string
  readonly number: number
  /** The request when it opened: it stays while it is withdrawn */
  readonly request: ReviewRequest
  /** Its place in the list when it opened */
  readonly index: number
  /** The write the row's menu asked for, or "review" (D on the row): an open with one starts the window over */
  readonly intent: ReviewIntent | null
  readonly source: ReviewSource
  /** A new value on every open */
  readonly openedAt: number
  readonly state: ReviewWindowState
}

/** "Review #101 Sort the start-up report", the title of the review window, as "Review login" and "Review a1b2c3d …" */
export function reviewWindowTitle(request: Pick<ReviewRequest, "number" | "title">): string {
  return `Review #${request.number} ${request.title}`
}

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

export type ReviewWindowRequest =
  | {
      readonly id: string
      readonly action: "approve" | "requestChanges" | "safePush"
      readonly repositoryPath: string
      /** The whole request: an approved one has left the list of the main window */
      readonly request: ReviewRequest
      readonly commitId: string
      /** The comment of a request for changes */
      readonly body: string | null
    }
  | {
      readonly id: string
      /** The window shows `request`: it is read, and its review refs stay */
      readonly action: "shown"
      readonly repositoryPath: string
      readonly request: ReviewRequest
    }
  | {
      readonly id: string
      readonly action: "openOnGitHub"
      readonly repositoryPath: string
      readonly request: ReviewRequest
    }

export type ReviewWindowFeedback = { readonly kind: "toast"; readonly toast: ReviewToast } | { readonly kind: "error"; readonly title: string; readonly message: string }

/**
 * What the review window of a pull request is keyed by: another request, or an open with an intent (a write of the
 * row's menu, D on the row) starts it over; a plain open of the request on screen only brings the window to the front,
 * where the review is
 */
export function pullRequestReviewWindowKey(data: PullRequestReviewWindowData): string {
  const subject = `${data.repositoryPath}\n${data.number}\n${data.source}`
  return data.intent === null ? subject : `${subject}\n${data.openedAt}`
}
