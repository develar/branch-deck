import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow"

function detectWindowLabel(): string {
  try {
    return getCurrentWebviewWindow().label
  } catch {
    // no window metadata (a unit test): the main window
    return "main"
  }
}

/** The label of the window of this page: `main`, or that of a sub-window (`review`, `settings`, …) */
export const windowLabel = detectWindowLabel()

/** This page is the main window's */
export const isMainWindow = windowLabel === "main"

/**
 * The label of the review window, one for every subject (a branch with its stack, a whole stack, a kept branch, a list
 * of commits), reused for the next one (`@/lib/review-window`). The window subscribes to the session of its repository
 * itself and builds the actions of what it shows from it (`useReviewActions`); it asks the main window only for those
 * that open a form under a row (`review/run`, `@/lib/review-runs`).
 */
export const REVIEW_WINDOW = "review"
