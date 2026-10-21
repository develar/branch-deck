import { useEffect } from "react"
import { isAwaitingApproval } from "@/hooks/useBranchNextStep"
import { isStackAwaitingApproval, readinessKeysNeedingAnswer, tellReadinessNews } from "@/hooks/useStackNextStep"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { setWindowFocus } from "@/lib/os-notification"
import { watchFailingRobots } from "@/lib/patronus-failing"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshGitHubAuthStatus } from "@/stores/github-auth"
import { refreshPatronus, setPatronusWindowFocus, watchRepositoryView } from "@/stores/patronus"
import { refreshPullRequests } from "@/stores/pull-requests"
import { hasAwaitingRow, watchReviewReadiness } from "@/stores/review-readiness"
import { onReviewRequestsPoll, refreshReviewRequests } from "@/stores/review-requests"
import { watchRobotCoverage } from "@/stores/robot-coverage"

/** A row or the header of a whole stack waits for an approval (Awaiting) */
function isAwaiting(): boolean {
  return hasAwaitingRow(isAwaitingApproval) || isStackAwaitingApproval()
}

/**
 * Keeps the review status of the main window current: the GitHub pull requests, the review requests of others and the
 * Patronus robots, after every successful sync and on window focus (each store throttles both). A push refreshes the
 * pull requests itself. Running robots are polled every minute while the window is focused, a robot that pushes every
 * five minutes while it is not; a running robot that will fail and a robot that failed are told once
 * (`watchFailingRobots`); what a merged robot landed is read by content when a change below its commit gave the
 * branches new copies (`watchRepositoryView`); which commits a running robot covers, once its branch changed since it
 * started (`watchRobotCoverage`); the review requests
 * every five minutes, focused or not. A sign-in or a sign-out of
 * GitHub (`github-auth-changed`, from any window) asks GitHub again at once. The readiness of the branches whose next
 * step depends on it is asked as they change (`watchReviewReadiness`), the whole stack of an island header too; while a
 * row or such a header waits for an approval (Awaiting), a
 * focus asks GitHub for the pull requests at once (the user comes back from asking for it), and so does every poll of
 * the review requests; an approval that arrives is news (`tellReadinessNews`).
 */
export function useReviewStatusRefresh(): void {
  useEffect(() => watchFailingRobots(), [])
  useEffect(() => watchRepositoryView(), [])
  useEffect(() => watchRobotCoverage(), [])
  useEffect(() => watchReviewReadiness(readinessKeysNeedingAnswer, tellReadinessNews), [])
  // a row waits for an approval: each poll of the review requests asks GitHub for the user's pull requests too, whose
  // new reviews make the readiness ask Patronus once
  useEffect(
    () =>
      onReviewRequestsPoll(() => {
        if (isAwaiting()) {
          void refreshPullRequests("manual")
        }
      }),
    [],
  )

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
      void refreshPullRequests(isAwaiting() ? "manual" : "focus")
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
