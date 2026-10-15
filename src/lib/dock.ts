import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"
import { isWindows } from "@/lib/platform"
import type { DockProgress } from "@/lib/robot-overview"

/** What the Dock shows now, as last set: a call only on a change */
let shownProgress: DockProgress = { status: "none" }
let shownBadge = 0

function sameProgress(a: DockProgress, b: DockProgress): boolean {
  return a.status === b.status && (a.status !== "normal" || a.progress === (b as typeof a).progress)
}

/**
 * The progress bar of the Dock (app-wide on macOS, the taskbar button on Windows): the running robot nearest to
 * finishing. An error is logged and otherwise ignored: the Dock is an extra.
 */
export async function setDockProgress(progress: DockProgress): Promise<void> {
  if (sameProgress(progress, shownProgress)) {
    return
  }
  shownProgress = progress
  try {
    switch (progress.status) {
      case "none":
        await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.None })
        break
      case "indeterminate":
        // macOS draws an indeterminate bar as a normal one at its last value: an empty track says "running"
        await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.Indeterminate, progress: 0 })
        break
      case "normal":
        await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.Normal, progress: progress.progress })
        break
    }
  }
  catch (error: unknown) {
    logError("Cannot show the robot progress in the Dock", error)
  }
}

/** The badge of the Dock: what needs the user, never a running robot; none at 0. Windows has no badge count. */
export async function setDockBadge(count: number): Promise<void> {
  if (isWindows || count === shownBadge) {
    return
  }
  shownBadge = count
  try {
    await getCurrentWindow().setBadgeCount(count === 0 ? undefined : count)
  }
  catch (error: unknown) {
    logError("Cannot show the badge in the Dock", error)
  }
}
