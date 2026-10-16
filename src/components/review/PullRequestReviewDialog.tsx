import { useRef, useState } from "react"
import { PullRequestReview } from "@/components/review/PullRequestReview"
import type { PullRequestReviewHost } from "@/components/review/PullRequestReview"
import { PreviewDialogFrame } from "@/components/shared/PreviewDialogFrame"
import { useReviewPatronus } from "@/hooks/useReviewPatronus"
import { getReviewSafePush } from "@/lib/pull-request-review"
import { openPullRequestReviewWindow } from "@/lib/pull-request-review-window"
import { endReviewFetch } from "@/stores/review-fetch"
import {
  approveAndSafePush,
  closeReviewRequest,
  openRequestOnGitHub,
  openReviewRequest,
  submitReview,
  useOpenReview,
  useReviewAction,
  useReviewRequests,
  useSubmittedReview,
} from "@/stores/review-requests"

/**
 * The review of a pull request that waits for the user, as a dialog over the main window, opened from the Your Turn
 * card or the toast of a new request (`PullRequestReview`): the writes run here, with their toasts. Open in Window (W)
 * moves the review into its own window. Escape closes a form first, goes back from Changes to the overview, then
 * closes the dialog, which kills the fetch.
 *
 * Stays mounted after the first open (see `PreviewDialogFrame`).
 */
export function PullRequestReviewDialog() {
  const openReview = useOpenReview()
  // the last shown request stays while the closing animation runs
  const [shown, setShown] = useState(openReview)
  if (openReview !== null && openReview !== shown) {
    setShown(openReview)
  }
  const open = openReview !== null
  const requests = useReviewRequests()
  const live = shown === null ? undefined : requests.find(it => it.number === shown.number)
  // a request withdrawn while the dialog is open stays, with a notice
  const request = live ?? shown?.request
  const index = live === undefined ? -1 : requests.indexOf(live)
  const repositoryPath = shown?.repositoryPath ?? ""
  const submitted = useSubmittedReview(request?.number ?? -1)
  const busy = useReviewAction(request?.number ?? -1)
  const patronus = useReviewPatronus(open)
  const [reviewing, setReviewing] = useState(false)
  const bodyRef = useRef<HTMLDivElement>(null)
  const escapeRef = useRef<(() => boolean) | null>(null)

  const close = () => {
    endReviewFetch()
    closeReviewRequest()
  }

  const host: PullRequestReviewHost | null = request === undefined || shown === null
    ? null
    : {
        chrome: "dialog",
        open,
        repositoryPath,
        requests,
        request,
        index,
        openedIndex: shown.index,
        withdrawn: live === undefined && submitted === undefined,
        submitted,
        busy,
        safePush: getReviewSafePush(request, patronus),
        intent: shown.intent,
        view: null,
        openToken: shown,
        showRequest: next => openReviewRequest(repositoryPath, requests[next]!.number),
        close,
        openInWindow: (view) => {
          // the window keeps the refs of the request before the dialog's close prunes the others; it starts where the
          // dialog was
          void openPullRequestReviewWindow(repositoryPath, request, index === -1 ? shown.index : index, null, view)
          close()
        },
        openOnGitHub: () => openRequestOnGitHub(request),
        approve: commitId => submitReview(repositoryPath, request, "approve", commitId),
        requestChanges: (commitId, body) => submitReview(repositoryPath, request, "requestChanges", commitId, body),
        startSafePush: commitId => approveAndSafePush(repositoryPath, request, commitId),
      }

  return (
    <PreviewDialogFrame
      data-testid="review-dialog"
      open={open && request !== undefined}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // Escape closes the form of an action first (the dialog may see the key before the popover), then goes back
        // from the review of the changes to the overview
        if (eventDetails.reason === "escape-key" && escapeRef.current?.() === true) {
          eventDetails.cancel()
          return
        }
        close()
      }}
      initialFocus={bodyRef}
      width={reviewing && open ? "full" : "xl"}
      showCloseButton={false}
    >
      {host !== null && <PullRequestReview host={host} bodyRef={bodyRef} escapeRef={escapeRef} onReviewingChange={setReviewing} />}
    </PreviewDialogFrame>
  )
}
