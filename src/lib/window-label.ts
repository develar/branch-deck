import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow"

function detectWindowLabel(): string {
  try {
    return getCurrentWebviewWindow().label
  } catch {
    // no window metadata (a unit test): the main window
    return "main"
  }
}

/** The label of the window of this page: `main`, or that of a sub-window (`commit-review`, `settings`, …) */
export const windowLabel = detectWindowLabel()

/** This page is the main window's */
export const isMainWindow = windowLabel === "main"
