import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { ACTIVITY_WINDOW } from "@/lib/activity-window"
import type { ActivityWindowRequest } from "@/lib/activity-window"
import { broadcastActivityState, runActivityRequest, subscribeActivityState } from "@/lib/activity-window-link"
import { windowLinkEvents } from "@/lib/sub-window-link"

/**
 * The Activity window, which doesn't own the log: the main window opens it (`useActivityPanel`: where the side panel
 * has no room, and its Open in Window); it is told the log of the selected repository whenever the log, the branches,
 * the landed report, the links of the landed branches or the selected repository change, and what it asks for runs
 * here. Main window only.
 */
export function useActivityWindowLink(): void {
  useEffect(() => subscribeActivityState(broadcastActivityState), [])
  useTauriListen<ActivityWindowRequest>(windowLinkEvents(ACTIVITY_WINDOW).request, runActivityRequest)
}
