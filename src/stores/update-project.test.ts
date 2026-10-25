import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { FetchBaselineResult, TransferProgress, UpdateCheck, UpdateProjectResult } from "@/lib/bindings"
import { resetEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import type { ToastActionProps } from "@/lib/toast-message"
import { getOperationBlocker, getOperations, operationsStore, runOperation } from "@/stores/operations"
import { retryUpdateWithoutFetch, runFetchBaseline, runUpdateProject, runUpdateProjectOf, selectRunningUpdate, updateProjectStore } from "@/stores/update-project"
import { openToasts, resetToasts, toasts } from "@/test-support/toast-mock"
import { conflictState } from "@/test-support/update-conflict"

/** A command of the backend that runs until the test finishes it */
interface RunningCommand {
  readonly kind: "update" | "fetch"
  readonly repositoryPath: string
  readonly progress: { onmessage: (message: unknown) => void }
  readonly finish: (result: unknown) => void
}

const { backend, selection, syncs, fetchTimes, timings } = vi.hoisted(() => {
  const backend = { commands: [] as RunningCommand[] }
  return {
    backend,
    selection: { path: "/a" },
    syncs: { count: 0, requested: [] as { run: number; end: () => void }[] },
    fetchTimes: [] as string[],
    timings: [] as { path: string; timings: unknown }[],
  }
})

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => {
  const run = (kind: RunningCommand["kind"], params: { repositoryPath: string }, progress: RunningCommand["progress"]) =>
    new Promise(resolve => {
      backend.commands.push({ kind, repositoryPath: params.repositoryPath, progress, finish: resolve })
    })
  return {
    commands: {
      updateProject: (params: { repositoryPath: string }, progress: RunningCommand["progress"]) => run("update", params, progress),
      fetchBaseline: (params: { repositoryPath: string }, progress: RunningCommand["progress"]) => run("fetch", params, progress),
    },
  }
})

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/stores/branch-sync", () => ({
  // the last check found the selected repository up to date
  branchSyncStore: { getState: () => ({ baselineStatus: { repositoryPath: "/a", update: { state: { status: "upToDate" } } } }) },
  setBaselineUpdate: () => {},
  syncBranches: async () => {
    syncs.count++
  },
  // the sync run of an applied update, ended by the test
  requestSync: (onStart: (run: number) => void) =>
    new Promise<void>(resolve => {
      const run = syncs.requested.length + 1
      syncs.requested.push({ run, end: resolve })
      onStart(run)
    }),
}))
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
  useSelectedProject: () => ({ path: selection.path }),
}))

const PROGRESS: TransferProgress = { percent: 45, transferred: "5.79 MiB", speed: "32.57 MiB/s", line: "Receiving objects:  45% (136/302)" }

const FETCHED: FetchBaselineResult = { upstream: "origin/master", fetchedAt: 1_700_000_000 }

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

/** The command of `kind` of the repository, once it started */
async function commandIn(repositoryPath: string, kind: RunningCommand["kind"]): Promise<RunningCommand> {
  const find = () => backend.commands.find(it => it.repositoryPath === repositoryPath && it.kind === kind)
  await vi.waitFor(() => expect(find()).toBeDefined())
  return find()!
}

/** Ends the fetch of the update of the repository; resolves to the command of its check and apply */
async function fetched(repositoryPath: string): Promise<RunningCommand> {
  ;(await commandIn(repositoryPath, "fetch")).finish({ status: "ok", data: FETCHED })
  return commandIn(repositoryPath, "update")
}

/** The kinds of the commands that started, in their order */
function kinds(): string[] {
  return backend.commands.map(it => `${it.kind} ${it.repositoryPath}`)
}

const UNSTACK = { id: "unstack", subject: "branch:auth", objects: ["auth"], kind: "rewrite", label: "Unstacking auth", repositoryPath: "/a" } as const
const MOVE = { id: "move-to", subject: "branch:login", objects: ["login"], kind: "rewrite", label: "Moving 1 commit to login", repositoryPath: "/a" } as const

