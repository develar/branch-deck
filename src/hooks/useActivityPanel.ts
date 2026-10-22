import { useEffect, useState } from "react"
import type { RefObject } from "react"
import { useAppCommand } from "@/hooks/useAppCommand"
import type { ActivityFilter, ActivityTarget } from "@/lib/activity-window"
import { openActivityWindow } from "@/lib/activity-window-link"
import { isKeyboardInput, watchInputModality } from "@/lib/input-modality"
import { MIN_ISLANDS_WIDTH } from "@/lib/zoom"
import { closeActivityPanel, isActivityPanelOpen, openActivityPanel } from "@/stores/activity-panel"
import { useSelectedProjectPath } from "@/stores/repository"
import { useAppSettings } from "@/stores/settings/appSettings"

/** The narrowest Activity side panel: its search and a line of an entry */
export const ACTIVITY_PANEL_MIN_WIDTH = 320

/** Opens Activity in its window at the chip of `kind`, the side panel closed: the log shows in one place */
export function openActivityInWindow(kind: ActivityFilter | null): void {
  closeActivityPanel()
  openActivityWindow(kind)
}

/** The panel fits beside the islands, which keep their minimum width (`MIN_ISLANDS_WIDTH`) */
function fits(width: number): boolean {
  return width >= MIN_ISLANDS_WIDTH + ACTIVITY_PANEL_MIN_WIDTH
}

/**
 * The Activity side panel of the main window (spec/repository/activity.spec.md): "Activity" of every window (the header
 * button, the menus, the app command `open_activity`) toggles it; "Show in Activity" opens it at its chip. Where the
 * area under the header (`area`) has no room for it beside the islands, the open goes to the Activity window instead.
 * The focus goes into the panel only when the keyboard opened it; the native menu, which the page doesn't see, never
 * moves it. Returns whether the panel shows: it is open, it fits, and a repository is selected.
 */
export function useActivityPanel(area: RefObject<HTMLElement | null>): boolean {
  const isOpen = useAppSettings(state => state.activityPanel)
  const hasRepository = useSelectedProjectPath() !== undefined
  const [width, setWidth] = useState<number | null>(null)
  useEffect(() => watchInputModality(), [])
  useEffect(() => {
    const element = area.current
    if (element === null) {
      return
    }
    const observer = new ResizeObserver(() => setWidth(element.offsetWidth))
    observer.observe(element)
    return () => observer.disconnect()
  }, [area])

  // the native item sends no target (null): a menu of the system, not a key of the page
  useAppCommand<ActivityTarget | null>("open_activity", target => {
    const kind = target?.kind ?? null
    const element = area.current
    const hasRoom = element !== null && fits(element.offsetWidth)
    if (isActivityPanelOpen() && hasRoom && kind === null) {
      closeActivityPanel()
    } else if (hasRoom) {
      openActivityPanel(kind, target !== null && isKeyboardInput())
    } else {
      openActivityInWindow(kind)
    }
  })
  return isOpen && hasRepository && width !== null && fits(width)
}
