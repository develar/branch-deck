import { useEffect } from "react"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import { setBadgeSource, setDockProgress } from "@/lib/dock"
import { getRobotTiming } from "@/lib/robot-eta"
import { countConflicts, getDockProgress, isFailingEntry } from "@/lib/robot-overview"
import { useBranchSync } from "@/stores/branch-sync"
import { getRobotEstimate, useRepositoryViewCurrent } from "@/stores/patronus"
import { useReviewRequestCount } from "@/stores/review-requests"
import { isMainWindow } from "@/stores/settings/app-store"
import { useAppSettings } from "@/stores/settings/appSettings"

/**
 * The Dock of the app, from the main window only: the progress of the running robot nearest to finishing (indeterminate
 * without an estimate, none while nothing runs; a failing robot is not waited for), and a badge that adds up what needs
 * the user (the robots that failed on the branch as it is, "Needs review" too, the running robots that will fail, and
 * the conflicts of the last sync; never another running robot) and the pull
 * requests that wait for the user's review, each behind its switch in Settings. The robots' part waits for the
 * repository view of the robots as they are (`useRepositoryViewCurrent`): a failure another robot lands asks for
 * nothing, so no badge comes and goes before the view says it. It renews with the relative times; the
 * Tauri calls go out only on a change. While an update runs (`updateRunning`), the progress is the update's. Use it in a
 * component that renders nothing else.
 */
export function useDockStatus(updateRunning: boolean): void {
  const overview = useRobotOverview()
  const conflicts = useBranchSync(state => countConflicts(state.branches))
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const progress = getDockProgress(overview.running.filter(it => !isFailingEntry(it)).map(it => getRobotTiming(it.robot, getRobotEstimate(it.robot), now)))
  const attentionBadge = useAppSettings(state => state.attentionBadge)
  const reviewBadge = useAppSettings(state => state.reviewBadge)
  const attention = attentionBadge ? overview.attention.length + overview.running.filter(isFailingEntry).length + conflicts : 0
  const viewCurrent = useRepositoryViewCurrent()
  const reviewRequests = useReviewRequestCount()
  const reviews = reviewBadge ? reviewRequests : 0

  const percent = progress.status === "normal" ? progress.progress : null
  useEffect(() => {
    // a running update shows its own progress (`DockStatus`); the robots' comes back once it ends
    if (isMainWindow && !updateRunning) {
      void setDockProgress(percent === null ? { status: progress.status as "none" | "indeterminate" } : { status: "normal", progress: percent })
    }
  }, [progress.status, percent, updateRunning])
  useEffect(() => {
    if (isMainWindow && viewCurrent) {
      void setBadgeSource("attention", attention)
    }
  }, [attention, viewCurrent])
  useEffect(() => {
    if (isMainWindow) {
      void setBadgeSource("reviewRequests", reviews)
    }
  }, [reviews])
}
