import { useEffect, useRef, useState } from "react"
import { emit } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { PullRequestReview } from "@/components/review/PullRequestReview"
import type { PullRequestReviewHost } from "@/components/review/PullRequestReview"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { PullRequestReviewEvent, ReviewRequest } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { openRobotUrl } from "@/lib/patronus"
import { REVIEW_WINDOW_FEEDBACK, REVIEW_WINDOW_REPLY, REVIEW_WINDOW_REQUEST, REVIEW_WINDOW_STATE_CHANGED, getReviewSafePush, reviewWindowTitle } from "@/lib/pull-request-review"
import type { PullRequestReviewWindowData, ReviewWindowFeedback, ReviewWindowReply, ReviewWindowRequest, ReviewWindowState } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { showToast } from "@/lib/toast-message"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"
import { otherRepositoryReason } from "@/stores/review-requests"

/**
 * The review window of a pull request (`pull-request-review`): the review of the dialog on the canvas of a window of its
 * own. The main window owns the requests: it sends their state (`review-window-state-changed`), runs the writes the
 * window asks for (`review-window-request`) and sends back how they went, which the window shows.
 */
export function PullRequestReviewPage() {
  const data = useSubWindowData<PullRequestReviewWindowData>()
  if (data === null) {
    return null
  }
  // a new open (another request, or a row clicked while the window is open) starts over
  return <PullRequestReviewWindow key={data.openedAt} data={data} />
}

/** The answers the window waits for, by the id of its request */
const replies = new Map<string, (ok: boolean) => void>()
let nextRequestId = 0

/** Asks the main window; resolves to whether it did it (a refusal was told by its feedback). No timeout: Patronus may take a minute. */
function askMainWindow(request: DistributiveOmit<ReviewWindowRequest, "id">): Promise<boolean> {
  const id = `${Date.now()}-${nextRequestId++}`
  return new Promise((resolve) => {
    replies.set(id, resolve)
    emit(REVIEW_WINDOW_REQUEST, { ...request, id }).catch((error: unknown) => {
      replies.delete(id)
      notifyError("Cannot reach the main window", error)
      resolve(false)
    })
  })
}

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never

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
  const [state, setState] = useState<ReviewWindowState>(data.state)
  const [shown, setShown] = useState<Shown>({ request: data.request, index: data.index, intent: data.intent, token: data.openedAt })
  // what runs and what the user submitted, until the main window's state says it
  const [localBusy, setLocalBusy] = useState<Readonly<Record<number, ReviewAction>>>({})
  const [localReviewed, setLocalReviewed] = useState<Readonly<Record<number, SubmittedReview>>>({})
  // Patronus merged here before: while the main window shows another repository, Safe Push says why it waits
  const [patronusSeen, setPatronusSeen] = useState(data.state.patronus.available)
  if (state.patronus.available && !patronusSeen) {
    setPatronusSeen(true)
  }
  const bodyRef = useRef<HTMLDivElement>(null)

  useTauriListen<ReviewWindowState>(REVIEW_WINDOW_STATE_CHANGED, (payload) => {
    if (payload.repositoryPath === repositoryPath) {
      setState(payload)
    }
  })
  useTauriListen<ReviewWindowReply>(REVIEW_WINDOW_REPLY, (reply) => {
    replies.get(reply.id)?.(reply.ok)
    replies.delete(reply.id)
  })

  const live = state.requests.find(it => it.number === shown.request.number)
  const request = live ?? shown.request
  const index = live === undefined ? -1 : state.requests.indexOf(live)
  const number = request.number
  const submitted = state.reviewed[number] ?? localReviewed[number]
  const busy = localBusy[number] ?? state.busy[number]

  const openOnGitHub = () => void askMainWindow({ action: "openOnGitHub", repositoryPath, request })
  useTauriListen<ReviewWindowFeedback>(REVIEW_WINDOW_FEEDBACK, feedback => showFeedback(feedback, openOnGitHub))

  // the focus in the page, so the keys work from the start
  useEffect(() => {
    bodyRef.current?.focus()
  }, [])

  const run = (action: ReviewAction, ask: () => Promise<boolean>, reviewed: SubmittedReview | null): Promise<boolean> => {
    setLocalBusy(previous => ({ ...previous, [number]: action }))
    return ask().then((ok) => {
      if (ok && reviewed !== null) {
        setLocalReviewed(previous => ({ ...previous, [number]: reviewed }))
      }
      return ok
    }).finally(() => setLocalBusy(previous => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== String(number)))))
  }
  const review = (event: PullRequestReviewEvent, commitId: string, body: string | null) =>
    run(event, () => askMainWindow({ action: event, repositoryPath, request, commitId, body }), { event, commitId })

  const safePush = state.selected || !patronusSeen
    ? getReviewSafePush(request, state.patronus)
    : { blocked: otherRepositoryReason(repositoryPath) }

  const host: PullRequestReviewHost = {
    chrome: "window",
    open: true,
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
    showRequest: (next) => {
      const target = state.requests[next]
      if (target === undefined) {
        return
      }
      setShown({ request: target, index: next, intent: null, token: Date.now() })
      void askMainWindow({ action: "shown", repositoryPath, request: target })
      getCurrentWindow().setTitle(reviewWindowTitle(target)).catch((error: unknown) => logError("Cannot retitle the review window", error))
    },
    close: () => {
      getCurrentWindow().close().catch((error: unknown) => logError("Cannot close the review window", error))
    },
    openInWindow: null,
    openOnGitHub,
    approve: commitId => review("approve", commitId, null),
    requestChanges: (commitId, body) => review("requestChanges", commitId, body),
    startSafePush: commitId => run("safePush", () => askMainWindow({ action: "safePush", repositoryPath, request, commitId, body: null }), null),
  }

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
    action: target.kind === "robot"
      ? { children: "Open in Patronus", onClick: () => openRobotUrl(target.url) }
      : { children: "Open Pull Request", onClick: openOnGitHub },
  })
}
