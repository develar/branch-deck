import { useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useStore } from "zustand"
import { PullRequestReview } from "@/components/review/PullRequestReview"
import type { PullRequestReviewHost } from "@/components/review/PullRequestReview"
import { useWindowTitle } from "@/hooks/useWindowTitle"
import type { PullRequestReviewEvent, ReviewRequest, ReviewsView } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { ReviewFeedback } from "@/lib/pull-request-review"
import { showReviewToast } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import type { ReviewPlace, ShownReviewWindow } from "@/lib/review-reuse"
import { reviewWindowTitle } from "@/lib/review-reuse"
import { askReviewRun, PULL_REQUEST_READ, PULL_REQUEST_SAFE_PUSH } from "@/lib/review-runs"
import { showReviewWindowPullRequest } from "@/model/demand"
import { useRowPart } from "@/model/store"
import { useBranchSync } from "@/stores/branch-sync"
import { stopReviewFetch } from "@/stores/review-fetch"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"
import { getReviewBusy, reviewRequestsStore, submitReview } from "@/stores/review-requests"
import { reviewRowsOf, reviewsStore } from "@/stores/reviews"
import type { ReviewRows } from "@/stores/reviews"

/*
 * The review of a pull request in the review window (openspec/specs/repository/review-requests/spec.md#review-window), a chunk of
 * its own: the page loads it for a pull request only, so the first frame of the review of a branch or a commit keeps
 * its size. It reads the requests, Reviews, the actions Rust decides and the writes that run from the window's own
 * session, approves and requests changes itself; the Safe Push and the read mark of Your Turn are the main window's
 * (`review/run`).
 */

type ShownPullRequest = Extract<ShownReviewWindow, { kind: "pullRequest" }>

interface Shown {
  readonly request: ReviewRequest
  /** Its place in the list when it was shown */
  readonly index: number
  readonly intent: ReviewIntent | null
  /** A new value per shown request: its intent is offered once */
  readonly token: number
}

const NO_REQUESTS: readonly ReviewRequest[] = []
const NO_GROUPS: PullRequestReviewHost["reviewGroups"] = []

/** The rows of the list the window walks: Your Turn's, or those of Reviews with how the user takes part in each */
function useListRows(repositoryPath: string, source: ShownPullRequest["source"]): ReviewRows {
  const requests = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.requests ?? NO_REQUESTS)
  const sections = useStore(reviewsStore, state => state.repositories[repositoryPath]?.sections ?? NO_SECTIONS)
  return source === "yourTurn" ? { requests, involvements: NO_INVOLVEMENTS } : reviewRowsOf(requests, sections)
}

const NO_SECTIONS: ReviewsView["sections"] = []
const NO_INVOLVEMENTS: ReviewRows["involvements"] = {}

/** How a write the window runs ends: its toast and its error in the window */
const WINDOW_REVIEW_FEEDBACK: ReviewFeedback = { toast: showReviewToast, error: notifyError }

/**
 * The review of the pull request of `data`: ↑/↓ walk the list it came from, a move (an open of a request of that list)
 * shows that request; Back restores the request it showed (`onPlace`)
 */
