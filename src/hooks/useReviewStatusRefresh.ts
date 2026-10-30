import { useEffect } from "react"
import { tellReadinessNews } from "@/hooks/useStackNextStep"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { approvalNewsOf } from "@/lib/approval-news"
import { tellAutoMergedNews } from "@/lib/auto-merge-news"
import { tellLandedNews } from "@/lib/landed-closes"
import { isWindowFocused, setWindowFocus } from "@/lib/os-notification"
import { tellRobotNews, watchFailingRobots } from "@/lib/patronus-failing"
import { tellRemoteCommitsNews } from "@/lib/remote-commits-news"
import { setDemandFocused } from "@/model/demand"
import { onSessionFacts } from "@/model/store"
import { branchSyncStore } from "@/stores/branch-sync"
import { applySessionFacts } from "@/stores/patronus"
import { applyPullRequestsFacts, refreshPullRequests } from "@/stores/pull-requests"
import { applyReadinessFacts } from "@/stores/review-readiness"
import { applyReviewRequestsFacts } from "@/stores/review-requests"
import { applyReviewsView } from "@/stores/reviews"
import { watchRobotCoverage } from "@/stores/robot-coverage"
import { applyRobotDetailsFacts } from "@/stores/robot-details"

/** A row or the header of a whole stack waits for an approval (Awaiting: rows 10 and 10′ of the next step) */
function isAwaiting(): boolean {
  return Object.values(branchSyncStore.getState().nextSteps).some(({ step }) => step.kind.kind === "awaiting" || step.kind.kind === "awaitingUnjudged")
}

/**
 * Keeps the review status of the main window current. The session of the repository lists the GitHub pull requests
 * (after a sync, on a focus, after a push and a write of the app, by its rules) and lists and polls the robots of
 * Patronus itself (`remote_facts::scheduler`, by the window's focus, which it tells from the start, and its demand); the
 * stores take them from its view (`applyPullRequestsFacts`, `applySessionFacts`) with the view the session derives of
 * the robots (what they land, landed and hold, the landing needs), the rules of the bases of the approved pull requests
 * and the merge profiles; a running robot that will fail (`watchFailingRobots`), a robot that merged or failed (the news
 * of the session, `tellRobotNews`) are told once; which commits a running robot covers, once its branch changed since
 * it started (`watchRobotCoverage`). The review requests of others are the session's too, with their news
 * (`applyReviewRequestsFacts`: after a sync, on a focus, every five minutes focused or not, and while a subject waits
 * for approvals each of those polls lists the pull requests too; a sign-in or a sign-out of GitHub in any window has the
 * session list them again at once, and read its fact `auth`, which the store of the session applies). The session
 * asks the readiness of the shown subjects
 * whose next step depends on it (the demand of the window), the whole stack of an island header too; while a row or
 * such a header waits for an approval
 * (Awaiting), a focus asks the session for the pull requests at once (the user comes back from asking for it); the
 * approvals that arrived are news of the session (`tellReadinessNews`), and so are the commits of others the session
 * fetched when a pull request showed a head the remote branch lacked (`tellRemoteCommitsNews`), and the landings without
 * a robot whose pull requests close (`tellLandedNews`), and the pull requests GitHub merged by their auto-merge
 * (`tellAutoMergedNews`).
 */
export function useReviewStatusRefresh(): void {
  useEffect(
    () =>
      onSessionFacts((repositoryPath, view, changes, news) => {
        applySessionFacts(repositoryPath, view, changes)
        applyPullRequestsFacts(repositoryPath, view.facts, changes?.facts ?? null)
        applyReadinessFacts(repositoryPath, view.facts, changes?.facts ?? null)
        applyRobotDetailsFacts(repositoryPath, view.facts, changes?.facts ?? null)
        applyReviewRequestsFacts(repositoryPath, view.facts, changes?.facts ?? null, news)
        applyReviewsView(repositoryPath, view, changes)
        tellRobotNews(repositoryPath, news)
        approvalNewsOf(news).forEach(tellReadinessNews)
        tellRemoteCommitsNews(repositoryPath, news)
        tellLandedNews(repositoryPath, news)
        tellAutoMergedNews(repositoryPath, news)
      }),
    [],
  )
  // the focus the window has now: the session polls by it until the window gains or loses it
  useEffect(() => setDemandFocused(isWindowFocused()), [])
  useEffect(() => watchFailingRobots(), [])
  useEffect(() => watchRobotCoverage(), [])
  useWindowFocus(
    () => {
      setWindowFocus(true)
      // the session lists them on the focus by its rule; a row that waits for an approval asks at once
      if (isAwaiting()) {
        void refreshPullRequests("manual")
      }
    },
    () => setWindowFocus(false),
  )
}
