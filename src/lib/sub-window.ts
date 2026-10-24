import { getCurrentWindow } from "@tauri-apps/api/window"
import { info } from "@tauri-apps/plugin-log"
import type { SubWindowOpen } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { markOpenPhase, takeOpenTimingLine } from "@/lib/open-timing"
import type { OpenPhase } from "@/lib/open-timing"
import { acceptSettingsChanges, listeningToSettings } from "@/stores/settings/persistent-store"

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
    // components that handle Escape themselves (popovers, inline forms) stop propagation or prevent default; a dialog
    // of the window (Create pull request) closes itself, its Escape is not the window's
    if (event.key === "Escape" && !event.defaultPrevented && !(event.target instanceof Element && event.target.closest("[role=dialog]") !== null)) {
      currentWindow.close().catch((error: unknown) => {
        logError("Failed to close sub-window", error)
      })
    }
  })
}

installEscapeToClose()

/** The label of the page's window, null without one (a unit test of another module) */
function currentLabel(): string | null {
  try {
    return getCurrentWindow().label
  } catch {
    return null
  }
}

/**
 * The open the page shows (`__SUB_WINDOW_OPEN__`, set by the backend with the data of each open of a sub-window; the
 * main window's is its build)
 */
function currentOpen(): SubWindowOpen | undefined {
  return typeof window === "undefined" ? undefined : window.__SUB_WINDOW_OPEN__
}

/**
 * The page of a window loaded and listens to the settings: the backend hands over the data of an open that came while
 * the page loaded and the changes of the settings after the snapshot the window was built with, which reached no
 * listener; the page applies them by their versions before it says its content settled (`markWindowContentReady`), so
 * a zoom changed meanwhile is the window's when it shows. Once per page load, at its start; the main window gets
 * neither (it has one open and asks for the changes itself, `get_settings_changes`). From now the backend waits for the
 * shell of the page (`markWindowShellDrawn`), no longer for its load. No animation frame waits on this path: a hidden
 * window runs none.
 */
function announceWindowLoaded(): Promise<void> {
  if (currentLabel() === null) {
    return Promise.resolve()
  }
  markOpenPhase(currentOpen(), "loaded")
  return listeningToSettings
    .then(() => commands.subWindowLoaded())
    .then(result => {
      if (result.status === "error") {
        logError("Cannot show the sub-window", result.error)
        return
      }
      const { update, settings } = result.data
      acceptSettingsChanges(settings)
      if (update !== null) {
        window.__INIT_DATA__ = JSON.parse(update.data) as unknown
        window.__SUB_WINDOW_OPEN__ = update.open
        window.dispatchEvent(new CustomEvent("init-data-updated"))
      }
    })
    .catch((error: unknown) => {
      logError("Cannot show the sub-window", error)
    })
}

/** Resolves once the backend answered the load of the page and the page applied what it handed over */
const pageLoaded = announceWindowLoaded()

let shellDrawn = false

/**
 * The page committed its shell, the title row of its window (`WindowTitleBar`; the main window's with its loading
 * table, which the same render commits): the backend counts the content budget of the window from now, so a page whose
 * content is slow shows its shell, never a bare canvas. Once per page load, after the load (`sub_window_loaded`), which
 * the backend must have heard first. No animation frame waits on this path: a hidden window runs none.
 */
export function markWindowShellDrawn(): void {
  if (shellDrawn || currentLabel() === null) {
    return
  }
  shellDrawn = true
  markOpenPhase(currentOpen(), "shell")
  void pageLoaded
    .then(() => commands.subWindowDrawn())
    .then(result => {
      if (result.status === "error") {
        logError("Cannot show the sub-window", result.error)
      }
    })
}

/** Records a phase of the open the page shows, for its timing line (`markWindowContentReady`) */
export function markSubWindowOpenPhase(phase: Exclude<OpenPhase, "loaded" | "shell" | "ready">): void {
  markOpenPhase(currentOpen(), phase)
}

let lastReadyOpen: number | null = null

/**
 * The page settled the content of the open it shows: the backend shows the window, built hidden so it never shows a
 * blank frame, if that open is the latest. Once per open, and logs how long the open took, by phase ("commit-review
 * open 96 ms: loaded 41, files 44, diffs 47"). A page whose content is its data calls it on render (`WindowPage`), a
 * review once its first diffs are prepared, the main window once it drew its first rows or the state without rows
 * (`BranchCreator`; "main open 812 ms: loaded 240", from the build of the window).
 */
export function markWindowContentReady(): void {
  const label = currentLabel()
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
  // after the load's changes of the settings, so the window shows with them
  void pageLoaded
    .then(() => commands.subWindowReady(open.id))
    .then(result => {
      if (result.status === "error") {
        logError("Cannot show the sub-window", result.error)
      }
    })
}
