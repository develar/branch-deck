import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { ApiError, EffectStatus, Intent, OperationEvent, OperationStep, Outcome, PlannedEffect, UpdateCheck, UpdateProjectResult } from "@/lib/bindings"
import { resetEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import type { ToastActionProps } from "@/lib/toast-message"
import { retryUpdateWithoutFetch, runFetchBaseline, runUpdateProject, runUpdateProjectOf, selectRunningUpdate, updateProjectStore } from "@/stores/update-project"
import { openToasts, resetToasts, toasts } from "@/test-support/toast-mock"
import { conflictState } from "@/test-support/update-conflict"

/** An `execute` of the page that runs until the test answers it */
interface Execution {
  readonly intent: Intent
  readonly repositoryPath: string
  readonly events: { onmessage: (event: OperationEvent) => void }
  readonly answer: (result: unknown) => void
}

const { backend, selection, fetchTimes, timings, canceled } = vi.hoisted(() => ({
  backend: { executions: [] as Execution[] },
  selection: { path: "/a" },
  fetchTimes: [] as string[],
  timings: [] as { path: string; timings: unknown }[],
  canceled: [] as string[],
}))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    execute: (params: { repositoryPath: string; intent: Intent }, events: Execution["events"]) =>
      new Promise(resolve => {
        backend.executions.push({ intent: params.intent, repositoryPath: params.repositoryPath, events, answer: resolve })
      }),
    cancelOperation: async ({ op }: { op: string }) => {
      canceled.push(op)
      return { status: "ok", data: true }
    },
  },
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/model/store", () => ({ nextRunNumber: () => 7, whenSessionViewAt: async () => {} }))
vi.mock("@/stores/branch-sync", () => ({ branchSyncStore: { getState: () => ({ operations: {} }), subscribe: () => () => {} }, setBaselineUpdate: () => {} }))
vi.mock("@/stores/landed", () => ({ addLandedUpdate: () => {} }))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)
// the time of a fetch and the durations a run learns go to the state of the repository, never to the recent projects
vi.mock("@/stores/settings/repositoryState", () => ({
  getRepositoryState: () => undefined,
  updateRepositoryState: (path: string, update: (state: { updateTimings?: unknown; lastFetchTime?: number }) => { updateTimings?: unknown; lastFetchTime?: number }) => {
    const state = update({})
    if (state.lastFetchTime === undefined) {
      timings.push({ path, timings: state.updateTimings })
    } else {
      fetchTimes.push(path)
    }
  },
}))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selection.path }),
  isSelectedRepository: (path: string) => path === selection.path,
  useSelectedProjectPath: () => selection.path,
}))

const PROGRESS = { percent: 45, transferred: "5.79 MiB", speed: "32.57 MiB/s", line: "Receiving objects:  45% (136/302)" }

