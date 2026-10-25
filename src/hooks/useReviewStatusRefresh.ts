import { useEffect } from "react"
import { isAwaitingApproval } from "@/hooks/useBranchNextStep"
import { isStackAwaitingApproval, readinessKeysNeedingAnswer, tellReadinessNews } from "@/hooks/useStackNextStep"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { approvalNewsOf } from "@/lib/approval-news"
import { isWindowFocused, setWindowFocus } from "@/lib/os-notification"
import { tellRobotNews, watchFailingRobots } from "@/lib/patronus-failing"
import { setDemandFocused } from "@/model/demand"
import { onSessionFacts } from "@/model/store"
import { branchSyncStore } from "@/stores/branch-sync"
import { refreshGitHubAuthStatus } from "@/stores/github-auth"
import { applySessionFacts } from "@/stores/patronus"
import { applyPullRequestsFacts, refreshPullRequests } from "@/stores/pull-requests"
import { applyReadinessFacts, hasAwaitingRow, watchReviewReadiness } from "@/stores/review-readiness"
import { onReviewRequestsPoll, refreshReviewRequests } from "@/stores/review-requests"
import { watchRobotCoverage } from "@/stores/robot-coverage"

/** A row or the header of a whole stack waits for an approval (Awaiting) */
function isAwaiting(): boolean {
  return hasAwaitingRow(isAwaitingApproval) || isStackAwaitingApproval()
}

/**
 * Keeps the review status of the main window current. The session of the repository lists the GitHub pull requests
 * (after a sync, on a focus, after a push and a write of the app, by its rules) and lists and polls the robots of
 * Patronus itself (`remote_facts::scheduler`, by the window's focus, which it tells from the start, and its demand); the
 * stores take them from its view (`applyPullRequestsFacts`, `applySessionFacts`) with the view the session derives of
 * the robots (what they land, landed and hold, the landing needs), the rules of the bases of the approved pull requests
 * and the merge profiles; a running robot that will fail (`watchFailingRobots`), a robot that merged or failed (the news
 * of the session, `tellRobotNews`) are told once; which commits a running robot covers, once its branch changed since
 * it started (`watchRobotCoverage`). The review requests of others after every successful sync, on window focus and
 * every five minutes, focused or not. A sign-in or a sign-out of GitHub (`github-auth-changed`, from any window) asks
 * GitHub again at once. The readiness of the branches whose next step depends on it is asked as they change
 * (`watchReviewReadiness` tells the session which shown subjects wait for it: the session asks them), the whole stack of an
 * island header too; while a row or such a header waits for an approval (Awaiting), a focus asks the session for the
 * pull requests at once (the user comes back from asking for it), and so does every poll of the review requests; the
 * approvals that arrived are news of the session (`tellReadinessNews`).
 */
export function useReviewStatusRefresh(): void {
  useEffect(
    () =>
      onSessionFacts((repositoryPath, view, changes, news) => {
        applySessionFacts(repositoryPath, view, changes)
        applyPullRequestsFacts(repositoryPath, view.facts, changes?.facts ?? null)
        applyReadinessFacts(repositoryPath, view.facts, changes?.facts ?? null)
        tellRobotNews(repositoryPath, news)
        approvalNewsOf(news).forEach(tellReadinessNews)
      }),
    [],
  )
  // the focus the window has now: the session polls by it until the window gains or loses it
  useEffect(() => setDemandFocused(isWindowFocused()), [])
  useEffect(() => watchFailingRobots(), [])
  useEffect(() => watchRobotCoverage(), [])
  useEffect(() => watchReviewReadiness(readinessKeysNeedingAnswer), [])
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
          void refreshReviewRequests("sync")
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
      // the session lists them on the focus by its rule; a row that waits for an approval asks at once
      if (isAwaiting()) {
        void refreshPullRequests("manual")
      }
      void refreshReviewRequests("focus")
    },
    () => setWindowFocus(false),
  )
}
