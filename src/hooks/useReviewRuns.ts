import { useTauriListen } from "@/hooks/useTauriListen"
import { REVIEW_RUN_EVENTS, runReviewRequest } from "@/lib/review-runs"
import type { ReviewRunRequest } from "@/lib/review-runs"

/**
 * Runs the actions the review window asks for at the rows of their subjects (`review/run`, `runReviewRequest`): those
 * that open a form under a row. The window builds and runs every other action itself. Main window only.
 */
export function useReviewRuns(): void {
  useTauriListen<ReviewRunRequest>(REVIEW_RUN_EVENTS.request, runReviewRequest)
}
