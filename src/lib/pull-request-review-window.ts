import type { ReviewRequest } from "@/lib/bindings"
import { logError, notifyError, recordError } from "@/lib/errors"
import { PULL_REQUEST_REVIEW_WINDOW, reviewWindowTitle } from "@/lib/pull-request-review"
import type { PullRequestReviewWindowData, ReviewFeedback, ReviewPatronus, ReviewToast, ReviewWindowFeedback, ReviewWindowRequest, ReviewWindowState } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { createWindowLink } from "@/lib/sub-window-link"
import { isPatronusAvailable, readPatronusTargetBranch } from "@/stores/patronus"
import { getSelectedProject } from "@/stores/repository"
import {
  approveAndSafePush,
  markReviewRequestRead,
  noteReviewRefsWritten,
  openRequestOnGitHub,
  connectReviewWindow,
  pruneReviewRefsOf,
  reviewRequestsStore,
  submitReview,
} from "@/stores/review-requests"

/*
 * The main window's side of the review window (`pull-request-review`, spec/repository/review-requests.spec.md#review-window):
 * opens it, tells it the state of the requests of its repository, and runs the writes it asks for with its feedback.
 */

/** The request the review window shows, while it is open: rows open there, and its review refs stay */
const link = createWindowLink<{ repositoryPath: string, number: number }, ReviewWindowState>({
  label: PULL_REQUEST_REVIEW_WINDOW,
  buildState: shown => buildState(shown.repositoryPath),
  // its refs go with it
  onClose: closed => pruneReviewRefsOf(closed.repositoryPath),
})
/** The branch Patronus pushes into, by repository, once read */
const patronusTargets = new Map<string, string | null>()
/** The reads of `patronusTargets` in flight */
const patronusReads = new Map<string, Promise<void>>()

/** The review window is open on a request of `repositoryPath`: Your Turn opens its rows there */
export function isReviewWindowOpenFor(repositoryPath: string): boolean {
  return link.isOpenFor(repositoryPath)
}

/** The number of the request the review window shows in `repositoryPath`, if any: its refs stay */
export function reviewWindowNumber(repositoryPath: string): number | null {
  const shown = link.shown()
  return shown?.repositoryPath === repositoryPath ? shown.number : null
}

connectReviewWindow({
  shownNumber: reviewWindowNumber,
  show: (repositoryPath, request, index, intent, always) => {
    if (!always && !isReviewWindowOpenFor(repositoryPath)) {
      return false
    }
    void openPullRequestReviewWindow(repositoryPath, request, index, intent)
    return true
  },
})

/**
 * Opens the review window on `request` (or shows it in the open one): its data is the state of the requests of the
 * repository; the window fetches the pull request itself. Its close forgets it and prunes the review refs.
 */
export async function openPullRequestReviewWindow(repositoryPath: string, request: ReviewRequest, index: number, intent: ReviewIntent | null): Promise<void> {
  // the window fetches it: the next prune keeps its refs, deletes those of the request it showed before
  const shown = { repositoryPath, number: request.number }
  link.setShown(shown)
  noteReviewRefsWritten(repositoryPath)
  markReviewRequestRead(repositoryPath, request)
  // the window opens knowing whether Patronus merges the request
  await readPatronusTarget(repositoryPath)
  const state = link.takeState()!
  const data: PullRequestReviewWindowData = { repositoryPath, number: request.number, request, index, intent, openedAt: Date.now(), state }
  try {
    await link.open(shown, { url: "/pull-request-review", title: reviewWindowTitle(request), data })
  }
  catch (error) {
    notifyError(`Cannot open the review of #${request.number}`, error)
  }
}

/** What the window shows of the requests of its repository; Patronus as the main window knows it for the selected one */
function buildState(repositoryPath: string): ReviewWindowState {
  const entry = reviewRequestsStore.getState().repositories[repositoryPath]
  const selected = getSelectedProject()?.path === repositoryPath
  return {
    repositoryPath,
    selected,
    requests: entry?.requests ?? [],
    reviewed: entry?.reviewed ?? {},
    busy: entry?.busy ?? {},
    patronus: selected ? reviewPatronus(repositoryPath) : { available: false, target: null },
  }
}

/** Patronus of the selected repository: the target is read once per repository, then the window is told again */
function reviewPatronus(repositoryPath: string): ReviewPatronus {
  if (!isPatronusAvailable()) {
    return { available: false, target: null }
  }
  if (!patronusTargets.has(repositoryPath)) {
    void readPatronusTarget(repositoryPath).then(broadcastReviewWindowState)
  }
  return { available: true, target: patronusTargets.get(repositoryPath) }
}

/** Reads the branch Patronus pushes into for the selected repository, once; nothing without Patronus */
function readPatronusTarget(repositoryPath: string): Promise<void> {
  if (!isPatronusAvailable() || patronusTargets.has(repositoryPath) || getSelectedProject()?.path !== repositoryPath) {
    return Promise.resolve()
  }
  let read = patronusReads.get(repositoryPath)
  if (read === undefined) {
    read = readPatronusTargetBranch()
      .then(target => void patronusTargets.set(repositoryPath, target), (error: unknown) => logError("Cannot read the branch Patronus pushes into", error))
      .finally(() => patronusReads.delete(repositoryPath))
    patronusReads.set(repositoryPath, read)
  }
  return read
}

/**
 * Tells the review window the state of the requests of its repository when it changed: a reviewed request, a write that
 * runs, a poll, the main window showing another repository. Main window only.
 */
export function broadcastReviewWindowState(): void {
  link.broadcast()
}

/** The feedback of a write the window asked for: it shows the toast; the main window records the error in Activity */
function windowFeedback(send: (feedback: ReviewWindowFeedback) => void): ReviewFeedback {
  return {
    toast: (toast: ReviewToast) => send({ kind: "toast", toast }),
    error: (title, error) => send({ kind: "error", title, message: recordError(title, error) }),
  }
}

/** Runs what the review window asks for, in the main window, and answers it. Main window only. */
export const runReviewWindowRequest = link.serve<ReviewWindowRequest, ReviewWindowFeedback>((request, send) => {
  const { repositoryPath } = request
  switch (request.action) {
    case "shown":
      link.setShown({ repositoryPath, number: request.request.number })
      noteReviewRefsWritten(repositoryPath)
      markReviewRequestRead(repositoryPath, request.request)
      broadcastReviewWindowState()
      return true
    case "openOnGitHub":
      openRequestOnGitHub(request.request)
      return true
    case "approve":
    case "requestChanges":
      return submitReview(repositoryPath, request.request, request.action, request.commitId, request.body, windowFeedback(send))
    case "safePush":
      return approveAndSafePush(repositoryPath, request.request, request.commitId, windowFeedback(send))
  }
})

/** Forgets the window (tests) */
export function resetReviewWindow(): void {
  link.reset()
  patronusTargets.clear()
  patronusReads.clear()
}
