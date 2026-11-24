import { requestSafePush } from "@/hooks/useSafePush"
import type { PatronusRobot } from "@/lib/bindings"
import type { EventToastAction } from "@/lib/event-toast"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { foundLandedNews } from "@/lib/landed"
import { keepStartAfterMerge, takeWaitingStart } from "@/lib/merge-dependents"
import { openRobot } from "@/lib/patronus"
import { lastFetchTime } from "@/lib/update-state"
import { getRowPart } from "@/model/store"
import { branchSyncStore } from "@/stores/branch-sync"
import { landedStore } from "@/stores/landed"
import { getSelectedProject, repositoryStore } from "@/stores/repository"
import { getRepositoryState } from "@/stores/settings/repositoryState"
import { runFetchBaseline, runUpdateProjectOf, selectRunningUpdate, updateProjectStore } from "@/stores/update-project"

/**
 * How much earlier than the merge a landing may be found and still be of it (s): Patronus' clock and this one differ
 * by a few seconds; a report of an older landing of a branch of the same name is days old
 */
const LANDED_AFTER_MERGE_SLACK_S = 600

/**
 * The baseline was fetched after `finishTime` (Unix seconds), by Branch Deck or elsewhere (`FETCH_HEAD`,
 * `lastFetchTime`): the header knows already what the update brings in
 */
function isFetchedSince(repositoryPath: string, finishTime: number | null): boolean {
  if (finishTime === null) {
    return false
  }
  const baseline = branchSyncStore.getState().baselineStatus
  const update = baseline?.repositoryPath === repositoryPath ? baseline.update : null
  const fetched = lastFetchTime(update, getRepositoryState(repositoryPath)?.lastFetchTime)
  return fetched !== null && fetched > finishTime * 1000
}

/**
 * What a sync found landed of the branches of the merge, since the merge (an update elsewhere: the IDE, `git pull`,
 * `brd update`): "auth landed in origin/master" / "Found by a sync"; null while the report of the repository has none
 */
function landedSinceMerge(repositoryPath: string, robot: PatronusRobot, branches: ReadonlySet<string>): { title: string; description: string } | null {
  const { repositoryPath: reported, landed, upstream } = landedStore.getState()
  if (reported !== repositoryPath || upstream === null || robot.finishTime === null) {
    return null
  }
  const since = robot.finishTime - LANDED_AFTER_MERGE_SLACK_S
  return foundLandedNews(
    landed.filter(branch => branch.name !== null && branches.has(branch.name) && branch.landedAt >= since),
    upstream,
  )
}

/**
 * Resolves once the step of `key` is decided: no sync runs and it waits for no fact (the needs of the tips the update
 * moved), or the row is gone. The start that waited for a merge is pressed right after the update, while its needs are
 * computed again: the press starts then, never tells the user to press again
 */
function whenStepDecided(key: string): Promise<void> {
  const decided = () => {
    const view = getRowPart(key, "landing")
    return view === undefined || (view.step.kind.kind !== "pending" && !branchSyncStore.getState().isSyncing)
  }
  if (decided()) {
    return Promise.resolve()
  }
  return new Promise(resolve => {
    const unsubscribe = branchSyncStore.subscribe(() => {
      if (decided()) {
        unsubscribe()
        resolve()
      }
    })
  })
}

/**
 * The toast of a merge (docs/contributing/ux-writing.md#one-toast-per-event), one toast from the merge to the landing:
 * - "auth merged into master" / "Safe Push" with Update Project, which fetches the baseline and updates onto it, then
 *   "Open in Patronus"; in a repository the main window doesn't show Fetch instead (Update Project updates the shown
 *   one), and only "Open in Patronus" once its baseline was fetched since the merge;
 * - while an update or a fetch of its repository runs, started from the toast, the header or a key: "Updating…"
 *   ("Fetching…"), disabled, and the toast stays;
 * - the outcome of that update takes its place (`toastKeys.merged`): what landed ("auth landed in origin/master,
 *   squashed" / "Updated master: rebased 104 commits"), or up to date; a run that ended otherwise closes it, its
 *   dialog or its error toast says why;
 * - a landing of its branches that a sync found after an update elsewhere: "auth landed in origin/master" / "Found by a
 *   sync", in place;
 * - once landed, of a subject whose start waited for this robot (`afterLanding`: "Lands after auth"): "Start ui", the
 *   click of its Safe Push once its step is decided again (`whenStepDecided`), which opens its popover when it has
 *   something to say; nothing starts by itself.
 * `landedBranches` are the branches the merge landed besides its own (the ancestors of a stacked branch, every branch of
 * the whole stack).
 */
export function showMergeToast(repositoryPath: string, robot: PatronusRobot, title: string, description: string, landedBranches: readonly string[]): void {
  const branches = new Set(robot.branchName === null ? landedBranches : [robot.branchName, ...landedBranches])
  const openInPatronus: EventToastAction = { label: "Open in Patronus", onClick: () => openRobot(robot) }
  // the start that waited for this robot, once: the toast offers it after the update that lands the merge
  const dependent = takeWaitingStart(robot.id)
  const startDependent: EventToastAction | null =
    dependent === null
      ? null
      : { label: `Start ${dependent}`, onClick: () => void whenStepDecided(dependent).then(() => requestSafePush(dependent, { mode: "rebaseAutosquash", via: "click" })) }
  if (startDependent !== null) {
    // the update that lands the merge says it in this toast's place (`showUpdateToast`), with the start
    keepStartAfterMerge(repositoryPath, startDependent)
  }
  showEventToast({
    key: toastKeys.robot(repositoryPath, robot.id),
    aliases: [toastKeys.merged(repositoryPath)],
    watch: [updateProjectStore, landedStore, repositoryStore],
    closesAfterRun: true,
    render: () => {
      const landed = landedSinceMerge(repositoryPath, robot, branches)
      if (landed !== null) {
        return startDependent === null ? { type: "success", ...landed } : { type: "success", ...landed, action: startDependent, secondaryAction: openInPatronus }
      }
      const running = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
      if (running !== undefined) {
        return { type: "success", title, description, action: { label: running.run.kind === "fetch" ? "Fetching…" : "Updating…", running: true }, secondaryAction: openInPatronus }
      }
      const shown = getSelectedProject()?.path === repositoryPath
      if (!shown && isFetchedSince(repositoryPath, robot.finishTime)) {
        return { type: "success", title, description, action: openInPatronus }
      }
      // the repository of the merge, whichever the main window shows at the click: a fetch only once it is another
      const update: EventToastAction = shown
        ? { label: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) }
        : { label: "Fetch", onClick: () => void runFetchBaseline(repositoryPath) }
      return { type: "success", title, description, action: update, secondaryAction: openInPatronus }
    },
  })
}
