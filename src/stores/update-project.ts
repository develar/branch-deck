import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { toast } from "@/components/ui/toast"
import type { TransferProgress, UpdateCheck, UpdatePhase, UpdateProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { appliedUpdateToast } from "@/lib/landed"
import type { FinishedStep, TimedStep, UpdateRun, UpdateStep } from "@/lib/update-progress"
import { advanceRun, finishRun, learnTimings, startRun } from "@/lib/update-progress"
import { recordUpdate } from "@/stores/activity"
import { requestSync, setBaselineUpdate, syncBranches } from "@/stores/branch-sync"
import { addLandedUpdate } from "@/stores/landed"
import type { OperationKey } from "@/stores/operations"
import { runOperation } from "@/stores/operations"
import { getSelectedProject, isSelectedRepository, useSelectedProjectPath } from "@/stores/repository"
import { getRecentProjects, updateProject } from "@/stores/settings/repositorySettings"

/** How long the bar shows the end of a run (ms), so it reads as done; the buttons work again at once */
export const END_HOLD_MS = 300

/** An update or a fetch of the baseline that runs in a repository, or one that just ended */
export interface RunningUpdate {
  /** The step it is at and its progress (`update-progress.ts`) */
  readonly run: UpdateRun
  /** The sync run (`requestSync`) of the sync of an applied update; null before it starts */
  readonly syncRun: number | null
  /** When the run ended (ms): the bar shows 100 % for `END_HOLD_MS`; null while it runs */
  readonly endedAt: number | null
}

export interface UpdateProjectState {
  /**
   * What runs, by repository path: the Fetch of a merge toast fetches the baseline of its own repository, which need not
   * be the selected one, and the header shows only what runs in the selected repository
   */
  readonly running: Readonly<Record<string, RunningUpdate>>
  /** A conflicting or blocked update to explain; null while the dialog is closed */
  readonly refused: UpdateCheck | null
}

export const updateProjectStore = createStore<UpdateProjectState>(() => ({
  running: {},
  refused: null,
}))

/** What runs in the repository, or ended a moment ago; undefined otherwise, or without a repository */
export function selectShownUpdate(state: UpdateProjectState, repositoryPath: string | undefined): RunningUpdate | undefined {
  return repositoryPath === undefined ? undefined : state.running[repositoryPath]
}

/** What runs in the repository; undefined when nothing does (also for a run that just ended), or without a repository */
export function selectRunningUpdate(state: UpdateProjectState, repositoryPath: string | undefined): RunningUpdate | undefined {
  const shown = selectShownUpdate(state, repositoryPath)
  return shown?.endedAt === null ? shown : undefined
}

/** The update or fetch of the selected repository, also for `END_HOLD_MS` after its end; undefined otherwise */
export function useShownUpdate(): RunningUpdate | undefined {
  const repositoryPath = useSelectedProjectPath()
  return useStore(updateProjectStore, state => selectShownUpdate(state, repositoryPath))
}

export function useRefusedUpdate(): UpdateCheck | null {
  return useStore(updateProjectStore, state => state.refused)
}

export function closeRefusedUpdate(): void {
  updateProjectStore.setState({ refused: null })
}

function setRunning(repositoryPath: string, running: RunningUpdate | null): void {
  const { [repositoryPath]: _previous, ...others } = updateProjectStore.getState().running
  updateProjectStore.setState({ running: running === null ? others : { ...others, [repositoryPath]: running } })
}

function patchRunning(repositoryPath: string, patch: (running: RunningUpdate) => RunningUpdate): void {
  const running = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
  if (running !== undefined) {
    setRunning(repositoryPath, patch(running))
  }
}

function timingsOf(repositoryPath: string) {
  return getRecentProjects().find(it => it.path === repositoryPath)?.updateTimings
}

/** Takes the one-at-a-time slot of the repository for an update or a fetch; false when one runs there already */
function startRunning(repositoryPath: string, kind: UpdateRun["kind"], plan: readonly TimedStep[]): boolean {
  if (selectRunningUpdate(updateProjectStore.getState(), repositoryPath) !== undefined) {
    return false
  }
  setRunning(repositoryPath, { run: startRun(kind, plan, timingsOf(repositoryPath), Date.now()), syncRun: null, endedAt: null })
  return true
}

/**
 * Ends the run of the repository and learns what its steps took (`done`); the bar shows the end for a moment, unless a
 * new run took the slot meanwhile
 */
function endRunning(repositoryPath: string, done: readonly FinishedStep[]): void {
  const running = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
  if (running === undefined) {
    return
  }
  if (done.length > 0) {
    updateProject(repositoryPath, { updateTimings: learnTimings(timingsOf(repositoryPath), done) })
  }
  const ended: RunningUpdate = { ...running, endedAt: Date.now() }
  setRunning(repositoryPath, ended)
  setTimeout(() => {
    if (updateProjectStore.getState().running[repositoryPath] === ended) {
      setRunning(repositoryPath, null)
    }
  }, END_HOLD_MS)
}

const PHASE_STEPS: Readonly<Record<UpdatePhase, UpdateStep>> = {
  fetching: "fetch",
  waiting: "waiting",
  checking: "check",
  updating: "apply",
}

/**
 * Every step of an update, listed from its start whatever the last check found: the files follow, and the sync rebuilds
 * the rebased branches. A check that finds nothing to apply ends the run with its result, the steps after it never run.
 */
const UPDATE_PLAN: readonly TimedStep[] = ["fetch", "check", "apply", "sync"]

/** What a step that ends did: the amount of the fetch, as git prints it */
function finishedDetail(run: UpdateRun): string | null {
  return run.step === "fetch" ? (run.fetch?.transferred ?? null) : null
}

/** The run at the step of the message: a new step only when the phase changes, the fetch progress within the fetch */
function applyProgress(run: UpdateRun, { phase, fetch }: UpdateProgress): UpdateRun {
  const step = PHASE_STEPS[phase]
  if (step === run.step) {
    return step === "fetch" ? { ...run, fetch } : run
  }
  return advanceRun(run, step, Date.now(), finishedDetail(run))
}

/** The operation of an update of the selected repository: a rewrite of it, as a regroup or a squash */
export const UPDATE_OPERATION: OperationKey = { id: "update-project", subject: "repository", objects: [], kind: "rewrite" }

/**
 * Fetches the baseline, checks the update and applies it if it is clean (Update Project, ⌘T). A conflict or a block
 * opens the dialog that explains it; nothing changed then. A sync follows every update whose fetch finished: the sync
 * of an applied update is the last step of its progress, the others show on Sync (`syncFetched`). One update at a time
 * in a repository. A rewrite of the repository (`runOperation`): it waits for a
 * running regroup or squash, said in a toast, and they wait for it; its progress is the run of this store.
 */
export async function runUpdateProject(): Promise<void> {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined) {
    return
  }
  await runOperation(
    { ...UPDATE_OPERATION, label: "Updating the project", repositoryPath },
    () => updateProjectOf(repositoryPath),
    reason => toast.add({ type: "info", title: "Cannot update the project", description: reason }),
  )
}

