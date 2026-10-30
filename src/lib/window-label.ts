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

/**
 * The label of the review window of a branch: one window, reused for the next branch. The window subscribes to the
 * session of its repository itself for the heads a sync built (`settledReviewTable`); its link
 * (`@/lib/sub-window-link`) sends the actions of each branch of its chain (`BranchReviewWindowState`), and carries
 * the actions it asks for (`BranchReviewRequest`) with their refusals.
 */
export const BRANCH_REVIEW_WINDOW = "branch-review"

/** The label of the review window of a commit: one window, reused for the next commit (`openCommitReviewWindow`) */
export const COMMIT_REVIEW_WINDOW = "commit-review"