export function PullRequestReviewWindow({ data, back, onPlace }: { data: ShownPullRequest; back: ReactNode; onPlace: (place: ReviewPlace) => void }) {
  const { repositoryPath, source } = data
  const [shown, setShown] = useState<Shown>({ request: data.request, index: data.index, intent: data.intent, token: data.openedAt })
  // a move: an open of a request of the list on screen shows it
  const [openedAt, setOpenedAt] = useState(data.openedAt)
  if (data.openedAt !== openedAt) {
    setOpenedAt(data.openedAt)
    if (shown.request.number !== data.number) {
      setShown({ request: data.request, index: data.index, intent: null, token: data.openedAt })
    }
  }
  const rows = useListRows(repositoryPath, source)
  // what runs and what the user submitted, until the session says it
  const [localBusy, setLocalBusy] = useState<Readonly<Record<number, ReviewAction>>>({})
  const [localReviewed, setLocalReviewed] = useState<Readonly<Record<number, SubmittedReview>>>({})
  const bodyRef = useRef<HTMLDivElement>(null)

  const live = rows.requests.find(it => it.number === shown.request.number)
  const request = live ?? shown.request
  const index = live === undefined ? -1 : rows.requests.indexOf(live)
  const number = request.number
  const reviewed = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.reviewed[number])
  const submitted = localReviewed[number] ?? reviewed
  const operations = useBranchSync(state => state.operations)
  const busy = localBusy[number] ?? getReviewBusy(operations)[number]
  const actions = useRowPart(`pull:${number}`, "actions")
  const reviewGroups = useRowPart(`pull:${number}`, "reviewGroups") ?? NO_GROUPS
  const baselineBranch = useBranchSync(state => state.baselineBranch)

  // the session keeps the review refs and the actions of the request the window shows, and asks Patronus of it
  useEffect(() => showReviewWindowPullRequest(number), [number])
  // the window leaves the pull request: its fetch stops, its range goes; a review of it again (Back) fetches it again
  useEffect(() => stopReviewFetch, [])
  // the focus in the page, so the keys work from the start
  useEffect(() => {
    bodyRef.current?.focus()
  }, [])
  useEffect(() => {
    onPlace({ kind: "pullRequest", request, index: index === -1 ? shown.index : index })
  })

  // Your Turn marks a request read once the user saw it: the main window keeps that mark
  const markRead = (target: ReviewRequest) => void askReviewRun({ repositoryPath, subject: `pull:${target.number}`, itemId: PULL_REQUEST_READ, request: target })
  const openOnGitHub = () => {
    markRead(request)
    openPullRequestUrl(request.url)
  }
  const run = (action: ReviewAction, write: () => Promise<boolean>, submittedReview: SubmittedReview | null): Promise<boolean> => {
    setLocalBusy(previous => ({ ...previous, [number]: action }))
    return write()
      .then(ok => {
        if (ok && submittedReview !== null) {
          setLocalReviewed(previous => ({ ...previous, [number]: submittedReview }))
        }
        return ok
      })
      .finally(() => setLocalBusy(previous => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== String(number)))))
  }
  const review = (event: PullRequestReviewEvent, commitId: string, body: string | null) =>
    run(event, () => submitReview(repositoryPath, request, event, commitId, body, WINDOW_REVIEW_FEEDBACK), { event, commitId })

  const host: PullRequestReviewHost = {
    repositoryPath,
    source,
    requests: rows.requests,
    involvement: rows.involvements[number] ?? null,
    request,
    index,
    openedIndex: shown.index,
    withdrawn: live === undefined && submitted === undefined,
    submitted,
    actions,
    reviewGroups,
    baselineBranch,
    busy,
    intent: shown.intent,
    openToken: shown.token,
    openedAt,
    showRequest: next => {
      const target = rows.requests[next]
      if (target === undefined) {
        return
      }
      setShown({ request: target, index: next, intent: null, token: Date.now() })
      markRead(target)
    },
    close: () => {
      getCurrentWindow()
        .close()
        .catch((error: unknown) => logError("Cannot close the review window", error))
    },
    openOnGitHub,
    approve: commitId => review("approve", commitId, null),
    requestChanges: (commitId, body) => review("requestChanges", commitId, body),
    // Patronus answers the comment of the Safe Push in the main window, which watches the robot
    startSafePush: commitId =>
      run("safePush", () => askReviewRun({ repositoryPath, subject: `pull:${number}`, itemId: PULL_REQUEST_SAFE_PUSH, commitIds: [commitId], request }), null),
  }

  // the native title names the request shown
  useWindowTitle(reviewWindowTitle({ ...data, number, request }))
  return <PullRequestReview host={host} bodyRef={bodyRef} back={back} />
}
