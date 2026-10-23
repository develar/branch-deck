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
import { getOperationBlocker, getOperations, runOperation } from "@/stores/operations"
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

/** The steps of an update that joined a Fetch running in its repository: the fetch is that one's */
const APPLY_PLAN: readonly TimedStep[] = ["check", "apply", "sync"]

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

/** The update of each repository, from its fetch to its sync: a second start joins it */
const updates = new Map<string, Promise<void>>()

/** The fetch of the baseline of each repository, of an update or of a Fetch: resolves to whether it fetched */
const fetches = new Map<string, Promise<boolean>>()

/**
 * Fetches the baseline, checks the update and applies it if it is clean (Update Project, ⌘T). A conflict or a block
 * opens the dialog that explains it; nothing changed then. A sync follows every update whose fetch finished: the sync
 * of an applied update is the last step of its progress, the others show on Sync (`syncFetched`). One update at a time
 * in a repository: a second start joins the running one, and a start during the Fetch of a toast goes on from that
 * fetch. The fetch runs first, outside the operations (`fetch_baseline`): the rewrites of the page run meanwhile. Only
 * the check and the apply are a rewrite of the repository (`runOperation`, `update_project` without a fetch): they
 * wait for a running regroup or squash in the queue, and the rewrites wait for them; its progress is the run of this
 * store.
 */
export function runUpdateProject(): Promise<void> {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined) {
    return Promise.resolve()
  }
  const running = updates.get(repositoryPath)
  if (running !== undefined) {
    return running
  }
  const update = updateProjectOf(repositoryPath).finally(() => updates.delete(repositoryPath))
  updates.set(repositoryPath, update)
  return update
}

async function updateProjectOf(repositoryPath: string): Promise<void> {
  // ⌘T during the Fetch of a toast: the update goes on from that fetch, which says itself why it failed
  const joined = fetches.get(repositoryPath)
  if (joined !== undefined && !(await joined)) {
    return
  }
  if (!startRunning(repositoryPath, "update", joined === undefined ? UPDATE_PLAN : APPLY_PLAN)) {
    return
  }
  // the steps that ended; the one that failed says nothing about its usual time
  let done: readonly FinishedStep[] = []
  try {
    if (joined === undefined && !(await fetchOf(repositoryPath, "Cannot update the project"))) {
      return
    }
    const spec = { ...UPDATE_OPERATION, label: "Updating the project", repositoryPath }
    // a rewrite runs: the update waits for it, and says so
    if (getOperationBlocker(getOperations(), spec) !== null) {
      patchRunning(repositoryPath, running => ({ ...running, run: advanceRun(running.run, "waiting", Date.now(), finishedDetail(running.run)) }))
    }
    await runOperation(spec, async () => {
      done = await applyUpdate(repositoryPath)
    })
  } finally {
    endRunning(repositoryPath, done)
  }
}

/**
 * Fetches the baseline of the repository (`fetch_baseline`), its progress in the run of the repository; resolves to
 * whether it fetched, after an error toast titled `failureTitle` if not. An update that starts meanwhile goes on from it
 */
function fetchOf(repositoryPath: string, failureTitle: string): Promise<boolean> {
  const progress = new Channel<TransferProgress>()
  let closed = false
  progress.onmessage = fetch => {
    if (!closed) {
      patchRunning(repositoryPath, running => ({ ...running, run: { ...running.run, fetch } }))
    }
  }
  const fetch = invokeCommand(() => commands.fetchBaseline({ repositoryPath }, progress))
    .then(result => {
      closed = true
      if (result.status !== "ok") {
        notifyError(failureTitle, result.error)
        return false
      }
      updateProject(repositoryPath, { lastFetchTime: Date.now() })
      return true
    })
    .finally(() => fetches.delete(repositoryPath))
  fetches.set(repositoryPath, fetch)
  return fetch
}

/**
 * Checks the update against the fetched baseline and applies it (`update_project` without a fetch), as the operation of
 * the update; resolves to the steps that ended
 */
async function applyUpdate(repositoryPath: string): Promise<readonly FinishedStep[]> {
  const progress = new Channel<UpdateProgress>()
  // a channel can deliver a message after its invoke resolved: the steps after it are the frontend's
  let closed = false
  progress.onmessage = message => {
    if (!closed) {
      patchRunning(repositoryPath, running => ({ ...running, run: applyProgress(running.run, message) }))
    }
  }
  const result = await invokeCommand(() => commands.updateProject({ repositoryPath, fetch: false }, progress))
  closed = true
  const run = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)?.run
  const done = run?.done ?? []
  if (result.status !== "ok") {
    syncFetched(repositoryPath)
    notifyError("Cannot update the project", result.error)
    return done
  }

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
    return done
  }
  // up to date, refused, or another repository selected meanwhile: the update ends here, its sync shows on Sync
  if (applied === null || getSelectedProject()?.path !== repositoryPath) {
    syncFetched(repositoryPath)
    return finishRun(run, Date.now())
  }
  // the branches were rebased: their rows are rebuilt, the last step of the update
  const syncing = advanceRun(run, "sync", Date.now())
  patchRunning(repositoryPath, running => ({ ...running, run: syncing }))
  await requestSync({ autoScroll: false, autoExpand: false }, syncRun => {
    patchRunning(repositoryPath, running => ({ ...running, syncRun }))
  })
  return syncing.done
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
 * "Update Project" of a toast about `repositoryPath` (a merge, a Safe Push refused by the target): the update of that
 * repository while the main window shows it, else only the fetch of its baseline. Update Project updates the shown
 * repository, so a click after a switch never moves the HEAD and the working tree of another one.
 */
export function runUpdateProjectOf(repositoryPath: string): Promise<void> {
  return isSelectedRepository(repositoryPath) ? runUpdateProject() : runFetchBaseline(repositoryPath)
}

/**
 * Fetches only the baseline of the repository, without updating (the Fetch of a merged robot's toast): the button shows
 * the fetch as for an update, then a sync of the selected repository, on Sync, checks the update against the fetched
 * baseline. A Fetch while the update or the fetch of the repository runs joins it; an update started meanwhile goes on
 * from this fetch.
 */
export function runFetchBaseline(repositoryPath: string): Promise<void> {
  const update = updates.get(repositoryPath)
  if (update !== undefined) {
    return update
  }
  const running = fetches.get(repositoryPath)
  if (running !== undefined) {
    return running.then(() => {})
  }
  if (!startRunning(repositoryPath, "fetch", ["fetch"])) {
    return Promise.resolve()
  }
  return fetchOf(repositoryPath, "Cannot fetch the baseline").then(fetched => {
    const run = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)?.run
    const done = !fetched || run === undefined ? [] : finishRun(run, Date.now(), finishedDetail(run))
    endRunning(repositoryPath, done)
    if (fetched) {
      syncFetched(repositoryPath)
    }
  })
}
