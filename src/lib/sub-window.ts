import { getCurrentWindow } from "@tauri-apps/api/window"
import { info } from "@tauri-apps/plugin-log"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"

// Escape closes a sub-window. Installed once at startup for the lifetime of the window, not per component.
function installEscapeToClose(): void {
  if (typeof window === "undefined") {
    return
  }

  let currentWindow: ReturnType<typeof getCurrentWindow>
  try {
    currentWindow = getCurrentWindow()
  } catch {
    // window metadata unavailable: treat as main
    return
  }
  if (currentWindow.label === "main") {
    return
  }

  window.addEventListener("keydown", event => {
    // components that handle Escape themselves (popovers, inline forms) stop propagation or prevent default
    if (event.key === "Escape" && !event.defaultPrevented) {
      currentWindow.close().catch((error: unknown) => {
        logError("Failed to close sub-window", error)
      })
    }
  })
}

installEscapeToClose()

let isPaintAnnounced = false

function isSubWindow(): boolean {
  try {
    return getCurrentWindow().label !== "main"
  } catch {
    return false
  }
}

/**
 * The page of a sub-window painted (`WindowPage` calls it on its first render): the backend shows the window, built
 * hidden so it never shows a blank frame, and hands over the data of an open that came while the page loaded. Once per
 * page load; nothing in the main window.
 */
export function announceSubWindowPainted(): void {
  if (isPaintAnnounced || !isSubWindow()) {
    return
  }
  isPaintAnnounced = true
  requestAnimationFrame(() => {
    // how long an open takes until the page is on screen (a new window loads the app; a reused one only renders)
    const openedAt = (window.__INIT_DATA__ as { openedAt?: unknown } | undefined)?.openedAt
    if (typeof openedAt === "number") {
      void info(`${getCurrentWindow().label} painted ${Date.now() - openedAt} ms after its open`)
    }
    void commands.subWindowReady().then(result => {
      if (result.status === "error") {
        logError("Cannot show the sub-window", result.error)
        return
      }
      const update = result.data
      if (update !== null) {
        window.__TAURI_STORE__ = JSON.parse(update.storeCache) as Record<string, unknown>
        window.__INIT_DATA__ = JSON.parse(update.data) as unknown
        window.dispatchEvent(new CustomEvent("init-data-updated"))
      }
    })
  })
}
