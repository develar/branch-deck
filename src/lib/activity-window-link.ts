import { getCurrentWindow } from "@tauri-apps/api/window"
import { ACTIVITY_WINDOW, activityWindowTitle } from "@/lib/activity-window"
import type { ActivityFilter, ActivityWindowData, ActivityWindowFeedback, ActivityWindowRequest, ActivityWindowState, LandedBranchLinks } from "@/lib/activity-window"
import { logError, notifyError, recordError } from "@/lib/errors"
import { formatPushMode } from "@/lib/patronus"
import { repositoryName } from "@/lib/repository-config"
import { createWindowLink } from "@/lib/sub-window-link"
import { activityStore, canUndoActivity, clearActivity, selectActivity, undoActivity } from "@/stores/activity"
import { branchSyncStore } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { dismissLanded, landedStore } from "@/stores/landed"
import { getBranchRobot, patronusStore } from "@/stores/patronus"
import { getPullRequest, pullRequestsStore } from "@/stores/pull-requests"
import { getSelectedProject, repositoryStore } from "@/stores/repository"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/*
 * The main window's side of the Activity window (`@/lib/activity-window`): the log is the main window's (it records,
 * keeps the Undo of this session's rewrites and the landed report), so it tells the window the log of the selected
 * repository whenever it changed, and runs what the window asks for. The window follows the selected repository. The
 * side panel of the main window reads the same state from the stores (`buildActivityState`, `subscribeActivityState`).
 */

interface ActivityShown {
  readonly repositoryPath: string
}

/** The links of the branches the landed entries name, as the island's chips open them */
function landedLinks(state: Pick<ActivityWindowState, "entries">): Record<string, LandedBranchLinks> {
  const links: Record<string, LandedBranchLinks> = {}
  for (const entry of state.entries) {
    if (entry.details?.type !== "landed") {
      continue
    }
    for (const branch of entry.details.report) {
      if (branch.name === null || branch.name in links) {
        continue
      }
      const pullRequest = getPullRequest(branch.name)
      const robot = getBranchRobot(branch.name)
      links[branch.name] = {
        ...(pullRequest !== undefined && { pullRequest: { number: pullRequest.number, url: pullRequest.url } }),
        ...(robot !== undefined && { robot: { url: robot.url, mode: formatPushMode(robot.pushMode) } }),
      }
    }
  }
  return links
}

/** What Activity shows of the repository: its log, the branches of the table, the undismissed landed report, the links */
export function buildActivityState({ repositoryPath }: ActivityShown): ActivityWindowState {
  const entries = selectActivity(activityStore.getState(), repositoryPath)
  const landed = landedStore.getState()
  const branches = branchSyncStore.getState()
  return {
    repositoryPath,
    repositoryName: repositoryName(repositoryPath),
    entries,
    // the rows of another repository a switch left behind name no branch of this one
    branches: branches.syncedRepositoryPath === repositoryPath ? branches.branchOrder : [],
    undismissed: landed.repositoryPath === repositoryPath ? landed.landed.flatMap(branch => branch.commits.map(commit => commit.id)) : [],
    links: landedLinks({ entries }),
  }
}

/** The stores Activity is built from (`buildActivityState`), the selected repository among them */
const ACTIVITY_STORES = [activityStore, branchSyncStore, landedStore, pullRequestsStore, patronusStore, repositoryStore, repositorySettingsStore] as const

/** Calls `listener` on every change of what Activity shows; returns the unsubscribe */
export function subscribeActivityState(listener: () => void): () => void {
  const unsubscribes = ACTIVITY_STORES.map(store => store.subscribe(listener))
  return () => {
    for (const unsubscribe of unsubscribes) {
      unsubscribe()
    }
  }
}

const link = createWindowLink<ActivityShown, ActivityWindowState>({ label: ACTIVITY_WINDOW, buildState: buildActivityState })

/** Opens the Activity window on the log of the selected repository, at the chip of `kind`, or brings it to the front there */
export function openActivityWindow(kind: ActivityFilter | null): void {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined) {
    return
  }
  const shown = { repositoryPath }
  link.setShown(shown)
  const data: ActivityWindowData = { state: link.takeState()!, kind, openedAt: Date.now() }
  link
    .open(shown, { url: "/activity", title: activityWindowTitle(repositoryPath), data, width: 560, height: 720 })
    .catch((error: unknown) => notifyError("Cannot open Activity", error))
}

/** The log changed, or the selected repository: the window shows the log of the one selected now */
export function broadcastActivityState(): void {
  const shown = link.shown()
  const repositoryPath = getSelectedProject()?.path
  if (shown === null || repositoryPath === undefined) {
    return
  }
  if (shown.repositoryPath !== repositoryPath) {
    link.setShown({ repositoryPath })
  }
  link.broadcast()
}

/** What the window asks for, on the log of the repository the main window shows; a request of another one is refused */
export const runActivityRequest = link.serve<ActivityWindowRequest, ActivityWindowFeedback>((request, send) => {
  const refuse = (title: string, reason: string) => {
    send({ title, message: recordError(title, reason) })
    return false
  }
  if (getSelectedProject()?.path !== request.repositoryPath) {
    return refuse("Cannot do it from Activity", "the main window shows another repository now")
  }
  if ("reveal" in request) {
    getCurrentWindow()
      .setFocus()
      .catch((error: unknown) => logError("Cannot focus the main window", error))
    requestReveal({ branchName: request.reveal, commitIds: [] })
    return true
  }
  if ("undo" in request) {
    const entry = activityStore.getState().items.find(it => it.id === request.undo)
    if (entry === undefined || !canUndoActivity(entry)) {
      return refuse(`Cannot undo ${entry?.title ?? "the change"}`, "its time ran out")
    }
    undoActivity(entry)
    return true
  }
  if ("dismiss" in request) {
    return dismissLanded(request.repositoryPath).then(() => true)
  }
  clearActivity(request.repositoryPath)
  return true
})
