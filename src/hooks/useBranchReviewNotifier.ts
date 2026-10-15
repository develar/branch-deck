import { useEffect } from "react"
import { emit } from "@tauri-apps/api/event"
import { BRANCH_TIPS_CHANGED, branchTips } from "@/lib/branch-review"
import type { BranchTipsChanged } from "@/lib/branch-review"
import { logError } from "@/lib/errors"
import { branchSyncStore } from "@/stores/branch-sync"

function sameTips(a: Readonly<Record<string, string>>, b: Readonly<Record<string, string>>): boolean {
  const keys = Object.keys(a)
  return keys.length === Object.keys(b).length && keys.every(key => a[key] === b[key])
}

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
      if (sameTips(tips, sent)) {
        return
      }
      sent = tips
      const payload: BranchTipsChanged = { repositoryPath, tips }
      emit(BRANCH_TIPS_CHANGED, payload).catch((error: unknown) => logError("Cannot tell the review window about the sync", error))
    })
  }, [])
}
