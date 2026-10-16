import { useEffect } from "react"
import { emit } from "@tauri-apps/api/event"
import { BRANCH_TIPS_CHANGED, branchTips } from "@/lib/branch-review"
import type { BranchTipsChanged } from "@/lib/branch-review"
import { logError } from "@/lib/errors"
import { sameStringRecord } from "@/lib/record"
import { branchSyncStore } from "@/stores/branch-sync"

/**
 * Tells the review window that a sync moved the tips of the branches (`branch-tips-changed`, a broadcast: the main
 * window doesn't track whether the window is open); the window reads its stack again when a tip of it moved. Main
 * window only.
 */
export function useBranchReviewNotifier(): void {
  useEffect(() => {
    let sent: Record<string, string> = {}
    return branchSyncStore.subscribe((state, previous) => {
      const repositoryPath = state.syncedRepositoryPath
      if (state.syncCount === previous.syncCount || repositoryPath === null) {
        return
      }
      const tips = branchTips(state.branches)
      if (sameStringRecord(tips, sent)) {
        return
      }
      sent = tips
      const payload: BranchTipsChanged = { repositoryPath, tips }
      emit(BRANCH_TIPS_CHANGED, payload).catch((error: unknown) => logError("Cannot tell the review window about the sync", error))
    })
  }, [])
}
