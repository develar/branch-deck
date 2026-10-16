import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { PULL_REQUEST_REVIEW_WINDOW } from "@/lib/pull-request-review"
import type { ReviewWindowRequest } from "@/lib/pull-request-review"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { broadcastReviewWindowState, runReviewWindowRequest } from "@/lib/pull-request-review-window"
import { patronusStore } from "@/stores/patronus"
import { repositoryStore } from "@/stores/repository"
import { reviewRequestsStore } from "@/stores/review-requests"

/**
 * The review window of a pull request, which doesn't own the requests: tells it the state of the requests of its
 * repository whenever the requests, Patronus or the selected repository change, and runs the writes it asks for
 * (`pull-request-review/request`). Main window only.
 */
export function useReviewWindowBridge(): void {
  useEffect(() => {
    const unsubscribes = [reviewRequestsStore, patronusStore, repositoryStore].map(store => store.subscribe(broadcastReviewWindowState))
    return () => {
      for (const unsubscribe of unsubscribes) {
        unsubscribe()
      }
    }
  }, [])
  useTauriListen<ReviewWindowRequest>(windowLinkEvents(PULL_REQUEST_REVIEW_WINDOW).request, runReviewWindowRequest)
}
