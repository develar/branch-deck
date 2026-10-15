import { useEffect } from "react"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import { setBadgeSource, setDockProgress } from "@/lib/dock"
import { getRobotTiming } from "@/lib/robot-eta"
import { countConflicts, getDockProgress } from "@/lib/robot-overview"
import { useBranchSync } from "@/stores/branch-sync"
import { getExpectedRobotDuration } from "@/stores/patronus"
import { useReviewRequestCount } from "@/stores/review-requests"
import { isMainWindow } from "@/stores/settings/app-store"
import { useAppSettings } from "@/stores/settings/appSettings"

/**
 * The Dock of the app, from the main window only: the progress of the running robot nearest to finishing (indeterminate
 * without an estimate, none while nothing runs), and a badge that adds up what needs the user (the robots that failed
 * on the branch as it is, "Needs review" too, and the conflicts of the last sync; never a running robot) and the pull
 * requests that wait for the user's review, each behind its switch in Settings. It renews with the relative times; the
 * Tauri calls go out only on a change. Use it in a component that renders nothing else.
 */
export function useDockStatus(): void {
  const overview = useRobotOverview()
  const conflicts = useBranchSync(state => countConflicts(state.branches))
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const progress = getDockProgress(overview.running.map(it => getRobotTiming(it.robot, getExpectedRobotDuration(it.robot), now)))
  const attentionBadge = useAppSettings(state => state.attentionBadge)
  const reviewBadge = useAppSettings(state => state.reviewBadge)
  const attention = attentionBadge ? overview.attention.length + conflicts : 0
  const reviewRequests = useReviewRequestCount()
  const reviews = reviewBadge ? reviewRequests : 0

  const percent = progress.status === "normal" ? progress.progress : null
  useEffect(() => {
    if (isMainWindow) {
      void setDockProgress(percent === null ? { status: progress.status as "none" | "indeterminate" } : { status: "normal", progress: percent })
    }
  }, [progress.status, percent])
  useEffect(() => {
    if (isMainWindow) {
      void setBadgeSource("attention", attention)
    }
  }, [attention])
  useEffect(() => {
    if (isMainWindow) {
      void setBadgeSource("reviewRequests", reviews)
    }
  }, [reviews])
}
