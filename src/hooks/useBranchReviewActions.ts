import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { BRANCH_ACTION } from "@/lib/branch-review"
import type { BranchActionRequest } from "@/lib/branch-review"
import { broadcastBranchReviewActions, runBranchAction } from "@/lib/branch-review-window"
import { branchSyncStore } from "@/stores/branch-sync"
import { inlineRowActionStore } from "@/stores/inline-row-action"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { repositoryStore } from "@/stores/repository"

/**
 * The actions of the review window, which doesn't own the branches: tells it what it can do whenever the branches, their
 * pull requests, the Patronus robots or the inline action of a row change, and runs what it asks for (`branch-action`).
 * Main window only.
 */
export function useBranchReviewActions(): void {
  useEffect(() => {
    const stores = [branchSyncStore, pullRequestsStore, patronusStore, inlineRowActionStore, repositoryStore]
    const unsubscribes = stores.map(store => store.subscribe(broadcastBranchReviewActions))
    return () => {
      for (const unsubscribe of unsubscribes) {
        unsubscribe()
      }
    }
  }, [])
  useTauriListen<BranchActionRequest>(BRANCH_ACTION, runBranchAction)
}
