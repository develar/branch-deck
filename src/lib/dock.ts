import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"
import { isWindows } from "@/lib/platform"
import type { DockProgress, DockProgressKind } from "@/lib/robot-overview"
import { hasProgressText } from "@/lib/title-bar"

/** What the Dock shows now, as last set: a call only on a change */
let shownProgress: DockProgress = { status: "none" }
let shownBadge = 0

/** What the badge counts: what needs the user (failed robots, conflicts), and the pull requests that wait for a review */
export type BadgeSource = "attention" | "reviewRequests"

const badgeCounts: Record<BadgeSource, number> = { attention: 0, reviewRequests: 0 }

function sameProgress(a: DockProgress, b: DockProgress): boolean {
  if (a.status !== b.status) {
    return false
  }
  return a.status === "none" || ((a.status !== "normal" || a.progress === (b as typeof a).progress) && a.kind === (b as typeof a).kind)
}

/** The text of the progress in the tab of the IDE, before its percent: what runs */
const PROGRESS_TEXTS: Readonly<Record<DockProgressKind, string>> = { safePush: "Safe Push", dryRun: "Dry Run", update: "Update", fetch: "Fetch" }

/**
 * The progress bar of the Dock (app-wide on macOS, the taskbar button on Windows): the running robot nearest to
 * finishing. In the IDE, whose tab shows a text beside the progress (`hasProgressText`), the progress also names what
 * runs ("Safe Push", "Dry Run"): the plugin call of Tauri with the text, which `brd mcp` takes; the Tauri call has no
 * text. An error is logged and otherwise ignored: the Dock is an extra.
 */
export async function setDockProgress(progress: DockProgress): Promise<void> {
  if (sameProgress(progress, shownProgress)) {
    return
  }
  shownProgress = progress
  try {
    if (hasProgressText) {
      await setProgressWithText(progress)
      return
    }
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
  } catch (error: unknown) {
    logError("Cannot show the robot progress in the Dock", error)
  }
}

/**
 * The progress in the IDE: `plugin:window|set_progress_bar` as the Tauri API sends it, its value with the text of what
 * runs (`setProgress {view, fraction?, text}` of `brd mcp`); an indeterminate progress has its text and no percent
 */
async function setProgressWithText(progress: DockProgress): Promise<void> {
  const text = progress.status === "none" || progress.kind === undefined ? undefined : PROGRESS_TEXTS[progress.kind]
  const value =
    progress.status === "normal"
      ? { status: ProgressBarStatus.Normal, progress: progress.progress }
      : { status: progress.status === "none" ? ProgressBarStatus.None : ProgressBarStatus.Indeterminate }
  await invoke("plugin:window|set_progress_bar", { value: { ...value, ...(text !== undefined && { text }) } })
}

/**
 * Sets the count of one source of the badge of the Dock; the badge shows the sum, none at 0. Each source is set from
 * its own place (`useDockStatus`), behind its switch in Settings. Windows has no badge count.
 */
export async function setBadgeSource(source: BadgeSource, count: number): Promise<void> {
  badgeCounts[source] = count
  const total = badgeCounts.attention + badgeCounts.reviewRequests
  if (isWindows || total === shownBadge) {
    return
  }
  shownBadge = total
  try {
    await getCurrentWindow().setBadgeCount(total === 0 ? undefined : total)
  } catch (error: unknown) {
    logError("Cannot show the badge in the Dock", error)
  }
}

/** Forgets what the Dock shows (tests) */
export function resetDock(): void {
  shownProgress = { status: "none" }
  shownBadge = 0
  badgeCounts.attention = 0
  badgeCounts.reviewRequests = 0
}
