import { useEffect, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"
import { titleBar } from "@/lib/title-bar"

export interface WindowFrameState {
  fullscreen: boolean
  maximized: boolean
}

const NORMAL: WindowFrameState = { fullscreen: false, maximized: false }

/**
 * Whether the window is fullscreen or maximized, re-read on every resize. Only a page that draws the title bar
 * (`titleBar`) asks: macOS hides the traffic lights in fullscreen, Windows shows Restore for a maximized window.
 */
export function useWindowFrameState(): WindowFrameState {
  const [state, setState] = useState(NORMAL)
  useEffect(() => {
    if (titleBar === null) {
      return
    }

    const window = getCurrentWindow()
    let disposed = false
    const update = () => {
      Promise.all([window.isFullscreen(), window.isMaximized()])
        .then(([fullscreen, maximized]) => {
          if (!disposed) {
            setState(previous => previous.fullscreen === fullscreen && previous.maximized === maximized ? previous : { fullscreen, maximized })
          }
        })
        .catch((error: unknown) => {
          logError("Failed to read the window state", error)
        })
    }
    update()
    const unlisten = window.onResized(update)
    return () => {
      disposed = true
      unlisten
        .then(it => it())
        .catch((error: unknown) => {
          logError("Failed to stop listening to window resizes", error)
        })
    }
  }, [])
  return state
}
