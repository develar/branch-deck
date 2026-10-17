import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { BRANCH_REVIEW_WINDOW, branchTips } from "@/lib/branch-review"
import { broadcastBranchReviewState, noteSyncedTips, runBranchReviewRequest } from "@/lib/branch-review-window"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { branchSyncStore } from "@/stores/branch-sync"
import { inlineRowActionStore } from "@/stores/inline-row-action"
import { landedStore } from "@/stores/landed"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { repositoryStore } from "@/stores/repository"
import { reviewReadinessStore } from "@/stores/review-readiness"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"
import type { BranchReviewRequest } from "@/types/sub-window"

/**
 * The review window of a branch, which doesn't own the branches: tells it the heads a sync built and what it can do with
 * each branch of its stack whenever the branches, their pull requests, the Patronus robots, the review readiness, the
 * landed commits, the inline action of a row or the selected repository change, and runs what it asks for. Main window only.
 */
export function useBranchReviewLink(): void {
  useEffect(() => {
    // the heads of a finished sync, not of one that runs: the window reads its stack again once per sync
    const unsubscribeSync = branchSyncStore.subscribe((state, previous) => {
      if (state.syncCount !== previous.syncCount && state.syncedRepositoryPath !== null) {
        noteSyncedTips(state.syncedRepositoryPath, branchTips(state.branches))
      }
    })
    // the readiness and the landed commits decide the next step of a branch, which disables its Safe Push…
    const stores = [branchSyncStore, pullRequestsStore, patronusStore, inlineRowActionStore, repositoryStore, repositorySettingsStore, reviewReadinessStore, landedStore]
    const unsubscribes = stores.map(store => store.subscribe(broadcastBranchReviewState))
    return () => {
      unsubscribeSync()
      for (const unsubscribe of unsubscribes) {
        unsubscribe()
      }
    }
  }, [])
  useTauriListen<BranchReviewRequest>(windowLinkEvents(BRANCH_REVIEW_WINDOW).request, runBranchReviewRequest)
}
