import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { broadcastBranchReviewState, runBranchReviewRequest } from "@/lib/branch-review-window"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { BRANCH_REVIEW_WINDOW } from "@/lib/window-label"
import { branchSyncStore } from "@/stores/branch-sync"
import { indicatorsStore } from "@/stores/indicators"
import { landedStore } from "@/stores/landed"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { repositoryStore } from "@/stores/repository"
import { reviewReadinessStore } from "@/stores/review-readiness"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"
import type { BranchReviewRequest } from "@/types/sub-window"

/**
 * The review window of a branch, which doesn't own the branches: tells it what it can do with each branch of its stack
 * whenever the branches, their pull requests, the Patronus robots, the review readiness, the landed commits, the
 * operations of the session (the view's, the busy states this window gave them) or the selected repository change, and runs what it asks for. The heads a sync built the window
 * reads from its own subscription to the session of its repository. Main window only.
 */
export function useBranchReviewLink(): void {
  useEffect(() => {
    // the readiness and the landed commits decide the next step of a branch, which disables its Safe Push
    const stores = [branchSyncStore, pullRequestsStore, patronusStore, indicatorsStore, repositoryStore, repositorySettingsStore, reviewReadinessStore, landedStore]
    const unsubscribes = stores.map(store => store.subscribe(broadcastBranchReviewState))
    return () => {
      for (const unsubscribe of unsubscribes) {
        unsubscribe()
      }
    }
  }, [])
  useTauriListen<BranchReviewRequest>(windowLinkEvents(BRANCH_REVIEW_WINDOW).request, runBranchReviewRequest)
}
