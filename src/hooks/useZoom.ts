import { useEffect, useRef } from "react"
import { LogicalSize } from "@tauri-apps/api/dpi"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useAppCommand } from "@/hooks/useAppCommand"
import { logError } from "@/lib/errors"
import { isMac } from "@/lib/platform"
import { MIN_WINDOW, minSubWindowSize, minWindowWidth, nextZoom } from "@/lib/zoom"
import type { ZoomAction } from "@/lib/zoom"
import { isMainWindow } from "@/stores/settings/app-store"
import { appSettingsStore, setZoom, useAppSettings } from "@/stores/settings/appSettings"

function zoom(action: ZoomAction): void {
  setZoom(nextZoom(appSettingsStore.getState().zoom, action))
}

async function applyZoom(level: number): Promise<void> {
  try {
    await getCurrentWebview().setZoom(level)
    // a wider minimum also widens a narrower window
    const current = getCurrentWindow()
    const subWindow = minSubWindowSize(current.label, level)
    const minSize = isMainWindow ? new LogicalSize(minWindowWidth(level), MIN_WINDOW.height) : new LogicalSize(subWindow.width, subWindow.height)
    await current.setMinSize(minSize)
  } catch (error: unknown) {
    logError("Cannot zoom the window", error)
  }
}

/**
 * Zooms the page of this window to the saved level when it changes, in any window. The window builder applies the
 * level the window opens with, before the first paint (`src-tauri/src/zoom.rs`).
 */
export function useZoom(): void {
  const level = useAppSettings(state => state.zoom)
  const appliedLevel = useRef(level)
  useEffect(() => {
    if (appliedLevel.current !== level) {
      appliedLevel.current = level
      void applyZoom(level)
    }
  }, [level])

  // the menu items Actual Size, Zoom In and Zoom Out (⌘0, ⌘=, ⌘-): the native menu sends them to the main window only,
  // the main menu and its keys (`useMainMenuKeys`) run them in the focused window; one press is one step for all windows
  useAppCommand<ZoomAction>("zoom", action => zoom(action))

  // a menu item has a single shortcut, so the page takes the other one of Zoom In, as browsers have both: ⌘= on macOS,
  // where the item is ⌘+ (`show_zoom_in_as_plus` in src-tauri/src/menu.rs), Ctrl++ (Ctrl+⇧=, or + on the numeric
  // keypad) elsewhere, where it is Ctrl+=. The page sees a key first, so handling it keeps the menu from getting it too.
  useEffect(() => {
    const otherKey = isMac ? "=" : "+"
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key === otherKey && (event.metaKey || event.ctrlKey) && !event.altKey) {
        event.preventDefault()
        zoom("in")
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => {
      window.removeEventListener("keydown", onKeyDown)
    }
  }, [])
}