async function updateProjectOf(repositoryPath: string): Promise<void> {
  if (!startRunning(repositoryPath, "update", UPDATE_PLAN)) {
    return
  }
  // the steps that ended; the one that failed says nothing about its usual time
  let done: readonly FinishedStep[] = []
  try {
    const progress = new Channel<UpdateProgress>()
    // the fetch is done once the check starts: an error after it (e.g. bd holds the repository) keeps the fetch time
    let fetched = false
    // a channel can deliver a message after its invoke resolved: the steps after it are the frontend's
    let closed = false
    progress.onmessage = message => {
      if (closed) {
        return
      }
      fetched ||= message.phase !== "fetching"
      patchRunning(repositoryPath, running => ({ ...running, run: applyProgress(running.run, message) }))
    }
    const result = await invokeCommand(() => commands.updateProject({ repositoryPath }, progress))
    closed = true
    const run = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)?.run
    done = run?.done ?? []
    if (result.status !== "ok") {
      if (fetched) {
        updateProject(repositoryPath, { lastFetchTime: Date.now() })
        syncFetched(repositoryPath)
      }
      notifyError("Cannot update the project", result.error)
      return
    }

    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    const { check, applied, landed } = result.data
    recordUpdate(repositoryPath, result.data)
    switch (check.state.status) {
      case "upToDate":
        setBaselineUpdate(repositoryPath, check)
        toast.add({ type: "info", title: `${check.branch} is already up to date with ${check.upstream}` })
        break
      case "clean":
        if (applied !== null) {
          setBaselineUpdate(repositoryPath, { ...check, head: applied.newHead, state: { status: "upToDate" } })
          // the island of what landed shows at once, the sync that follows confirms it
          addLandedUpdate(repositoryPath, landed)
          toast.add({ type: "success", ...appliedUpdateToast(check, applied, landed) })
        }
        break
      case "conflict":
      case "blocked":
        setBaselineUpdate(repositoryPath, check)
        updateProjectStore.setState({ refused: check })
        break
    }
    if (run === undefined) {
      return
    }
    // up to date, refused, or another repository selected meanwhile: the update ends here, its sync shows on Sync
    if (applied === null || getSelectedProject()?.path !== repositoryPath) {
      done = finishRun(run, Date.now())
      syncFetched(repositoryPath)
      return
    }
    // the branches were rebased: their rows are rebuilt, the last step of the update
    const syncing = advanceRun(run, "sync", Date.now())
    done = syncing.done
    patchRunning(repositoryPath, running => ({ ...running, run: syncing }))
    await requestSync({ autoScroll: false, autoExpand: false }, syncRun => {
      patchRunning(repositoryPath, running => ({ ...running, syncRun }))
    })
  } finally {
    endRunning(repositoryPath, done)
  }
}

