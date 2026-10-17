import { useEffect } from "react"
import { branchesNeedingReadiness, isAwaitingApproval } from "@/hooks/useBranchNextStep"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { setWindowFocus } from "@/lib/os-notification"
import { watchFailingRobots } from "@/lib/patronus-failing"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshGitHubAuthStatus } from "@/stores/github-auth"
import { refreshPatronus, setPatronusWindowFocus } from "@/stores/patronus"
import { refreshPullRequests } from "@/stores/pull-requests"
import { hasAwaitingRow, watchReviewReadiness } from "@/stores/review-readiness"
import { refreshReviewRequests } from "@/stores/review-requests"

/**
 * Keeps the review status of the main window current: the GitHub pull requests, the review requests of others and the
 * Patronus robots, after every successful sync and on window focus (each store throttles both). A push refreshes the
 * pull requests itself. Running robots are polled every minute while the window is focused, a robot that pushes every
 * five minutes while it is not; a running robot that will fail and a robot that failed are told once
 * (`watchFailingRobots`); the review requests
 * every five minutes, focused or not. A sign-in or a sign-out of
 * GitHub (`github-auth-changed`, from any window) asks GitHub again at once. The readiness of the branches whose next
 * step depends on it is asked as they change (`watchReviewReadiness`); while a row waits for an approval (Awaiting), a
 * focus asks GitHub for the pull requests at once: the user comes back from asking for it.
 */
export function useReviewStatusRefresh(): void {
  useEffect(() => watchFailingRobots(), [])
  useEffect(() => watchReviewReadiness(branchesNeedingReadiness), [])

  useEffect(
    () =>
      branchSyncStore.subscribe((state, previous) => {
        if (previous.isSyncing && !state.isSyncing && state.syncError === null) {
          void refreshPullRequests("sync")
          void refreshReviewRequests("sync")
          void refreshPatronus("sync")
        }
      }),
    [],
  )

  useTauriListen("github-auth-changed", () => {
    void refreshGitHubAuthStatus()
    void refreshPullRequests("manual")
    void refreshReviewRequests("manual")
  })

  useWindowFocus(
    () => {
      setWindowFocus(true)
      void refreshPullRequests(hasAwaitingRow(isAwaitingApproval) ? "manual" : "focus")
      void refreshReviewRequests("focus")
      void refreshPatronus("focus")
      setPatronusWindowFocus(true)
    },
    () => {
      setWindowFocus(false)
      setPatronusWindowFocus(false)
    },
  )
}
