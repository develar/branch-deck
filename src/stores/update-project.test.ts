import { beforeEach, describe, expect, it, vi } from "vitest"
import type { FetchBaselineResult, TransferProgress, UpdateCheck, UpdateProjectResult } from "@/lib/bindings"
import { runFetchBaseline, runUpdateProject, selectRunningUpdate, updateProjectStore } from "@/stores/update-project"

/** A command of the backend that runs until the test finishes it */
interface RunningCommand {
  readonly repositoryPath: string
  readonly progress: { onmessage: (message: unknown) => void }
  readonly finish: (result: unknown) => void
}

const { backend, selection, syncs } = vi.hoisted(() => {
  const backend = { commands: [] as RunningCommand[] }
  return {
    backend,
    selection: { path: "/a" },
    syncs: { count: 0 },
  }
})

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => {
  const run = (params: { repositoryPath: string }, progress: RunningCommand["progress"]) => new Promise((resolve) => {
    backend.commands.push({ repositoryPath: params.repositoryPath, progress, finish: resolve })
  })
  return { commands: { updateProject: run, fetchBaseline: run } }
})

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
vi.mock("@/stores/branch-sync", () => ({
  setBaselineUpdate: () => {},
  syncBranches: async () => {
    syncs.count++
  },
}))
vi.mock("@/stores/landed", () => ({ addLandedUpdate: () => {} }))
vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: () => {} }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selection.path }),
  useSelectedProject: () => ({ path: selection.path }),
}))

const PROGRESS: TransferProgress = { percent: 45, transferred: "5.79 MiB", speed: "32.57 MiB/s", line: "Receiving objects:  45% (136/302)" }

const FETCHED: FetchBaselineResult = { upstream: "origin/master", fetchedAt: 1_700_000_000 }

const UP_TO_DATE: UpdateProjectResult = {
  check: { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", state: { status: "upToDate" }, fetchedAt: null } satisfies UpdateCheck,
  applied: null,
  landed: [],
}

function runningIn(repositoryPath: string) {
  return selectRunningUpdate(updateProjectStore.getState(), repositoryPath)
}

function commandOf(repositoryPath: string): RunningCommand {
  const command = backend.commands.find(it => it.repositoryPath === repositoryPath)
  expect(command).toBeDefined()
  return command!
}

beforeEach(() => {
  updateProjectStore.setState({ running: {}, refused: null })
  backend.commands = []
  selection.path = "/a"
  syncs.count = 0
})

describe("the running update", () => {
  it("belongs to its repository: switching repositories never shows the other repository's phase", async () => {
    // the Fetch of a merge toast of /a
    const fetch = runFetchBaseline("/a")
    commandOf("/a").progress.onmessage(PROGRESS)
    expect(runningIn("/a")).toEqual({ phase: "fetching", fetch: PROGRESS })

    selection.path = "/b"
    expect(runningIn("/b")).toBeUndefined()

    // /b updates while /a fetches
    const update = runUpdateProject()
    commandOf("/b").progress.onmessage({ phase: "checking", fetch: null })
    expect(runningIn("/b")).toEqual({ phase: "checking", fetch: null })
    expect(runningIn("/a")).toEqual({ phase: "fetching", fetch: PROGRESS })

    commandOf("/a").finish({ status: "ok", data: FETCHED })
    await fetch
    expect(runningIn("/a")).toBeUndefined()
    expect(runningIn("/b")).toEqual({ phase: "checking", fetch: null })
    // the fetched repository is not the selected one: the table of /b is not synced for it
    expect(syncs.count).toBe(0)

    commandOf("/b").finish({ status: "ok", data: UP_TO_DATE })
    await update
    expect(updateProjectStore.getState().running).toEqual({})
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
})
