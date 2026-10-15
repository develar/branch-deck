import { useEffect, useRef } from "react"
import { LogicalSize } from "@tauri-apps/api/dpi"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTauriListen } from "@/hooks/useTauriListen"
import { logError } from "@/lib/errors"
import { MIN_WINDOW, minWindowWidth, nextZoom } from "@/lib/zoom"
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
    if (isMainWindow) {
      await getCurrentWindow().setMinSize(new LogicalSize(minWindowWidth(level), MIN_WINDOW.height))
    }
  }
  catch (error: unknown) {
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

  // the menu items Actual Size, Zoom In and Zoom Out (⌘0, ⌘=, ⌘-) go to the main window only, so one press is one
  // step for all windows
  useTauriListen<ZoomAction>("zoom", (action) => {
    if (isMainWindow) {
      zoom(action)
    }
  })

  // ⌘+ (⌘⇧=, or + on the numeric keypad) as in browsers: a menu item has a single shortcut, and the page sees the key
  // first, so handling it here keeps the menu from getting it as well
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key === "+" && (event.metaKey || event.ctrlKey) && !event.altKey) {
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