/**
 * After a fetch the branches are built again: the baseline moved (integration, remote status, what landed) also when the
 * update is up to date, conflicts or is refused. The table shows the selected repository: another one selected meanwhile
 * has nothing to sync for this fetch.
 */
function syncFetched(repositoryPath: string): void {
  if (getSelectedProject()?.path === repositoryPath) {
    void syncBranches({ autoScroll: false, autoExpand: false })
  }
}

/**
 * Fetches only the baseline of the repository, without updating (the Fetch of a merged robot's toast): the button shows
 * the fetch as for an update, then a sync of the selected repository, on Sync, checks the update against the fetched
 * baseline. Shares the one-at-a-time slot of the update in the repository.
 */
/**
 * "Update Project" of a toast about `repositoryPath` (a merge, a Safe Push refused by the target): the update of that
 * repository while the main window shows it, else only the fetch of its baseline. Update Project updates the shown
 * repository, so a click after a switch never moves the HEAD and the working tree of another one.
 */
export function runUpdateProjectOf(repositoryPath: string): Promise<void> {
  return isSelectedRepository(repositoryPath) ? runUpdateProject() : runFetchBaseline(repositoryPath)
}

export async function runFetchBaseline(repositoryPath: string): Promise<void> {
  if (!startRunning(repositoryPath, "fetch", ["fetch"])) {
    return
  }
  let done: readonly FinishedStep[] = []
  try {
    const progress = new Channel<TransferProgress>()
    let closed = false
    progress.onmessage = fetch => {
      if (!closed) {
        patchRunning(repositoryPath, running => ({ ...running, run: { ...running.run, fetch } }))
      }
    }
    const result = await invokeCommand(() => commands.fetchBaseline({ repositoryPath }, progress))
    closed = true
    if (result.status !== "ok") {
      notifyError("Cannot fetch the baseline", result.error)
      return
    }
    const run = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)?.run
    done = run === undefined ? [] : finishRun(run, Date.now(), finishedDetail(run))
    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    syncFetched(repositoryPath)
  } finally {
    endRunning(repositoryPath, done)
  }
}
