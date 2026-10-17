import { useEffect, useState } from "react"
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"
import { drawsTitleBar } from "@/lib/title-bar"

export interface WindowFrameState {
  fullscreen: boolean
  maximized: boolean
  /** The window has the focus: the buttons of an inactive window dim, as native ones do */
  active: boolean
  /** The window is tiled to an edge or a half of the screen (Linux, told by the backend: Tauri doesn't know it) */
  tiled: boolean
}

const NORMAL: WindowFrameState = { fullscreen: false, maximized: false, active: true, tiled: false }

/**
 * Whether the window is fullscreen, maximized, tiled and focused, re-read on every resize and focus change. Only a page that
 * draws the title bar (`drawsTitleBar`) asks: macOS hides the traffic lights in fullscreen, the window buttons show
 * Restore for a maximized or fullscreen window and dim while it is inactive.
 */
export function useWindowFrameState(): WindowFrameState {
  const [state, setState] = useState(() => ({ ...NORMAL, active: document.hasFocus() }))
  useEffect(() => {
    if (!drawsTitleBar) {
      return
    }

    const window = getCurrentWindow()
    let disposed = false
    const update = () => {
      Promise.all([window.isFullscreen(), window.isMaximized()])
        .then(([fullscreen, maximized]) => {
          if (!disposed) {
            setState(previous => (previous.fullscreen === fullscreen && previous.maximized === maximized ? previous : { ...previous, fullscreen, maximized }))
          }
        })
        .catch((error: unknown) => {
          logError("Failed to read the window state", error)
        })
    }
    update()
    const unlistenResize = window.onResized(update)
    const unlistenFocus = window.onFocusChanged(({ payload: active }) => {
      if (!disposed) {
        setState(previous => (previous.active === active ? previous : { ...previous, active }))
      }
    })
    const unlistenTiled = getCurrentWebviewWindow().listen<boolean>("window-tiled", ({ payload: tiled }) => {
      if (!disposed) {
        setState(previous => (previous.tiled === tiled ? previous : { ...previous, tiled }))
      }
    })
    return () => {
      disposed = true
      for (const unlisten of [unlistenResize, unlistenFocus, unlistenTiled]) {
        unlisten
          .then(it => it())
          .catch((error: unknown) => {
            logError("Failed to stop listening to the window", error)
          })
      }
    }
  }, [])
  return state
}
