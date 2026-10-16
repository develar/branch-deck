import type { ReviewRequest } from "@/lib/bindings"
import type { ReviewIntent } from "@/lib/review-request-actions"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/*
 * The review window of a pull request (`pull-request-review`, spec/repository/review-requests.spec.md#review-window):
 * the main window owns the requests, the writes and Activity; the window shows a request, fetches it and asks.
 */

/** The label of the review window: one window, reused for the next request */
export const PULL_REQUEST_REVIEW_WINDOW = "pull-request-review"

/** What the main window knows of the requests of the window's repository, sent when it changed */
export const REVIEW_WINDOW_STATE_CHANGED = "review-window-state-changed"

/** A write or a notice of the window (`ReviewWindowRequest`): the main window runs it and answers `REVIEW_WINDOW_REPLY` */
export const REVIEW_WINDOW_REQUEST = "review-window-request"

/** The answer to a `ReviewWindowRequest`, by its id */
export const REVIEW_WINDOW_REPLY = "review-window-reply"

/** A toast or an error of a write the window asked for: the window shows it, the main window recorded it in Activity */
export const REVIEW_WINDOW_FEEDBACK = "review-window-feedback"

/** Where Patronus merges, as the main window knows it: Approve and Safe Push is offered for a pull request into `target` */
export interface ReviewPatronus {
  readonly available: boolean
  /** The branch Patronus pushes into for the baseline; undefined while it is read, null when the baseline is none */
  readonly target: string | null | undefined
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
  readonly intent: ReviewIntent | null
  /** A new value on every open: the same request opened again starts over */
  readonly openedAt: number
  readonly state: ReviewWindowState
}

/** "#101 Sort the start-up report", the title of the review window */
export function reviewWindowTitle(request: Pick<ReviewRequest, "number" | "title">): string {
  return `#${request.number} ${request.title}`
}

/** What a toast about a pull request opens */
export type ReviewToastTarget = { readonly kind: "pullRequest", readonly url: string } | { readonly kind: "robot", readonly url: string }

/** A toast of a write: "Approved #101" / its title, with "Open Pull Request" or "Open in Patronus" */
export interface ReviewToast {
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

export type ReviewWindowRequest
  = | {
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

export interface ReviewWindowReply {
  readonly id: string
  /** The write was done (a refused one was told by `REVIEW_WINDOW_FEEDBACK`) */
  readonly ok: boolean
}

export type ReviewWindowFeedback
  = | { readonly kind: "toast", readonly toast: ReviewToast }
    | { readonly kind: "error", readonly title: string, readonly message: string }

/**
 * Whether Patronus merges `request`: it does in a repository it guards, into the branch it pushes into for the baseline
 * only (a pull request into another base, e.g. of a stacked branch, would be merged there). null: no item.
 */
export function getReviewSafePush(request: ReviewRequest, patronus: ReviewPatronus): { blocked: string | null } | null {
  if (!patronus.available) {
    return null
  }
  if (patronus.target === undefined) {
    return { blocked: "Reading the branch Patronus pushes into…" }
  }
  if (patronus.target === null) {
    return { blocked: "Patronus pushes into a branch of origin: the baseline is none" }
  }
  if (request.baseRefName !== patronus.target) {
    return { blocked: `#${request.number} merges into ${request.baseRefName}: Patronus would merge it there, not into ${patronus.target}` }
  }
  return { blocked: null }
}
