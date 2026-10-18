import { useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { PullRequestReview } from "@/components/review/PullRequestReview"
import type { PullRequestReviewHost } from "@/components/review/PullRequestReview"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { useWindowTitle } from "@/hooks/useWindowTitle"
import type { PullRequestReviewEvent, ReviewRequest } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { openRobotUrl } from "@/lib/patronus"
import { PULL_REQUEST_REVIEW_WINDOW, getReviewSafePush, pullRequestReviewWindowKey, reviewWindowTitle } from "@/lib/pull-request-review"
import type { PullRequestReviewWindowData, ReviewWindowFeedback, ReviewWindowRequest } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { showToast } from "@/lib/toast-message"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/**
 * The review window of a pull request (`pull-request-review`), the only review of a pull request. The main window owns
 * the requests: over the link of the window (`@/lib/sub-window-link`) it sends their state,
 * runs the writes the window asks for and sends back how they went, which the window shows.
 */
export function PullRequestReviewPage() {
  const data = useSubWindowData<PullRequestReviewWindowData>()
  if (data === null) {
    return null
  }
  return <PullRequestReviewWindow key={pullRequestReviewWindowKey(data)} data={data} />
}

/** Asks the main window for a write or tells it a notice (`pull-request-review/request`) */
function ask(request: Parameters<typeof askMainWindow<ReviewWindowRequest>>[1]): Promise<boolean> {
  return askMainWindow<ReviewWindowRequest>(PULL_REQUEST_REVIEW_WINDOW, request)
}

interface Shown {
  request: ReviewRequest
  /** Its place in the list when it was shown */
  index: number
  intent: ReviewIntent | null
  /** A new value per shown request: its intent is offered once */
  token: number
}

function PullRequestReviewWindow({ data }: { data: PullRequestReviewWindowData }) {
  const { repositoryPath } = data
  const state = useLinkedState(PULL_REQUEST_REVIEW_WINDOW, data.state)
  const [shown, setShown] = useState<Shown>({ request: data.request, index: data.index, intent: data.intent, token: data.openedAt })
  // a plain open of the request the window was opened on, after ↑/↓ showed another one: that request again
  const [openedAt, setOpenedAt] = useState(data.openedAt)
  if (data.openedAt !== openedAt) {
    setOpenedAt(data.openedAt)
    if (shown.request.number !== data.number) {
      setShown({ request: data.request, index: data.index, intent: null, token: data.openedAt })
    }
  }
  // what runs and what the user submitted, until the main window's state says it
  const [localBusy, setLocalBusy] = useState<Readonly<Record<number, ReviewAction>>>({})
  const [localReviewed, setLocalReviewed] = useState<Readonly<Record<number, SubmittedReview>>>({})
  // Patronus as the main window showed it for this repository: while it shows another one, Safe Push stays (its click
  // switches the main window to this repository first)
  const [seenPatronus, setSeenPatronus] = useState(data.state.patronus)
  if (state.selected && state.patronus !== seenPatronus) {
    setSeenPatronus(state.patronus)
  }
  const bodyRef = useRef<HTMLDivElement>(null)

  const live = state.requests.find(it => it.number === shown.request.number)
  const request = live ?? shown.request
  const index = live === undefined ? -1 : state.requests.indexOf(live)
  const number = request.number
  const submitted = state.reviewed[number] ?? localReviewed[number]
  const busy = localBusy[number] ?? state.busy[number]

  const openOnGitHub = () => void ask({ action: "openOnGitHub", repositoryPath, request })
  useLinkedFeedback<ReviewWindowFeedback>(PULL_REQUEST_REVIEW_WINDOW, feedback => showFeedback(feedback, openOnGitHub))

  // the focus in the page, so the keys work from the start
  useEffect(() => {
    bodyRef.current?.focus()
  }, [])

  const run = (action: ReviewAction, ask: () => Promise<boolean>, reviewed: SubmittedReview | null): Promise<boolean> => {
    setLocalBusy(previous => ({ ...previous, [number]: action }))
    return ask()
      .then(ok => {
        if (ok && reviewed !== null) {
          setLocalReviewed(previous => ({ ...previous, [number]: reviewed }))
        }
        return ok
      })
      .finally(() => setLocalBusy(previous => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== String(number)))))
  }
  const review = (event: PullRequestReviewEvent, commitId: string, body: string | null) =>
    run(event, () => ask({ action: event, repositoryPath, request, commitId, body }), { event, commitId })

  const safePush = getReviewSafePush(request, state.selected ? state.patronus : seenPatronus)

  const host: PullRequestReviewHost = {
    repositoryPath,
    requests: state.requests,
    request,
    index,
    openedIndex: shown.index,
    withdrawn: live === undefined && submitted === undefined,
    submitted,
    busy,
    safePush,
    intent: shown.intent,
    openToken: shown.token,
    showRequest: next => {
      const target = state.requests[next]
      if (target === undefined) {
        return
      }
      setShown({ request: target, index: next, intent: null, token: Date.now() })
      void ask({ action: "shown", repositoryPath, request: target })
    },
    close: () => {
      getCurrentWindow()
        .close()
        .catch((error: unknown) => logError("Cannot close the review window", error))
    },
    openOnGitHub,
    approve: commitId => review("approve", commitId, null),
    requestChanges: (commitId, body) => review("requestChanges", commitId, body),
    startSafePush: commitId => run("safePush", () => ask({ action: "safePush", repositoryPath, request, commitId, body: null }), null),
  }

  // the native title names the request shown
  useWindowTitle(reviewWindowTitle(shown.request))
  return <PullRequestReview host={host} bodyRef={bodyRef} />
}

/** The toast of a write the main window ran for the window, or its error (recorded in Activity by the main window) */
function showFeedback(feedback: ReviewWindowFeedback, openOnGitHub: () => void): void {
  if (feedback.kind === "error") {
    notifyError(feedback.title, feedback.message)
    return
  }
  const { title, description, target } = feedback.toast
  showToast({
    title,
    description,
    action: target.kind === "robot" ? { children: "Open in Patronus", onClick: () => openRobotUrl(target.url) } : { children: "Open Pull Request", onClick: openOnGitHub },
  })
}