beforeEach(() => {
  updateProjectStore.setState({ running: {}, refused: null })
  operationsStore.setState({ running: [], claims: {} })
  backend.commands = []
  selection.path = "/a"
  syncs.count = 0
  syncs.requested = []
  fetchTimes.length = 0
  timings.length = 0
  resetEventToasts()
  resetToasts()
})

describe("the running update", () => {
  it("fetches outside the slot of the rewrites, which run meanwhile, then waits for a running rewrite in the queue for its check and apply, and the rewrites wait for those", async () => {
    const update = runUpdateProject()
    const fetch = await commandIn("/a", "fetch")
    fetch.progress.onmessage(PROGRESS)
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })
    // a move during the fetch reaches the backend at once
    await expect(runOperation(MOVE, () => Promise.resolve("moved"))).resolves.toBe("moved")

    // a rewrite that runs when the fetch ends: the check and the apply wait for it, the bar says so
    let finishRewrite!: () => void
    const rewrite = runOperation(UNSTACK, () => new Promise<void>(resolve => (finishRewrite = resolve)))
    fetch.finish({ status: "ok", data: FETCHED })
    await vi.waitFor(() => expect(getOperations().map(operation => operation.waitsFor)).toEqual([null, "Unstacking auth"]))
    expect(stepIn("/a")?.step).toBe("waiting")
    expect(kinds()).toEqual(["fetch /a"])
    finishRewrite()
    await rewrite

    const command = await commandIn("/a", "update")
    // the update checks against the fetched baseline, without a fetch of its own
    expect(getOperationBlocker(getOperations(), MOVE)?.label).toBe("Updating the project")
    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(getOperations()).toEqual([])
  })

  it("belongs to its repository: switching repositories never shows the other repository's phase", async () => {
    // the Fetch of a merge toast of /a
    const fetch = runFetchBaseline("/a")
    ;(await commandIn("/a", "fetch")).progress.onmessage(PROGRESS)
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    selection.path = "/b"
    expect(runningIn("/b")).toBeUndefined()

    // /b updates while /a fetches
    const update = runUpdateProject()
    const command = await fetched("/b")
    command.progress.onmessage({ phase: "checking", fetch: null })
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    ;(await commandIn("/a", "fetch")).finish({ status: "ok", data: FETCHED })
    await fetch
    expect(runningIn("/a")).toBeUndefined()
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })
    // the fetched repository is not the selected one: the table of /b is not synced for it
    expect(syncs.count).toBe(0)

    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(runningIn("/b")).toBeUndefined()
  })

  it("updates the repository a toast is about only while it is shown, else only fetches it: never the one shown since", async () => {
    // Update Project of a toast of /a while /a is shown: its update
    const update = runUpdateProjectOf("/a")
    ;(await fetched("/a")).finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(kinds()).toEqual(["fetch /a", "update /a"])

    // the same toast clicked after a switch to /b: the fetch of /a, nothing of /b moves
    backend.commands = []
    selection.path = "/b"
    const fetch = runUpdateProjectOf("/a")
    ;(await commandIn("/a", "fetch")).finish({ status: "ok", data: FETCHED })
    await fetch
    expect(kinds()).toEqual(["fetch /a"])
    expect(runningIn("/b")).toBeUndefined()
  })

  it("keeps the fetch time when the update fails after its fetch, e.g. while bd holds the repository", async () => {
    const failedFetch = runUpdateProject()
    ;(await commandIn("/a", "fetch")).finish({ status: "error", error: { ...internalError("Failed to fetch origin/master"), code: "network_error" } })
    await failedFetch
    expect(fetchTimes).toEqual([])
    // nothing was fetched: nothing to sync, nothing to check
    expect(syncs.count).toBe(0)
    expect(kinds()).toEqual(["fetch /a"])

    backend.commands = []
    const failedAfterTheFetch = runUpdateProject()
    const command = await fetched("/a")
    command.progress.onmessage({ phase: "checking", fetch: null })
    command.finish({
      status: "error",
      error: { ...internalError("`bd push auth` (pid 4242) is changing this repository; try again when it is done"), code: "locked", retry: "same" },
    })
    await failedAfterTheFetch
    expect(fetchTimes).toEqual(["/a"])
    // the baseline moved: the branches are built against it
    expect(syncs.count).toBe(1)
  })

  it("offers Retry Without Fetch for a held lock after the fetch, which checks and applies without a second fetch; a moved branch gets none", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {})
    const locked = runUpdateProject()
    ;(await fetched("/a")).finish({
      status: "error",
      error: {
        code: "locked",
        message: "`bd push auth` (pid 4242) is changing this repository; try again when it is done",
        details: { type: "locked", holder: { tool: "bd", command: "bd push auth", pid: 4242, held_for: 40 } },
        retry: "same",
        operation_id: null,
      },
    })
    await locked
    expect(lastToast()).toMatchObject({ title: "Cannot update the project", description: "`bd push auth` (pid 4242) has held the repository for 40 s" })
    // the action as `notifyError` hands it over
    const retry = lastToast()?.actionProps as ToastActionProps | undefined
    expect(retry?.children).toBe("Retry Without Fetch")

    backend.commands = []
    retry!.onClick()
    // a second retry joins the first
    const retried = retryUpdateWithoutFetch("/a")
    const command = await commandIn("/a", "update")
    // the check and the apply only: the fetch of the refused update stands
    expect(runningIn("/a")?.run.plan).toEqual(["check", "apply", "sync"])
    command.finish({ status: "ok", data: UP_TO_DATE })
    await retried
    expect(kinds()).toEqual(["update /a"])

    // the branch moved: the request is read again, not repeated
    backend.commands = []
    const moved = runUpdateProject()
    ;(await fetched("/a")).finish({ status: "error", error: { ...internalError("The branch moved"), code: "head_moved", retry: "after_reread" } })
    await moved
    expect(lastToast()).toMatchObject({ title: "Cannot update the project", description: "The branch moved" })
    expect(lastToast()?.actionProps).toBeUndefined()
  })

  it("retries without a fetch only the repository the main window shows", async () => {
    selection.path = "/b"
    await retryUpdateWithoutFetch("/a")
    expect(kinds()).toEqual([])
  })

  it("syncs after every update, also an update that was up to date, conflicted or was blocked", async () => {
    const answers: UpdateProjectResult[] = [
      UP_TO_DATE,
      { ...UP_TO_DATE, check: { ...UP_TO_DATE.check, state: conflictState(["a.txt"], 1) } },
      { ...UP_TO_DATE, check: { ...UP_TO_DATE.check, state: { status: "blocked", paths: ["a.txt"], pathCount: 1 } } },
    ]
    for (const [index, answer] of answers.entries()) {
      backend.commands = []
      const update = runUpdateProject()
      ;(await fetched("/a")).finish({ status: "ok", data: answer })
      await update
      expect(syncs.count).toBe(index + 1)
    }
  })

  it("syncs only the selected repository: another one selected during the update is not synced for it", async () => {
    const update = runUpdateProject()
    const command = await fetched("/a")
    selection.path = "/b"
    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(syncs.count).toBe(0)
  })

  it("runs one at a time in a repository: a second start joins it, and an update during a Fetch goes on from that fetch", async () => {
    const fetch = runFetchBaseline("/a")
    const again = runFetchBaseline("/a")
    // ⌘T during the Fetch of a toast
    const update = runUpdateProject()
    const second = runUpdateProject()
    expect(kinds()).toEqual(["fetch /a"])

    ;(await commandIn("/a", "fetch")).finish({ status: "ok", data: FETCHED })
    await fetch
    await again
    // the selected repository fetched: its table is synced against the fetched baseline
    expect(syncs.count).toBe(1)
    const command = await commandIn("/a", "update")
    expect(kinds()).toEqual(["fetch /a", "update /a"])
    expect(runningIn("/a")?.run.plan).toEqual(["check", "apply", "sync"])
    // a Fetch during the update joins it
    const third = runFetchBaseline("/a")
    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
    await second
    await third
    expect(kinds()).toEqual(["fetch /a", "update /a"])
    expect(runningIn("/a")).toBeUndefined()
  })

  it("is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync", async () => {
    const update = runUpdateProject()
    const command = await fetched("/a")
    command.progress.onmessage({ phase: "checking", fetch: null })
    command.progress.onmessage({ phase: "updating", fetch: null })
    command.finish({ status: "ok", data: APPLIED })
    await vi.waitFor(() => expect(syncs.requested).toHaveLength(1))
    expect(stepIn("/a")?.step).toBe("sync")
    expect(runningIn("/a")?.syncRun).toBe(1)
    // the sync of an applied update is the update's, not a sync of its own on Sync
    expect(syncs.count).toBe(0)

    syncs.requested[0]!.end()
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
    const command = await fetched("/a")
    command.finish({ status: "ok", data: { ...APPLIED, landed } })
    await vi.waitFor(() => expect(syncs.requested).toHaveLength(1))
    // in the place of the newest one, the others of the repository closed: their Update Project is done
    expect(openToasts().map(it => [it.id === kept, it.title, it.description])).toEqual([
      [false, "auth landed in origin/master, squashed", "Updated master: rebased 2 commits"],
      [false, "ui merged into master", undefined],
    ])
    syncs.requested[0]!.end()
    await update

    merge("oauth")
    backend.commands = []
    const upToDate = runUpdateProject()
    ;(await fetched("/a")).finish({ status: "ok", data: UP_TO_DATE })
    await upToDate
    expect(openToasts().map(it => it.title)).toEqual(["ui merged into master", "master is already up to date with origin/master"])
  })

  it("ignores a progress message that arrives after the command answered", async () => {
    const update = runUpdateProject()
    const command = await fetched("/a")
    command.finish({ status: "ok", data: APPLIED })
    await vi.waitFor(() => expect(syncs.requested).toHaveLength(1))
    command.progress.onmessage({ phase: "updating", fetch: null })
    expect(stepIn("/a")?.step).toBe("sync")
    syncs.requested[0]!.end()
    await update
  })

  it("lists every step of an update from its start, also when the last check found the branch up to date", async () => {
    const update = runUpdateProject()
    expect(runningIn("/a")?.run.plan).toEqual(["fetch", "check", "apply", "sync"])
    const command = await fetched("/a")
    command.progress.onmessage({ phase: "checking", fetch: null })
    expect(runningIn("/a")?.run.plan).toEqual(["fetch", "check", "apply", "sync"])
    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
  })

  it("changes the step only when the phase changes, and records what the steps took in the state of the repository once the update ends", async () => {
    const update = runUpdateProject()
    const fetch = await commandIn("/a", "fetch")
    fetch.progress.onmessage(PROGRESS)
    fetch.progress.onmessage(PROGRESS)
    const command = await fetched("/a")
    command.progress.onmessage({ phase: "waiting", fetch: null })
    command.progress.onmessage({ phase: "checking", fetch: null })
    command.progress.onmessage({ phase: "checking", fetch: null })
    expect(runningIn("/a")?.run.done.map(it => [it.step, it.detail])).toEqual([
      ["fetch", "5.79 MiB"],
      ["waiting", null],
    ])
    expect(timings).toEqual([])

    command.finish({ status: "ok", data: UP_TO_DATE })
    await update
    // the wait for the lock is no usual time of the repository
    expect(timings).toHaveLength(1)
    expect(Object.keys(timings[0]!.timings as object).toSorted()).toEqual(["check", "fetch"])
  })
})