const UP_TO_DATE: UpdateProjectResult = {
  check: { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", state: { status: "upToDate" }, fetchedAt: null } satisfies UpdateCheck,
  applied: null,
  landed: [],
}

const APPLIED: UpdateProjectResult = {
  check: { ...UP_TO_DATE.check, state: { status: "clean", kind: "rebase" } },
  applied: { kind: "rebase", newHead: "n", replayed: 2, droppedAlreadyUpstream: 0 },
  landed: [],
}

const FETCH: PlannedEffect = { kind: "fetch", upstream: "origin/master" }
const DONE: EffectStatus = { kind: "done" }
const SKIPPED: EffectStatus = { kind: "skipped", reason: null }

/** The number of the test that runs: the operations of a test are its own, as their ids are in the app */
let testRun = 0

/** The id of the operation `op` of the test that runs */
function opId(op: string): string {
  return `${op}-${testRun}`
}

/** The outcome of the operation `op`: each effect with its status, what it answered */
function outcome(op: string, effects: readonly (readonly [PlannedEffect, EffectStatus])[], result: Outcome["result"] = { kind: "none" }): Outcome {
  return {
    op: opId(op),
    effects: effects.map(([effect, status]) => ({ effect, status })),
    newsKey: null,
    versionAfter: null,
    result,
    undo: null,
    stats: { gitProcesses: {}, httpRequests: 0, millis: 0 },
  }
}

/** The outcome of an update `op` that fetched and answered `result` */
function updated(op: string, result: UpdateProjectResult): Outcome {
  return outcome(
    op,
    [
      [FETCH, DONE],
      [{ kind: "update" }, DONE],
      [{ kind: "sync" }, DONE],
    ],
    { kind: "update", ...result },
  )
}

/** The outcome of an update `op` whose check or move failed with `error` after its fetch */
function failedAfterTheFetch(op: string, error: ApiError): Outcome {
  return outcome(op, [
    [FETCH, DONE],
    [{ kind: "update" }, { kind: "failed", error }],
    [{ kind: "sync" }, DONE],
  ])
}

/** The outcome of a Fetch `op` */
function fetchedBy(op: string): Outcome {
  return outcome(
    op,
    [
      [FETCH, DONE],
      [{ kind: "sync" }, DONE],
    ],
    { kind: "fetch", upstream: "origin/master", fetchedAt: 1, transferred: "5.79 MiB" },
  )
}

/** The toast shown last */
function lastToast() {
  return toasts.at(-1)
}

function runningIn(repositoryPath: string) {
  return selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
}

/** The step the run of the repository is at, with the fetch progress it holds; undefined when none runs */
function stepIn(repositoryPath: string) {
  const running = runningIn(repositoryPath)
  return running === undefined ? undefined : { step: running.run.step, fetch: running.run.fetch }
}

/** The `execute` of `kind` (the intent) of the repository, the `at`-th of them, once the page sent it */
async function executionIn(repositoryPath: string, kind: Intent["kind"], at = 0): Promise<Execution> {
  const find = () => backend.executions.filter(it => it.repositoryPath === repositoryPath && it.intent.kind === kind)[at]
  await vi.waitFor(() => expect(find()).toBeDefined())
  return find()!
}

/** The operation `op` of `execution` went on to `step` */
function step(execution: Execution, op: string, to: OperationStep): void {
  execution.events.onmessage({ kind: "step", op: opId(op), step: to })
}

/** `execution` ends with `outcome`: its `done`, then its answer */
function end(execution: Execution, ended: Outcome): void {
  execution.events.onmessage({ kind: "done", outcome: ended })
  execution.answer({ status: "ok", data: ended })
}

/** The intents the page sent, in their order */
function intents(): string[] {
  return backend.executions.map(it => `${it.intent.kind === "updateProject" ? `update${it.intent.fetch ? "" : " without fetch"}` : it.intent.kind} ${it.repositoryPath}`)
}

beforeEach(() => {
  testRun++
  updateProjectStore.setState({ running: {}, refused: null })
  backend.executions = []
  selection.path = "/a"
  fetchTimes.length = 0
  timings.length = 0
  canceled.length = 0
  resetEventToasts()
  resetToasts()
})

describe("the running update", () => {
  it("shows the steps of its operation: the fetch with git's progress, the wait behind a rewrite, then the check", async () => {
    const update = runUpdateProject()
    const execution = await executionIn("/a", "updateProject")
    expect(execution.intent).toEqual({ kind: "updateProject", fetch: true })
    execution.events.onmessage({ kind: "progress", op: "u", ...PROGRESS })
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })
    // the fetch ended while a rewrite holds HEAD: the session queues the check behind it
    step(execution, "u", { kind: "queued", behind: "r" })
    expect(stepIn("/a")).toEqual({ step: "waiting", fetch: null })
    expect(fetchTimes).toEqual(["/a"])
    step(execution, "u", { kind: "checking" })
    expect(stepIn("/a")?.step).toBe("check")
    end(execution, updated("u", UP_TO_DATE))
    await update
    expect(runningIn("/a")).toBeUndefined()
  })

  it("names whom it waits for while another holder has the repository lock, and Stop Waiting cancels its operation", async () => {
    const update = runUpdateProject()
    const execution = await executionIn("/a", "updateProject")
    const holder = { tool: "bd" as const, command: "bd pick --follow", pid: 4242, held_for: 12 }
    step(execution, "u", { kind: "waitingForLock", holder })
    const wait = runningIn("/a")?.run.lockWait
    expect(wait?.waitingFor).toBe("Waiting for `bd pick --follow` (pid 4242, for 12 s)")
    wait!.stop()
    await vi.waitFor(() => expect(canceled).toEqual([opId("u")]))
    const stopped = { ...internalError("The wait for the repository lock was stopped; nothing changed"), code: "canceled" as const }
    end(execution, failedAfterTheFetch("u", stopped))
    await update
    expect(lastToast()).toMatchObject({ type: "info", title: "Canceled the update before it started" })
  })

  it("belongs to its repository: switching repositories never shows the other repository's phase", async () => {
    // the Fetch of a merge toast of /a
    const fetch = runFetchBaseline("/a")
    const fetching = await executionIn("/a", "fetchBaseline")
    fetching.events.onmessage({ kind: "progress", op: "f", ...PROGRESS })
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    selection.path = "/b"
    expect(runningIn("/b")).toBeUndefined()

    // /b updates while /a fetches
    const update = runUpdateProject()
    const updating = await executionIn("/b", "updateProject")
    step(updating, "u", { kind: "checking" })
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    end(fetching, fetchedBy("f"))
    await fetch
    expect(runningIn("/a")).toBeUndefined()
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })

    end(updating, updated("u", UP_TO_DATE))
    await update
    expect(runningIn("/b")).toBeUndefined()
  })

  it("updates the repository a toast is about only while it is shown, else only fetches it: never the one shown since", async () => {
    // Update Project of a toast of /a while /a is shown: its update
    const update = runUpdateProjectOf("/a")
    end(await executionIn("/a", "updateProject"), updated("u", UP_TO_DATE))
    await update
    expect(intents()).toEqual(["update /a"])

    // the same toast clicked after a switch to /b: the fetch of /a, nothing of /b moves
    backend.executions = []
    selection.path = "/b"
    const fetch = runUpdateProjectOf("/a")
    end(await executionIn("/a", "fetchBaseline"), fetchedBy("f"))
    await fetch
    expect(intents()).toEqual(["fetchBaseline /a"])
    expect(runningIn("/b")).toBeUndefined()
  })

  it("keeps the fetch time when the update fails after its fetch, e.g. while bd holds the repository", async () => {
    const failedFetch = runUpdateProject()
    const network = { ...internalError("Failed to fetch origin/master"), code: "network_error" as const }
    end(
      await executionIn("/a", "updateProject"),
      outcome("u", [
        [FETCH, { kind: "failed", error: network }],
        [{ kind: "update" }, SKIPPED],
        [{ kind: "sync" }, SKIPPED],
      ]),
    )
    await failedFetch
    expect(fetchTimes).toEqual([])
    expect(lastToast()).toMatchObject({ title: "Cannot update the project", description: "Failed to fetch origin/master" })

    backend.executions = []
    const failed = runUpdateProject()
    const locked = { ...internalError("`bd push auth` (pid 4242) is changing this repository; try again when it is done"), code: "locked" as const, retry: "same" as const }
    end(await executionIn("/a", "updateProject"), failedAfterTheFetch("v", locked))
    await failed
    expect(fetchTimes).toEqual(["/a"])
  })

  it("offers Retry Without Fetch for a held lock after the fetch, which checks and applies without a second fetch; a moved branch gets none", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {})
    const locked = runUpdateProject()
    const held: ApiError = {
      code: "locked",
      message: "`bd push auth` (pid 4242) is changing this repository; try again when it is done",
      details: { type: "locked", holder: { tool: "bd", command: "bd push auth", pid: 4242, held_for: 40 } },
      retry: "same",
      operation_id: opId("u"),
    }
    end(await executionIn("/a", "updateProject"), failedAfterTheFetch("u", held))
    await locked
    expect(lastToast()).toMatchObject({ title: "Cannot update the project", description: "`bd push auth` (pid 4242) has held the repository for 40 s" })
    // the action as `notifyError` hands it over
    const retry = lastToast()?.actionProps as ToastActionProps | undefined
    expect(retry?.children).toBe("Retry Without Fetch")

    backend.executions = []
    retry!.onClick()
    // a second retry joins the first: one run, both answered by the operation the session joined them in
    const again = retryUpdateWithoutFetch("/a")
    const first = await executionIn("/a", "updateProject")
    const second = await executionIn("/a", "updateProject", 1)
    // the check and the apply only: the fetch of the refused update stands
    expect(first.intent).toEqual({ kind: "updateProject", fetch: false })
    expect(runningIn("/a")?.run.plan).toEqual(["check", "apply", "sync"])
    const retried = outcome(
      "w",
      [
        [{ kind: "update" }, DONE],
        [{ kind: "sync" }, SKIPPED],
      ],
      { kind: "update", ...UP_TO_DATE },
    )
    end(first, retried)
    end(second, retried)
    await again
    expect(intents()).toEqual(["update without fetch /a", "update without fetch /a"])

    // the branch moved: the request is read again, not repeated
    backend.executions = []
    const moved = runUpdateProject()
    const headMoved = { ...internalError("The branch moved"), code: "head_moved" as const, retry: "after_reread" as const }
    end(await executionIn("/a", "updateProject"), failedAfterTheFetch("x", headMoved))
    await moved
    expect(lastToast()).toMatchObject({ title: "Cannot update the project", description: "The branch moved" })
    expect(lastToast()?.actionProps).toBeUndefined()
  })

  it("retries without a fetch only the repository the main window shows", async () => {
    selection.path = "/b"
    await retryUpdateWithoutFetch("/a")
    expect(intents()).toEqual([])
  })

  it("runs one run at a time in a repository: a second start joins it, and an update during a Fetch goes on from that fetch with its check", async () => {
    const fetch = runFetchBaseline("/a")
    const again = runFetchBaseline("/a")
    // ⌘T during the Fetch of a toast, twice
    const update = runUpdateProject()
    const second = runUpdateProject()
    await vi.waitFor(() => expect(intents()).toEqual(["fetchBaseline /a", "fetchBaseline /a", "update /a", "update /a"]))
    expect(runningIn("/a")?.run.kind).toBe("fetch")

    // the session joined the second Fetch and the fetch of the update into the first: one outcome each
    const [fetching, joinedFetch] = [await executionIn("/a", "fetchBaseline"), await executionIn("/a", "fetchBaseline", 1)]
    end(fetching, fetchedBy("f"))
    end(joinedFetch, fetchedBy("f"))
    await fetch
    await again
    // the update goes on from that fetch: its run starts with its check
    expect(runningIn("/a")?.run).toMatchObject({ kind: "update", step: "check", plan: ["check", "apply", "sync"] })

    const [updating, joinedUpdate] = [await executionIn("/a", "updateProject"), await executionIn("/a", "updateProject", 1)]
    step(updating, "u", { kind: "updating" })
    expect(stepIn("/a")?.step).toBe("apply")
    end(updating, updated("u", UP_TO_DATE))
    end(joinedUpdate, updated("u", UP_TO_DATE))
    await update
    await second
    expect(runningIn("/a")).toBeUndefined()
    // told once
    expect(openToasts().map(it => it.title)).toEqual(["master is already up to date with origin/master"])
  })

  it("is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync", async () => {
    const update = runUpdateProject()
    const execution = await executionIn("/a", "updateProject")
    step(execution, "u", { kind: "checking" })
    step(execution, "u", { kind: "updating" })
    // the operation ended once it asked its sync: its outcome comes on the channel, the answer once the sync ended
    execution.events.onmessage({ kind: "done", outcome: updated("u", APPLIED) })
    expect(lastToast()).toMatchObject({ title: "Updated master: rebased 2 commits onto origin/master" })
    expect(stepIn("/a")?.step).toBe("sync")
    expect(runningIn("/a")?.syncRun).toBe(7)

    execution.answer({ status: "ok", data: { ...updated("u", APPLIED), versionAfter: { epoch: 1, seq: 9 } } })
    await update
    expect(runningIn("/a")).toBeUndefined()
    // the bar shows the end for a moment
    expect(updateProjectStore.getState().running["/a"]?.endedAt).not.toBeNull()
  })

  it("takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date", async () => {
    const merge = (id: string, repositoryPath = "/a") =>
      showEventToast({
        key: toastKeys.robot(repositoryPath, id),
        aliases: [toastKeys.merged(repositoryPath)],
        render: () => ({ type: "success", title: `${id} merged into master` }),
      })
    const kept = merge("auth")
    merge("login")
    merge("ui", "/b")
    const landed = [{ name: "auth", commits: [{ id: "c", subject: "Add login", upstream: null }], remaining: 0, landedAt: 1 }]
    const update = runUpdateProject()
    end(await executionIn("/a", "updateProject"), updated("u", { ...APPLIED, landed }))
    await update
    // in the place of the newest one, the others of the repository closed: their Update Project is done
    expect(openToasts().map(it => [it.id === kept, it.title, it.description])).toEqual([
      [false, "auth landed in origin/master, squashed", "Updated master: rebased 2 commits"],
      [false, "ui merged into master", undefined],
    ])

    merge("oauth")
    backend.executions = []
    const upToDate = runUpdateProject()
    end(await executionIn("/a", "updateProject"), updated("v", UP_TO_DATE))
    await upToDate
    expect(openToasts().map(it => it.title)).toEqual(["ui merged into master", "master is already up to date with origin/master"])
  })

  it("ignores a step that arrives after its outcome", async () => {
    const update = runUpdateProject()
    const execution = await executionIn("/a", "updateProject")
    execution.events.onmessage({ kind: "done", outcome: updated("u", APPLIED) })
    step(execution, "u", { kind: "updating" })
    expect(stepIn("/a")?.step).toBe("sync")
    execution.answer({ status: "ok", data: updated("u", APPLIED) })
    await update
  })

  it("lists every step of an update from its start, also when the last check found the branch up to date", async () => {
    const update = runUpdateProject()
    expect(runningIn("/a")?.run.plan).toEqual(["fetch", "check", "apply", "sync"])
    const execution = await executionIn("/a", "updateProject")
    step(execution, "u", { kind: "checking" })
    expect(runningIn("/a")?.run.plan).toEqual(["fetch", "check", "apply", "sync"])
    end(execution, updated("u", UP_TO_DATE))
    await update
  })

  it("changes the step only when the operation does, and records what the steps took in the state of the repository once the update ends", async () => {
    const update = runUpdateProject()
    const execution = await executionIn("/a", "updateProject")
    execution.events.onmessage({ kind: "progress", op: "u", ...PROGRESS })
    execution.events.onmessage({ kind: "progress", op: "u", ...PROGRESS })
    step(execution, "u", { kind: "queued", behind: "r" })
    step(execution, "u", { kind: "checking" })
    step(execution, "u", { kind: "checking" })
    expect(runningIn("/a")?.run.done.map(it => [it.step, it.detail])).toEqual([
      ["fetch", "5.79 MiB"],
      ["waiting", null],
    ])
    expect(timings).toEqual([])

    end(execution, updated("u", UP_TO_DATE))
    await update
    // the wait in the queue is no usual time of the repository
    expect(timings).toHaveLength(1)
    expect(Object.keys(timings[0]!.timings as object).toSorted()).toEqual(["check", "fetch"])
  })

  it("opens the dialog of a conflicting or blocked update, which changed nothing", async () => {
    const conflict: UpdateProjectResult = { ...UP_TO_DATE, check: { ...UP_TO_DATE.check, state: conflictState(["a.txt"], 1) } }
    const update = runUpdateProject()
    end(await executionIn("/a", "updateProject"), updated("u", conflict))
    await update
    expect(updateProjectStore.getState().refused).toEqual(conflict.check)
  })
})
