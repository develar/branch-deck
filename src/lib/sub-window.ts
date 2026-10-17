import { getCurrentWindow } from "@tauri-apps/api/window"
import { info } from "@tauri-apps/plugin-log"
import type { SubWindowOpen } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { markOpenPhase, takeOpenTimingLine } from "@/lib/open-timing"
import type { OpenPhase } from "@/lib/open-timing"

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

function subWindowLabel(): string | null {
  try {
    const label = getCurrentWindow().label
    return label === "main" ? null : label
  } catch {
    return null
  }
}

/** The open the page shows (`__SUB_WINDOW_OPEN__`, set by the backend with the data of each open) */
function currentOpen(): SubWindowOpen | undefined {
  return typeof window === "undefined" ? undefined : window.__SUB_WINDOW_OPEN__
}

/**
 * The page of a sub-window loaded: the backend counts the content budget from now and hands over the data of an open
 * that came while the page loaded. Once per page load, at its start; nothing in the main window. No animation frame
 * waits on this path: a hidden window runs none.
 */
function announceSubWindowLoaded(): void {
  if (subWindowLabel() === null) {
    return
  }
  markOpenPhase(currentOpen(), "loaded")
  void commands.subWindowLoaded().then(result => {
    if (result.status === "error") {
      logError("Cannot show the sub-window", result.error)
      return
    }
    const update = result.data
    if (update !== null) {
      window.__TAURI_STORE__ = JSON.parse(update.storeCache) as Record<string, unknown>
      window.__INIT_DATA__ = JSON.parse(update.data) as unknown
      window.__SUB_WINDOW_OPEN__ = update.open
      window.dispatchEvent(new CustomEvent("init-data-updated"))
    }
  })
}

announceSubWindowLoaded()

/** Records a phase of the open the page shows, for its timing line (`markSubWindowContentReady`) */
export function markSubWindowOpenPhase(phase: Exclude<OpenPhase, "loaded" | "ready">): void {
  markOpenPhase(currentOpen(), phase)
}

let lastReadyOpen: number | null = null

/**
 * The page settled the content of the open it shows: the backend shows the window, built hidden so it never shows a
 * blank frame, if that open is the latest. Once per open, and logs how long the open took, by phase ("commit-review
 * open 96 ms: loaded 41, files 44, patch 47"). A page whose content is its data calls it on render (`WindowPage`), a
 * review once its first diffs are prepared. Nothing in the main window.
 */
export function markSubWindowContentReady(): void {
  const label = subWindowLabel()
  const open = currentOpen()
  if (label === null || open === undefined || open.id === lastReadyOpen) {
    return
  }
  lastReadyOpen = open.id
  markOpenPhase(open, "ready")
  const line = takeOpenTimingLine(label, open.id)
  if (line !== null) {
    void info(line)
  }
  void commands.subWindowReady(open.id).then(result => {
    if (result.status === "error") {
      logError("Cannot show the sub-window", result.error)
    }
  })
}
