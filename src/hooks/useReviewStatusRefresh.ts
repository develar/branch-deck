import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { setWindowFocus } from "@/lib/os-notification"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshPatronus, setPatronusWindowFocus } from "@/stores/patronus"
import { refreshPullRequests } from "@/stores/pull-requests"
import { refreshReviewRequests } from "@/stores/review-requests"

/**
 * Keeps the review status of the main window current: the GitHub pull requests, the review requests of others and the
 * Patronus robots, after every successful sync and on window focus (each store throttles both). A push refreshes the
 * pull requests itself. Running robots are polled every minute while the window is focused, a robot that pushes every
 * five minutes while it is not; the review requests every five minutes, focused or not.
 */
export function useReviewStatusRefresh(): void {
  useEffect(() => branchSyncStore.subscribe((state, previous) => {
    if (previous.isSyncing && !state.isSyncing && state.syncError === null) {
      void refreshPullRequests("sync")
      void refreshReviewRequests("sync")
      void refreshPatronus("sync")
    }
  }), [])

  useWindowFocus(() => {
    setWindowFocus(true)
    void refreshPullRequests("focus")
    void refreshReviewRequests("focus")
    void refreshPatronus("focus")
    setPatronusWindowFocus(true)
  }, () => {
    setWindowFocus(false)
    setPatronusWindowFocus(false)
  })
}
