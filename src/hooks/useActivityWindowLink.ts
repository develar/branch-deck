import { useEffect } from "react"
import { useActivityReads } from "@/hooks/useActivityReads"
import { useTauriListen } from "@/hooks/useTauriListen"
import { ACTIVITY_WINDOW } from "@/lib/activity-window"
import type { ActivityWindowRequest } from "@/lib/activity-window"
import { broadcastActivityState, runActivityRequest, subscribeActivityState } from "@/lib/activity-window-link"
import { gitHubRepositoryOf } from "@/lib/pull-request"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { usePullRequestAvailability } from "@/stores/pull-requests"

/**
 * Activity of the main window and its side of the Activity window. The main window reads the log
 * (`useActivityReads`), for its side panel and the header button; it opens the window (`useActivityPanel`: where the
 * side panel has no room, and its Open in Window), which reads the log itself, tells it what only the main window
 * knows (the branches, the landed report and the links of its branches, the GitHub repository, the Undo of this
 * session's rewrites, the selected repository) whenever it changes, and runs what it asks for. Main window only.
 */
export function useActivityWindowLink(): void {
  useActivityReads(gitHubRepositoryOf(usePullRequestAvailability()))
  useEffect(() => subscribeActivityState(broadcastActivityState), [])
  useTauriListen<ActivityWindowRequest>(windowLinkEvents(ACTIVITY_WINDOW).request, runActivityRequest)
}
