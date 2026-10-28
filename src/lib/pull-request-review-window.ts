import type { ActionState, ReviewRequest } from "@/lib/bindings"
import { notifyError, recordError } from "@/lib/errors"
import { closeEventToasts } from "@/lib/event-toast"
import { PULL_REQUEST_REVIEW_WINDOW, reviewWindowTitle } from "@/lib/pull-request-review"
import type {
  PullRequestReviewWindowData,
  ReviewFeedback,
  ReviewSource,
  ReviewToast,
  ReviewWindowFeedback,
  ReviewWindowRequest,
  ReviewWindowState,
} from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { createWindowLink } from "@/lib/sub-window-link"
import { showReviewWindowPullRequest } from "@/model/demand"
import { getRowPart, sessionRepositoryPath } from "@/model/store"
import { branchSyncStore } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"
import {
  approveAndSafePush,
  markReviewRequestRead,
  openRequestOnGitHub,
  connectReviewWindow,
  getReviewBusy,
  mainReviewFeedback,
  reviewRequestsStore,
  submitReview,
} from "@/stores/review-requests"
import { reviewRows } from "@/stores/reviews"

/*
 * The main window's side of the review window (`pull-request-review`, spec/repository/review-requests.spec.md#review-window):
 * opens it, tells it the state of the requests of its repository, and runs the writes it asks for with its feedback.
 */

/** The request the review window shows, while it is open: its review refs stay */
const link = createWindowLink<Shown, ReviewWindowState>({
  label: PULL_REQUEST_REVIEW_WINDOW,
  buildState: shown => buildState(shown.repositoryPath, shown.source),
  // the session no longer keeps its actions for it, and prunes its review refs once its demand names it no more
  onClose: () => releaseShownDemand(),
})

/** What the review window shows: the pull request, and the list it walks */
interface Shown {
  readonly repositoryPath: string
  readonly number: number
  readonly source: ReviewSource
}

/** The release of the demand of the pull request the window shows: its actions stay in the view while it shows it */
let releaseShown: (() => void) | null = null

function releaseShownDemand(): void {
  releaseShown?.()
  releaseShown = null
}

/** The window shows `shown` from now: its refs stay, the session keeps its actions */
function setShown(shown: Shown): void {
  link.setShown(shown)
  releaseShownDemand()
  releaseShown = showReviewWindowPullRequest(shown.number)
}

connectReviewWindow({
  open: (repositoryPath, request, index, intent, source) => void openPullRequestReviewWindow(repositoryPath, request, index, intent, source),
  rowsOf: repositoryPath => reviewRows(repositoryPath).requests,
})

/**
 * Opens the review window on `request` (or shows it in the open one), the only review of a pull request: its data is
 * the state of the requests of the list it was opened from (`source`) and the write the row's menu asked for
 * (`intent`); the window fetches the pull request itself. Its close forgets it and prunes the review refs.
 */
export async function openPullRequestReviewWindow(
  repositoryPath: string,
  request: ReviewRequest,
  index: number,
  intent: ReviewIntent | null,
  source: ReviewSource = "yourTurn",
): Promise<void> {
  // the window fetches it: the demand names it, so the session keeps its review refs and prunes the others
  const shown = { repositoryPath, number: request.number, source }
  setShown(shown)
  markReviewRequestRead(repositoryPath, request)
  const state = link.takeState()!
  const data: PullRequestReviewWindowData = { repositoryPath, number: request.number, request, index, intent, source, openedAt: Date.now(), state }
  try {
    await link.open(shown, { url: "/pull-request-review", title: reviewWindowTitle(request), data })
  } catch (error) {
    notifyError(`Cannot open the review of #${request.number}`, error)
  }
}

/**
 * What the window shows of the list it was opened from: the requests of Your Turn, or the pull requests of Reviews with
 * how the user takes part in each; the actions Rust decides of each, of the repository the main window shows
 */
function buildState(repositoryPath: string, source: ReviewSource): ReviewWindowState {
  const entry = reviewRequestsStore.getState().repositories[repositoryPath]
  const selected = getSelectedProject()?.path === repositoryPath
  const { requests, involvements } = source === "yourTurn" ? { requests: entry?.requests ?? [], involvements: {} } : reviewRows(repositoryPath)
  // the reviews run in the session of the repository: of the one the main window shows, its view has them
  const inSession = sessionRepositoryPath() === repositoryPath
  const shown = link.shown()
  const numbers = new Set([...requests.map(request => request.number), ...(shown?.repositoryPath === repositoryPath ? [shown.number] : [])])
  const actions: Record<number, readonly ActionState[]> = {}
  if (inSession) {
    for (const number of numbers) {
      const of = getRowPart(`pull:${number}`, "actions")
      if (of !== undefined) {
        actions[number] = of
      }
    }
  }
  return {
    repositoryPath,
    selected,
    source,
    requests,
    involvements,
    actions,
    busy: inSession ? getReviewBusy(branchSyncStore.getState().operations) : {},
  }
}

/**
 * Tells the review window the state of the requests of its repository when it changed: a reviewed request, a write that
 * runs, a poll, the main window showing another repository. Main window only.
 */
export function broadcastReviewWindowState(): void {
  link.broadcast()
}

/**
 * The feedback of a write the window asked for: the window shows the toast while it still shows the request, and the
 * toast of the request in the main window closes; once it closed or moved to another one (approve, then Escape), the
 * main window does, in place of that toast. The main window records the error in Activity either way.
 */
function windowFeedback(request: ReviewRequest, send: (feedback: ReviewWindowFeedback) => void): ReviewFeedback {
  const isShown = () => link.shown()?.number === request.number
  const main = mainReviewFeedback(request)
  return {
    toast: (toast: ReviewToast) => {
      if (!isShown()) {
        main.toast(toast)
        return
      }
      // the window says it: the toast of the request in the main window has nothing left to offer
      closeEventToasts(toast.key)
      send({ kind: "toast", toast })
    },
    error: (title, error) => (isShown() ? send({ kind: "error", title, message: recordError(title, error) }) : main.error(title, error)),
  }
}

/** Runs what the review window asks for, in the main window, and answers it. Main window only. */
export const runReviewWindowRequest = link.serve<ReviewWindowRequest, ReviewWindowFeedback>((request, send) => {
  const { repositoryPath } = request
  switch (request.action) {
    case "shown":
      setShown({ repositoryPath, number: request.request.number, source: link.shown()?.source ?? "yourTurn" })
      markReviewRequestRead(repositoryPath, request.request)
      broadcastReviewWindowState()
      return true
    case "openOnGitHub":
      openRequestOnGitHub(request.request)
      return true
    case "approve":
    case "requestChanges":
      return submitReview(repositoryPath, request.request, request.action, request.commitId, request.body, windowFeedback(request.request, send))
    case "safePush":
      return approveAndSafePush(repositoryPath, request.request, request.commitId, windowFeedback(request.request, send))
  }
})
