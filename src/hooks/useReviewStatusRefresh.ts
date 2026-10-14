import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshPatronus, setPatronusWindowFocus } from "@/stores/patronus"
import { refreshPullRequests } from "@/stores/pull-requests"

/**
 * Keeps the review status of the main window current: the GitHub pull requests and the Patronus robots, after every
 * successful sync and on window focus (each store throttles both). A push refreshes the pull requests itself. Running
 * robots are polled every minute while the window is focused, a robot that pushes every five minutes while it is not.
 */
export function useReviewStatusRefresh(): void {
  useEffect(() => branchSyncStore.subscribe((state, previous) => {
    if (previous.isSyncing && !state.isSyncing && state.syncError === null) {
      void refreshPullRequests("sync")
      void refreshPatronus("sync")
    }
  }), [])

  useWindowFocus(() => {
    void refreshPullRequests("focus")
    void refreshPatronus("focus")
    setPatronusWindowFocus(true)
  }, () => setPatronusWindowFocus(false))
}
