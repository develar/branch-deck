import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { REVIEW_RUN_EVENTS, runReviewRequest } from "@/lib/review-runs"
import type { ReviewRunRequest } from "@/lib/review-runs"
import { openPullRequestReview } from "@/lib/review-window"
import { connectReviewWindow } from "@/stores/review-requests"
import { reviewRows } from "@/stores/reviews"

/**
 * The main window's side of the review window: a request of a row of Your Turn or Reviews opens it on the pull request
 * (`connectReviewWindow`), and it runs what the window asks for at the rows of their subjects (`review/run`,
 * `runReviewRequest`): the forms of a row, the Safe Push and the read of a pull request. The window builds and runs every
 * other action itself. Main window only.
 */
export function useReviewRuns(): void {
  useEffect(() => {
    connectReviewWindow({
      open: (repositoryPath, request, index, intent, source) => void openPullRequestReview(repositoryPath, request, index, intent, source),
      rowsOf: repositoryPath => reviewRows(repositoryPath).requests,
    })
  }, [])
  useTauriListen<ReviewRunRequest>(REVIEW_RUN_EVENTS.request, runReviewRequest)
}
