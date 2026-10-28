// Update Project (⌘T) and the Fetch of a merge toast are operations of the session of their repository
// (`operations::update`, the intents `updateProject` and `fetchBaseline`; docs/design/operations.md, "Update Project"):
// the session joins a second start, a Fetch during the fetch of an update joins that fetch, and an update during a
// Fetch goes on from it without a second transfer; the update holds HEAD only for its check and its move, so a rewrite
// runs during its fetch, and one clicked during its check waits behind it. Rust decides all of it and asks the sync
// after the fetch and the move; this store keeps the run each window shows of its repository, its steps as the channel
// of its `execute` tells them, and words its outcome once.
import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { ApiError, Intent, OperationEvent, OperationId, OperationStep, Outcome, UpdateCheck, UpdateProjectResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { EventToastState } from "@/lib/event-toast"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { appliedUpdateToast } from "@/lib/landed"
import { executeIntent } from "@/lib/run-intent"
import { showToast } from "@/lib/toast-message"
import type { TimedStep, UpdateRun, UpdateStep } from "@/lib/update-progress"
import { advanceRun, finishRun, learnTimings, startRun } from "@/lib/update-progress"
import { nextRunNumber, whenSessionViewAt } from "@/model/store"
import { canceledBeforeStart, presentLockWait } from "@/presenters/lock-wait"
import { presentRunningLabel } from "@/presenters/operations"
import { recordUpdate } from "@/stores/activity"
import { branchSyncStore, setBaselineUpdate } from "@/stores/branch-sync"
import { addLandedUpdate } from "@/stores/landed"
import { getSelectedProject, isSelectedRepository, useSelectedProjectPath } from "@/stores/repository"
import { getRepositoryState, updateRepositoryState } from "@/stores/settings/repositoryState"

/** How long the bar shows the end of a run (ms), so it reads as done; the buttons work again at once */
export const END_HOLD_MS = 300

/** An update or a fetch of the baseline that runs in a repository, or one that just ended */
export interface RunningUpdate {
  /** The step it is at and its progress (`update-progress.ts`) */
  readonly run: UpdateRun
  /** The sync run (`nextRunNumber`) of the sync of an applied update; null before it starts */
  readonly syncRun: number | null
  /** When the run ended (ms): the bar shows 100 % for `END_HOLD_MS`; null while it runs */
  readonly endedAt: number | null
  /** An update started during the Fetch of a toast: it goes on from that fetch, its run starts with its check once the fetch ended */
  readonly thenUpdate?: boolean
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

/**
 * The busy state of the operation of this app that holds HEAD as the view of the session shows it ("Unstacking oauth…"):
 * an update started now waits for it in the queue of the session after its fetch; null while none does
 */
export function useHeadWait(): string | null {
  return useStore(branchSyncStore, state => {
    const holder = Object.values(state.operations).findLast(operation => operation.origin.kind === "thisProcess" && operation.resources.some(resource => resource.kind === "head"))
    return holder === undefined ? null : `${presentRunningLabel(holder)}…`
  })
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

/** The durations the steps of the repository took before (`repositoryState`), which the bar of a run divides by */
function timingsOf(repositoryPath: string) {
  return getRepositoryState(repositoryPath)?.updateTimings
}

/**
 * Every step of an update, listed from its start whatever the last check found: the files follow, and the sync rebuilds
 * the rebased branches. A check that finds nothing to apply ends the run with its result, the steps after it never run.
 */
const UPDATE_PLAN: readonly TimedStep[] = ["fetch", "check", "apply", "sync"]

/** The steps of an update without a fetch (Retry Without Fetch) */
const APPLY_PLAN: readonly TimedStep[] = ["check", "apply", "sync"]

/** What a step that ends did: the amount of the fetch, as git prints it */
function finishedDetail(run: UpdateRun): string | null {
  return run.step === "fetch" ? (run.fetch?.transferred ?? null) : null
}

/**
 * Ends the run of `kind` of the repository and learns what its steps took, the one that `failed` excepted, which says
 * nothing about its usual time; the bar shows the end for a moment, unless a new run took its place meanwhile. A run
 * that became another (a Fetch that goes on as an update) goes on
 */
function endRunning(repositoryPath: string, kind: UpdateRun["kind"], failed: boolean): void {
  const running = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
  if (running?.run.kind !== kind) {
    return
  }
  const done = failed ? running.run.done : finishRun(running.run, Date.now(), finishedDetail(running.run))
  if (done.length > 0) {
    updateRepositoryState(repositoryPath, state => ({ ...state, updateTimings: learnTimings(state.updateTimings, done) }))
  }
  if (running.thenUpdate === true && !failed) {
    setRunning(repositoryPath, { run: startRun("update", APPLY_PLAN, timingsOf(repositoryPath), Date.now()), syncRun: null, endedAt: null })
    return
  }
  const ended: RunningUpdate = { ...running, endedAt: Date.now() }
  setRunning(repositoryPath, ended)
  setTimeout(() => {
    if (updateProjectStore.getState().running[repositoryPath] === ended) {
      setRunning(repositoryPath, null)
    }
  }, END_HOLD_MS)
}

/** The step of the run an operation of the update is at; null for a step an update never takes */
function updateStepOf(step: OperationStep): UpdateStep | null {
  switch (step.kind) {
    case "fetching":
      return "fetch"
    case "queued":
    case "waitingForLock":
      return "waiting"
    case "checking":
      return "check"
    case "updating":
      return "apply"
    default:
      return null
  }
}

/**
 * The run of the repository at the step its operation `op` went on to: a new step only when it changes, and whom the
 * wait is for while another holder has the repository lock, with its Stop Waiting (`cancel_operation`); the fetch
 * finished once the update goes on from it, which the state of the repository keeps (the time of its last fetch)
 */
function stepRun(repositoryPath: string, op: OperationId, step: OperationStep): void {
  const next = updateStepOf(step)
  if (next === null) {
    return
  }
  patchRunning(repositoryPath, running => {
    const { run } = running
    if (run.step === "fetch" && next !== "fetch") {
      updateRepositoryState(repositoryPath, state => ({ ...state, lastFetchTime: Date.now() }))
    }
    const moved = next === run.step ? run : advanceRun(run, next, Date.now(), finishedDetail(run))
    if (step.kind !== "waitingForLock") {
      return { ...running, run: moved }
    }
    const stop = () => void commands.cancelOperation({ op }).catch(() => {})
    return { ...running, run: { ...moved, lockWait: { waitingFor: presentLockWait(step.holder), stop } } }
  })
}

/**
 * The channel of an `execute` of `kind` in the repository: its steps and its fetch progress move the run of that kind (a
 * Fetch that joined an update moves nothing), its outcome is told once; a message after the outcome changes nothing,
 * the steps after it are the page's
 */
function runChannel(repositoryPath: string, kind: UpdateRun["kind"]): Channel<OperationEvent> {
  const events = new Channel<OperationEvent>()
  let ended = false
  const own = () => !ended && selectRunningUpdate(updateProjectStore.getState(), repositoryPath)?.run.kind === kind
  events.onmessage = event => {
    if (event.kind === "done") {
      ended = true
      tellOutcome(repositoryPath, event.outcome)
    } else if (!own()) {
      return
    } else if (event.kind === "step") {
      stepRun(repositoryPath, event.op, event.step)
    } else {
      const fetched = { percent: event.percent, line: event.line, transferred: event.transferred, speed: event.speed }
      patchRunning(repositoryPath, running => (running.run.step === "fetch" ? { ...running, run: { ...running.run, fetch: fetched } } : running))
    }
  }
  return events
}

/** The outcomes this window told, by operation: a start that joined another hears the same outcome; the newest kept */
const told: OperationId[] = []

/** Whether the outcome of `op` was told already; it is from now on */
function toldBefore(op: OperationId): boolean {
  if (told.includes(op)) {
    return true
  }
  told.push(op)
  told.splice(0, Math.max(0, told.length - 20))
  return false
}

/** The error of the first effect of `outcome` that failed; null when none failed */
function failureOf(outcome: Outcome): ApiError | null {
  for (const { status } of outcome.effects) {
    if (status.kind === "failed") {
      return status.error
    }
  }
  return null
}

/**
 * Tells what an update or a Fetch of the repository did, once per operation: the failure of its fetch or of its update,
 * or the result of the update; an applied update of the selected repository goes on with its sync, the last step of its
 * run, the others end their run here
 */
function tellOutcome(repositoryPath: string, outcome: Outcome): void {
  if (toldBefore(outcome.op)) {
    return
  }
  const isUpdate = outcome.effects.some(record => record.effect.kind === "update")
  const fetched = outcome.effects.some(record => record.effect.kind === "fetch" && record.status.kind === "done")
  if (fetched) {
    updateRepositoryState(repositoryPath, state => ({ ...state, lastFetchTime: Date.now() }))
  }
  const failure = failureOf(outcome)
  if (failure !== null) {
    tellFailure(repositoryPath, failure, isUpdate ? "update" : "fetch")
    endRunning(repositoryPath, isUpdate ? "update" : "fetch", true)
    return
  }
  if (!isUpdate || outcome.result.kind !== "update") {
    endRunning(repositoryPath, "fetch", false)
    return
  }
  const result: UpdateProjectResult = outcome.result
  tellUpdate(repositoryPath, result)
  // up to date, refused, or another repository selected meanwhile: the update ends here, its sync shows on Sync
  if (result.applied === null || getSelectedProject()?.path !== repositoryPath) {
    endRunning(repositoryPath, "update", false)
    return
  }
  // the branches were rebased: their rows are rebuilt, the last step of the update
  const syncRun = nextRunNumber()
  patchRunning(repositoryPath, running => ({ ...running, run: advanceRun(running.run, "sync", Date.now()), syncRun }))
}

/** The result of an update: its toast, the state of the baseline it leaves, what landed, the dialog of a refusal */
function tellUpdate(repositoryPath: string, result: UpdateProjectResult): void {
  const { check, applied, landed } = result
  recordUpdate(repositoryPath, result)
  switch (check.state.status) {
    case "upToDate":
      setBaselineUpdate(repositoryPath, check)
      showUpdateToast(repositoryPath, { type: "info", title: `${check.branch} is already up to date with ${check.upstream}` })
      break
    case "clean":
      if (applied !== null) {
        setBaselineUpdate(repositoryPath, { ...check, head: applied.newHead, state: { status: "upToDate" } })
        showUpdateToast(repositoryPath, { type: "success", ...appliedUpdateToast(check, applied, landed) })
        // the island of what landed shows at once, the sync that follows confirms it
        addLandedUpdate(repositoryPath, landed, check.upstream)
      }
      break
    case "conflict":
    case "blocked":
      setBaselineUpdate(repositoryPath, check)
      updateProjectStore.setState({ refused: check })
      break
  }
}

/**
 * A failure of an update or of a Fetch (`kind`): a stopped wait for the lock says so in an info toast, nothing changed
 * but the fetch; an error of an update that the same request may pass (`retry: same`, a held lock) offers "Retry
 * Without Fetch", the check and the move against the baseline the last fetch got; a moved branch is read again by a
 * new Update
 */
function tellFailure(repositoryPath: string, error: ApiError, kind: UpdateRun["kind"]): void {
  if (error.code === "canceled") {
    showToast(canceledBeforeStart("the update", "Nothing changed; the fetch stays"))
    return
  }
  const update = kind === "update"
  const retry = update && error.retry === "same" ? { children: "Retry Without Fetch", onClick: () => void retryUpdateWithoutFetch(repositoryPath) } : undefined
  notifyError(update ? "Cannot update the project" : "Cannot fetch the baseline", error, retry)
}

/**
 * The outcome of an update: the merges of the repository it brought in were told by toasts of their own, whose Update
 * Project is done now, so it takes the place of theirs (`showMergeToast`): "auth merged into master" becomes "auth
 * landed in origin/master" in place, never a second toast of the same landing
 */
function showUpdateToast(repositoryPath: string, state: EventToastState): void {
  showEventToast({ key: toastKeys.update(repositoryPath), supersedes: [toastKeys.merged(repositoryPath)], render: () => state })
}

/**
 * The run of `kind` of the repository from now on: a new one when none runs; a start of an update during the Fetch of a
 * toast goes on from that fetch, its run once the fetch ended (`thenUpdate`); any other start joins the run that runs,
 * as its operation joins in the session. Whether the run is this start's: only its answer ends the run after a sync (a
 * request that joined answers when the operation ended, before its sync)
 */
function begin(repositoryPath: string, kind: UpdateRun["kind"], plan: readonly TimedStep[]): boolean {
  const running = selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
  if (running === undefined) {
    setRunning(repositoryPath, { run: startRun(kind, plan, timingsOf(repositoryPath), Date.now()), syncRun: null, endedAt: null })
    return true
  }
  if (running.run.kind === "fetch" && kind === "update") {
    setRunning(repositoryPath, { ...running, thenUpdate: true })
    return true
  }
  return false
}

/** Runs `intent` of the family in the repository as a run of `kind` (`execute`), resolving once it answered */
async function runIntentOf(repositoryPath: string, intent: Intent, kind: UpdateRun["kind"], plan: readonly TimedStep[]): Promise<void> {
  const own = begin(repositoryPath, kind, plan)
  const executed = await executeIntent({ repositoryPath, intent, fingerprint: "", accepted: [] }, runChannel(repositoryPath, kind))
  if (executed.status === "error") {
    // refused before its first effect: nothing was fetched or changed
    tellFailure(repositoryPath, executed.error, kind)
    endRunning(repositoryPath, kind, true)
    return
  }
  // an outcome whose `done` the channel lost is told now, once
  tellOutcome(repositoryPath, executed.data)
  if (own) {
    // an applied update answers once its sync ended: the run ends when the window shows what the sync rebuilt
    await whenSessionViewAt(repositoryPath, executed.data.versionAfter)
    endRunning(repositoryPath, kind, false)
  }
}

/**
 * Fetches the baseline, checks the update and applies it if it is clean (Update Project, ⌘T), in the session of the
 * selected repository. A conflict or a block opens the dialog that explains it; nothing changed then. The session asks
 * a sync after every update whose fetch finished: the sync of an applied update is the last step of its progress, the
 * others show on Sync. A second start joins the running update, and a start during the Fetch of a toast goes on from
 * that fetch
 */
export function runUpdateProject(): Promise<void> {
  const repositoryPath = getSelectedProject()?.path
  return repositoryPath === undefined ? Promise.resolve() : runIntentOf(repositoryPath, { kind: "updateProject", fetch: true }, "update", UPDATE_PLAN)
}

/**
 * "Retry Without Fetch" of an update refused after its fetch by an error the same request may pass (`retry: same`, a
 * held repository lock): the check and the apply against the baseline that fetch got. Only while the main window shows
 * the repository, as Update Project: a click after a switch never moves the HEAD of another one
 */
export function retryUpdateWithoutFetch(repositoryPath: string): Promise<void> {
  return isSelectedRepository(repositoryPath) ? runIntentOf(repositoryPath, { kind: "updateProject", fetch: false }, "update", APPLY_PLAN) : Promise.resolve()
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
 * the fetch as for an update, then the sync the session asks checks the update against the fetched baseline. A Fetch
 * while the update or the fetch of the repository runs joins it in the session; an update started meanwhile goes on
 * from this fetch.
 */
export function runFetchBaseline(repositoryPath: string): Promise<void> {
  return runIntentOf(repositoryPath, { kind: "fetchBaseline" }, "fetch", ["fetch"])
}
