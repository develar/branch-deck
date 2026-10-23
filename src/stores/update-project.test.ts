import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { FetchBaselineResult, TransferProgress, UpdateCheck, UpdateProjectResult } from "@/lib/bindings"
import { getOperationBlocker, getOperations, operationsStore, runOperation } from "@/stores/operations"
import { runFetchBaseline, runUpdateProject, runUpdateProjectOf, selectRunningUpdate, updateProjectStore } from "@/stores/update-project"

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
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
vi.mock("@/stores/branch-sync", () => ({
  // the last check found the selected repository up to date
  branchSyncStore: { getState: () => ({ baselineStatus: { repositoryPath: "/a", update: { state: { status: "upToDate" } } } }) },
  setBaselineUpdate: () => {},
  syncBranches: async () => {
    syncs.count++
  },
  // the sync run of an applied update, ended by the test
  requestSync: (_options: unknown, onStart: (run: number) => void) =>
    new Promise<void>(resolve => {
      const run = syncs.requested.length + 1
      syncs.requested.push({ run, end: resolve })
      onStart(run)
    }),
}))
vi.mock("@/stores/landed", () => ({ addLandedUpdate: () => {} }))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)
vi.mock("@/stores/settings/repositorySettings", () => ({
  getRecentProjects: () => [],
  updateProject: (path: string, update: { lastFetchTime?: number; updateTimings?: unknown }) => {
    if (update.lastFetchTime !== undefined) {
      fetchTimes.push(path)
    }
    if (update.updateTimings !== undefined) {
      timings.push({ path, timings: update.updateTimings })
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

function runningIn(repositoryPath: string) {
  return selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
}

/** The step the run of the repository is at, with the fetch progress it holds; undefined when none runs */
function stepIn(repositoryPath: string) {
  const running = runningIn(repositoryPath)
  return running === undefined ? undefined : { step: running.run.step, fetch: running.run.fetch }
}

function commandOf(repositoryPath: string): RunningCommand {
  const command = backend.commands.find(it => it.repositoryPath === repositoryPath)
  expect(command).toBeDefined()
  return command!
}

beforeEach(() => {
  updateProjectStore.setState({ running: {}, refused: null })
  operationsStore.setState({ running: [], claims: {} })
  backend.commands = []
  selection.path = "/a"
  syncs.count = 0
  syncs.requested = []
  fetchTimes.length = 0
  timings.length = 0
})

describe("the running update", () => {
  it("waits in the queue for a rewrite of the repository that runs, then runs, and the rewrites wait for it: one rewrite at a time", async () => {
    let finishRewrite!: () => void
    const rewrite = runOperation(
      { id: "unstack", subject: "branch:auth", objects: ["auth"], kind: "rewrite", label: "Unstacking auth", repositoryPath: "/a" },
      () => new Promise<void>(resolve => (finishRewrite = resolve)),
    )
    const update = runUpdateProject()
    expect(backend.commands).toEqual([])
    expect(getOperations().map(operation => operation.waitsFor)).toEqual([null, "Unstacking auth"])
    finishRewrite()
    await rewrite

    await vi.waitFor(() => expect(backend.commands).toHaveLength(1))
    const move = { id: "move-to", subject: "branch:login", objects: ["login"], kind: "rewrite", repositoryPath: "/a" } as const
    expect(getOperationBlocker(getOperations(), move)?.label).toBe("Updating the project")
    commandOf("/a").finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(getOperations()).toEqual([])
  })

  it("belongs to its repository: switching repositories never shows the other repository's phase", async () => {
    // the Fetch of a merge toast of /a
    const fetch = runFetchBaseline("/a")
    commandOf("/a").progress.onmessage(PROGRESS)
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    selection.path = "/b"
    expect(runningIn("/b")).toBeUndefined()

    // /b updates while /a fetches
    const update = runUpdateProject()
    commandOf("/b").progress.onmessage({ phase: "checking", fetch: null })
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })
    expect(stepIn("/a")).toEqual({ step: "fetch", fetch: PROGRESS })

    commandOf("/a").finish({ status: "ok", data: FETCHED })
    await fetch
    expect(runningIn("/a")).toBeUndefined()
    expect(stepIn("/b")).toEqual({ step: "check", fetch: null })
    // the fetched repository is not the selected one: the table of /b is not synced for it
    expect(syncs.count).toBe(0)

    commandOf("/b").finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(runningIn("/b")).toBeUndefined()
  })

  it("updates the repository a toast is about only while it is shown, else only fetches it: never the one shown since", async () => {
    // Update Project of a toast of /a while /a is shown: its update
    const update = runUpdateProjectOf("/a")
    expect(backend.commands.map(it => [it.kind, it.repositoryPath])).toEqual([["update", "/a"]])
    commandOf("/a").finish({ status: "ok", data: UP_TO_DATE })
    await update

    // the same toast clicked after a switch to /b: the fetch of /a, nothing of /b moves
    backend.commands = []
    selection.path = "/b"
    const fetch = runUpdateProjectOf("/a")
    expect(backend.commands.map(it => [it.kind, it.repositoryPath])).toEqual([["fetch", "/a"]])
    commandOf("/a").finish({ status: "ok", data: FETCHED })
    await fetch
    expect(runningIn("/b")).toBeUndefined()
  })

  it("keeps the fetch time when the update fails after its fetch, e.g. while bd holds the repository", async () => {
    const failedBeforeTheCheck = runUpdateProject()
    commandOf("/a").finish({ status: "error", error: { message: "Failed to fetch origin/master" } })
    await failedBeforeTheCheck
    expect(fetchTimes).toEqual([])
    // nothing was fetched: nothing to sync
    expect(syncs.count).toBe(0)

    backend.commands = []
    const failedAfterTheFetch = runUpdateProject()
    commandOf("/a").progress.onmessage({ phase: "checking", fetch: null })
    commandOf("/a").finish({ status: "error", error: { message: "Branch Deck or bd is changing this repository. Try again when it is done." } })
    await failedAfterTheFetch
    expect(fetchTimes).toEqual(["/a"])
    // the baseline moved: the branches are built against it
    expect(syncs.count).toBe(1)
  })

  it("syncs after every update, also an update that was up to date, conflicted or was blocked", async () => {
    const answers: UpdateProjectResult[] = [
      UP_TO_DATE,
      { ...UP_TO_DATE, check: { ...UP_TO_DATE.check, state: { status: "conflict", files: ["a.txt"], fileCount: 1, commits: [], upstreamCommits: [] } } },
      { ...UP_TO_DATE, check: { ...UP_TO_DATE.check, state: { status: "blocked", paths: ["a.txt"], pathCount: 1 } } },
    ]
    for (const [index, answer] of answers.entries()) {
      backend.commands = []
      const update = runUpdateProject()
      commandOf("/a").finish({ status: "ok", data: answer })
      await update
      expect(syncs.count).toBe(index + 1)
    }
  })

  it("syncs only the selected repository: another one selected during the update is not synced for it", async () => {
    const update = runUpdateProject()
    selection.path = "/b"
    commandOf("/a").finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(syncs.count).toBe(0)
  })

  it("runs one at a time in a repository", async () => {
    const fetch = runFetchBaseline("/a")
    await runFetchBaseline("/a")
    await runUpdateProject()
    expect(backend.commands).toHaveLength(1)

    commandOf("/a").finish({ status: "ok", data: FETCHED })
    await fetch
    // the selected repository fetched: its table is synced against the fetched baseline
    expect(syncs.count).toBe(1)
    expect(runningIn("/a")).toBeUndefined()
  })

  it("is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync", async () => {
    const update = runUpdateProject()
    commandOf("/a").progress.onmessage({ phase: "checking", fetch: null })
    commandOf("/a").progress.onmessage({ phase: "updating", fetch: null })
    commandOf("/a").finish({ status: "ok", data: APPLIED })
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

  it("ignores a progress message that arrives after the command answered", async () => {
    const update = runUpdateProject()
    const command = commandOf("/a")
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
    commandOf("/a").progress.onmessage({ phase: "checking", fetch: null })
    expect(runningIn("/a")?.run.plan).toEqual(["fetch", "check", "apply", "sync"])
    commandOf("/a").finish({ status: "ok", data: UP_TO_DATE })
    await update
  })

  it("changes the step only when the phase changes, and learns what the steps took once the update ends", async () => {
    const update = runUpdateProject()
    const command = commandOf("/a")
    command.progress.onmessage({ phase: "fetching", fetch: null })
    command.progress.onmessage({ phase: "fetching", fetch: PROGRESS })
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
