import type { ReviewRequest } from "@/lib/bindings"
import type { ReviewIntent } from "@/lib/review-request-actions"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

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

/** Where Patronus merges, as the main window knows it: Approve and Safe Push is offered for a pull request into `target` */
export interface ReviewPatronus {
  readonly available: boolean
  /** The branch Patronus pushes into for the baseline; undefined while it is read, null when the baseline is none */
  readonly target: string | null | undefined
  /** Why the target couldn't be read (`target` undefined then): no "the baseline is none" */
  readonly targetError?: string
}

export interface ReviewWindowState {
  readonly repositoryPath: string
  /** The main window shows this repository: only then can it start a Safe Push (Patronus asks for the selected one) */
  readonly selected: boolean
  /** Waiting the longest first, as in Your Turn */
  readonly requests: readonly ReviewRequest[]
  readonly reviewed: Readonly<Record<number, SubmittedReview>>
  readonly busy: Readonly<Record<number, ReviewAction>>
  readonly patronus: ReviewPatronus
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
 * Whether Patronus merges `request`: it does in a repository it guards, into the branch it pushes into for the baseline
 * only (a pull request into another base, e.g. of a stacked branch, would be merged there). null: no item.
 */
export function getReviewSafePush(request: ReviewRequest, patronus: ReviewPatronus): { blocked: string | null } | null {
  if (!patronus.available) {
    return null
  }
  if (patronus.target === undefined) {
    return { blocked: patronus.targetError === undefined ? "Reading the branch Patronus pushes into…" : `Cannot read the branch Patronus pushes into: ${patronus.targetError}` }
  }
  if (patronus.target === null) {
    return { blocked: "Patronus pushes into a branch of origin: the baseline is none" }
  }
  if (request.baseRefName !== patronus.target) {
    return { blocked: `#${request.number} merges into ${request.baseRefName}: Patronus would merge it there, not into ${patronus.target}` }
  }
  return { blocked: null }
}

/**
 * What the review window of a pull request is keyed by: another request, or an open with an intent (a write of the
 * row's menu, D on the row) starts it over; a plain open of the request on screen only brings the window to the front,
 * where the review is
 */
export function pullRequestReviewWindowKey(data: PullRequestReviewWindowData): string {
  const subject = `${data.repositoryPath}\n${data.number}`
  return data.intent === null ? subject : `${subject}\n${data.openedAt}`
}
