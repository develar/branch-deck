import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { ACTIVITY_WINDOW } from "@/lib/activity-window"
import type { ActivityWindowRequest } from "@/lib/activity-window"
import { broadcastActivityState, runActivityRequest, subscribeActivityState } from "@/lib/activity-window-link"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { refreshActivity, startActivityReads } from "@/stores/activity"
import { getGitHubRepository, pullRequestsStore } from "@/stores/pull-requests"
import { repositoryStore } from "@/stores/repository"

/** The event the backend tells a change of a journal Activity reads with (`activity-changed`: an operation of `bd` ended) */
const ACTIVITY_CHANGED = "activity-changed"

/**
 * The log of Activity and the Activity window, which doesn't own the log. The main window reads the log from the
 * journals (`startActivityReads`), again when one changes; it opens the window (`useActivityPanel`: where the side panel
 * has no room, and its Open in Window), tells it the log of the selected repository whenever the log, the branches,
 * the landed report, the links of the landed branches or the selected repository change, and runs what it asks for.
 * Main window only.
 */
export function useActivityWindowLink(): void {
  useEffect(
    () =>
      startActivityReads(getGitHubRepository, changed => {
        const unsubscribes = [pullRequestsStore.subscribe(changed), repositoryStore.subscribe(changed)]
        return () => {
          for (const unsubscribe of unsubscribes) {
            unsubscribe()
          }
        }
      }),
    [],
  )
  useTauriListen<null>(ACTIVITY_CHANGED, () => void refreshActivity())
  useEffect(() => subscribeActivityState(broadcastActivityState), [])
  useTauriListen<ActivityWindowRequest>(windowLinkEvents(ACTIVITY_WINDOW).request, runActivityRequest)
}
