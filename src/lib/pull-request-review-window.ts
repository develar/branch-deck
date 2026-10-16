import { emit, listen } from "@tauri-apps/api/event"
import type { ReviewRequest } from "@/lib/bindings"
import { logError, notifyError, recordError } from "@/lib/errors"
import { PULL_REQUEST_REVIEW_WINDOW, REVIEW_WINDOW_FEEDBACK, REVIEW_WINDOW_REPLY, REVIEW_WINDOW_STATE_CHANGED, reviewWindowTitle } from "@/lib/pull-request-review"
import type { PullRequestReviewWindowData, ReviewFeedback, ReviewPatronus, ReviewToast, ReviewWindowFeedback, ReviewWindowReply, ReviewWindowRequest, ReviewWindowState } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { openSubWindow } from "@/lib/window-management"
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
let windowReview: { repositoryPath: string, number: number } | null = null
/** The state last sent to the window, as JSON: a change of nothing sends nothing */
let sentState: string | null = null
/** The branch Patronus pushes into, by repository, once read */
const patronusTargets = new Map<string, string | null>()
/** The reads of `patronusTargets` in flight */
const patronusReads = new Map<string, Promise<void>>()

/** The review window is open on a request of `repositoryPath`: Your Turn opens its rows there */
export function isReviewWindowOpenFor(repositoryPath: string): boolean {
  return windowReview?.repositoryPath === repositoryPath
}

/** The number of the request the review window shows in `repositoryPath`, if any: its refs stay */
export function reviewWindowNumber(repositoryPath: string): number | null {
  return windowReview?.repositoryPath === repositoryPath ? windowReview.number : null
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
  const wasOpen = windowReview !== null
  windowReview = { repositoryPath, number: request.number }
  // the window fetches it: the next prune keeps its refs, deletes those of the request it showed before
  noteReviewRefsWritten(repositoryPath)
  markReviewRequestRead(repositoryPath, request)
  // the window opens knowing whether Patronus merges the request
  await readPatronusTarget(repositoryPath)
  const state = buildState(repositoryPath)
  sentState = JSON.stringify(state)
  const data: PullRequestReviewWindowData = { repositoryPath, number: request.number, request, index, intent, openedAt: Date.now(), state }
  try {
    await openSubWindow({ windowId: PULL_REQUEST_REVIEW_WINDOW, url: "/pull-request-review", title: reviewWindowTitle(request), data })
  }
  catch (error) {
    windowReview = null
    notifyError(`Cannot open the review of #${request.number}`, error)
    return
  }
  if (!wasOpen) {
    void listenForClose()
  }
}

async function listenForClose(): Promise<void> {
  const unlisten = await listen("tauri://destroyed", () => {
    unlisten()
    const closed = windowReview
    windowReview = null
    sentState = null
    if (closed !== null) {
      // its refs go with it
      pruneReviewRefsOf(closed.repositoryPath)
    }
  }, { target: { kind: "WebviewWindow", label: PULL_REQUEST_REVIEW_WINDOW } })
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
 * Tells the review window the state of the requests of its repository when it changed (`review-window-state-changed`):
 * a reviewed request, a write that runs, a poll, the main window showing another repository. Main window only.
 */
export function broadcastReviewWindowState(): void {
  const shown = windowReview
  if (shown === null) {
    return
  }
  const state = buildState(shown.repositoryPath)
  const json = JSON.stringify(state)
  if (json === sentState) {
    return
  }
  sentState = json
  emit(REVIEW_WINDOW_STATE_CHANGED, state).catch((error: unknown) => logError("Cannot tell the review window about the requests", error))
}

/** The feedback of a write the window asked for: it shows the toast; the main window records the error in Activity */
function windowFeedback(): ReviewFeedback {
  const send = (feedback: ReviewWindowFeedback) => emit(REVIEW_WINDOW_FEEDBACK, feedback).catch((error: unknown) => logError("Cannot tell the review window", error))
  return {
    toast: (toast: ReviewToast) => void send({ kind: "toast", toast }),
    error: (title, error) => void send({ kind: "error", title, message: recordError(title, error) }),
  }
}

/** Runs what the review window asks for, in the main window, and answers it (`review-window-reply`). Main window only. */
export function runReviewWindowRequest(request: ReviewWindowRequest): void {
  const reply = (ok: boolean) => {
    const answer: ReviewWindowReply = { id: request.id, ok }
    emit(REVIEW_WINDOW_REPLY, answer).catch((error: unknown) => logError("Cannot answer the review window", error))
  }
  const { repositoryPath } = request
  switch (request.action) {
    case "shown":
      windowReview = { repositoryPath, number: request.request.number }
      noteReviewRefsWritten(repositoryPath)
      markReviewRequestRead(repositoryPath, request.request)
      broadcastReviewWindowState()
      reply(true)
      return
    case "openOnGitHub":
      openRequestOnGitHub(request.request)
      reply(true)
      return
    case "approve":
    case "requestChanges":
      void submitReview(repositoryPath, request.request, request.action, request.commitId, request.body, windowFeedback()).then(reply)
      return
    case "safePush":
      void approveAndSafePush(repositoryPath, request.request, request.commitId, windowFeedback()).then(reply)
  }
}

/** Forgets the window (tests) */
export function resetReviewWindow(): void {
  windowReview = null
  sentState = null
  patronusTargets.clear()
  patronusReads.clear()
}
