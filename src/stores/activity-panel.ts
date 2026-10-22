import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { ActivityFilter } from "@/lib/activity-window"
import { appSettingsStore } from "@/stores/settings/appSettings"

/**
 * The open of the Activity side panel of the main window (spec/repository/activity.spec.md): whether it is open is a
 * setting (`appSettings.activityPanel`, kept across starts), the rest is the session's: each open starts over at its
 * chip with an empty search, and takes the focus only when the keyboard opened it.
 */
interface ActivityPanelOpen {
  /** When it was opened: a new open mounts the content anew */
  readonly openedAt: number
  /** The chip the open names ("Show in Activity": Robots), null for All */
  readonly kind: ActivityFilter | null
  /** The keyboard opened it: the focus goes into the log */
  readonly focus: boolean
}

const activityPanelStore = createStore<ActivityPanelOpen>(() => ({ openedAt: Date.now(), kind: null, focus: false }))

export function useActivityPanelOpen(): ActivityPanelOpen {
  return useStore(activityPanelStore, state => state)
}

/** The panel is open (it shows where it fits, `activityPanelFits`) */
export function isActivityPanelOpen(): boolean {
  return appSettingsStore.getState().activityPanel
}

export function openActivityPanel(kind: ActivityFilter | null, focus: boolean): void {
  activityPanelStore.setState({ openedAt: Date.now(), kind, focus })
  if (!appSettingsStore.getState().activityPanel) {
    appSettingsStore.setState({ activityPanel: true })
  }
}

export function closeActivityPanel(): void {
  if (appSettingsStore.getState().activityPanel) {
    appSettingsStore.setState({ activityPanel: false })
  }
}
