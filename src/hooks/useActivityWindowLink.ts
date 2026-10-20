import { useEffect } from "react"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useTauriListen } from "@/hooks/useTauriListen"
import { ACTIVITY_WINDOW } from "@/lib/activity-window"
import type { ActivityTarget, ActivityWindowRequest } from "@/lib/activity-window"
import { broadcastActivityState, openActivityWindow, runActivityRequest } from "@/lib/activity-window-link"
import { windowLinkEvents } from "@/lib/sub-window-link"
import { activityStore } from "@/stores/activity"
import { branchSyncStore } from "@/stores/branch-sync"
import { landedStore } from "@/stores/landed"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { repositoryStore } from "@/stores/repository"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/**
 * The Activity window, which doesn't own the log: "Activity" of every window opens it here (the header button, the
 * menus, "Show in Activity" of the landing overview); it is told the log of the selected repository whenever the log,
 * the branches, the landed report, the links of the landed branches or the selected repository change, and what it
 * asks for runs here. Main window only.
 */
export function useActivityWindowLink(): void {
  useAppCommand<ActivityTarget | null>("open_activity", target => openActivityWindow(target?.kind ?? null))
  useEffect(() => {
    const stores = [activityStore, branchSyncStore, landedStore, pullRequestsStore, patronusStore, repositoryStore, repositorySettingsStore]
    const unsubscribes = stores.map(store => store.subscribe(broadcastActivityState))
    return () => {
      for (const unsubscribe of unsubscribes) {
        unsubscribe()
      }
    }
  }, [])
  useTauriListen<ActivityWindowRequest>(windowLinkEvents(ACTIVITY_WINDOW).request, runActivityRequest)
}
