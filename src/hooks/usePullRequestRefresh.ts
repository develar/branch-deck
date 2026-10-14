import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshPullRequests } from "@/stores/pull-requests"

/**
 * Keeps the pull requests of the main window current without polling: after every successful sync, and on window
 * focus when they are older than two minutes (the store throttles both). A push refreshes them itself.
 */
export function usePullRequestRefresh(): void {
  useEffect(() => branchSyncStore.subscribe((state, previous) => {
    if (previous.isSyncing && !state.isSyncing && state.syncError === null) {
      void refreshPullRequests("sync")
    }
  }), [])

  useWindowFocus(() => {
    void refreshPullRequests("focus")
  })
}
