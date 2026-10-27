// @spec spec/branch/move-commits.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, OperationEvent, Outcome, RewriteIntent, Version } from "@/lib/bindings"
import { runRewriteIntent, undoRewrite } from "@/lib/rewrite-run"
import { getOperations, stopWaitingItem } from "@/stores/operations"

const { engine } = vi.hoisted(() => ({
  engine: {
    /** What reached Rust, in order */
    calls: [] as string[],
    /** The events the next `execute` sends before it answers, and the test's move on: the answer waits for it */
    events: [] as unknown[],
    proceed: null as Promise<void> | null,
    /** The error the next `execute` answers; null: it ran */
    error: null as ApiError | null,
    /** The versions the page waited for */
    waited: [] as string[],
    /** What the toasts said */
    toasts: [] as string[],
    errors: [] as string[],
  },
}))

const VERSION: Version = { epoch: 1, seq: 9 }
const WITNESS = { head: "h1", storeTips: { issueGroups: null, stackParents: null, parked: null } }

vi.mock("@tauri-apps/api/core", async () => await import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", () => ({
  commands: {
    execute: async (params: { repositoryPath: string; intent: RewriteIntent & { kind: string } }, events: { onmessage: (event: OperationEvent) => void }) => {
      engine.calls.push(`execute ${params.intent.rewrite} in ${params.repositoryPath}`)
      for (const event of engine.events) {
        events.onmessage(event as OperationEvent)
      }
      await engine.proceed
      if (engine.error !== null) {
        return { status: "error", error: engine.error }
      }
      const outcome: Outcome = {
        op: "op-1",
        effects: [],
        newsKey: null,
        versionAfter: VERSION,
        result: { kind: "rewrite", rewrite: "undone", answer: { alreadyUndone: false } },
        undo: null,
        stats: { gitProcesses: {}, httpRequests: 0, millis: 1 },
      }
      return { status: "ok", data: outcome }
    },
    cancelOperation: async (params: { op: string }) => {
      engine.calls.push(`cancel ${params.op}`)
      return true
    },
  },
}))
vi.mock("@/model/store", () => ({
  getSessionWitness: () => WITNESS,
  whenSessionViewAt: async (repositoryPath: string, version: Version | null) => {
    engine.waited.push(`${repositoryPath} at ${version?.seq}`)
  },
}))
vi.mock("@/stores/activity", () => ({ refreshActivity: async () => {} }))
vi.mock("@/stores/branch-sync", () => ({ branchSyncStore: { getState: () => ({ operations: {} }) } }))
vi.mock("@/stores/repository", () => ({ getSelectedProject: () => ({ path: "/repo" }) }))
vi.mock("@/lib/toast-message", () => ({ showToast: (toast: { title: string }) => engine.toasts.push(toast.title) }))
vi.mock("@/lib/errors", () => ({ notifyError: (title: string) => engine.errors.push(title) }))

const UNSTACK: RewriteIntent = { rewrite: "unstack", branchName: "oauth", witness: WITNESS }
const SPEC = { id: "unstack", subject: "branch:oauth", objects: ["oauth"], repositoryPath: "/repo" }

beforeEach(() => {
  engine.calls.length = 0
  engine.events = []
  engine.proceed = null
  engine.error = null
  engine.waited.length = 0
  engine.toasts.length = 0
  engine.errors.length = 0
})

describe("runRewriteIntent", () => {
  it("runs the rewrite in Rust without a plan of the page, then waits for the view of the sync it asked", async () => {
    const done = await runRewriteIntent(SPEC, UNSTACK)
    expect(done?.result).toEqual({ kind: "rewrite", rewrite: "undone", answer: { alreadyUndone: false } })
    expect(engine.calls).toEqual(["execute unstack in /repo"])
    expect(engine.waited).toEqual(["/repo at 9"])
    expect(getOperations()).toEqual([])
  })

  it("shows whom a rewrite of the engine waits for and stops the wait by its operation", async () => {
    let proceed!: () => void
    engine.proceed = new Promise(resolve => {
      proceed = resolve
    })
    engine.events = [{ kind: "step", op: "op-1", step: { kind: "waitingForLock", holder: { tool: "bd", command: "bd pick --follow", pid: 4242, held_for: 12 } } }]
    const run = runRewriteIntent(SPEC, UNSTACK)
    await vi.waitFor(() => expect(getOperations()[0]?.lockWait).toBe("Waiting for `bd pick --follow` (pid 4242, for 12 s)"))
    const [operation] = getOperations()
    // the page queues nothing it does not run: the session admitted it
    expect(operation?.waitsFor).toBeNull()

    stopWaitingItem(operation)!.onSelect!({ surface: null })
    await vi.waitFor(() => expect(engine.calls).toContain("cancel op-1"))
    engine.error = { code: "canceled", message: "The wait for the repository lock was stopped; nothing changed", details: null, retry: "no", operation_id: "op-1" }
    proceed()
    expect(await run).toBeNull()
    expect(engine.toasts).toEqual(["Canceled unstacking oauth before it started"])
    expect(engine.errors).toEqual([])
  })

  it("says behind which rewrite of this window it waits in the queue of the session", async () => {
    let proceed!: () => void
    engine.proceed = new Promise(resolve => {
      proceed = resolve
    })
    engine.events = [{ kind: "step", op: "op-1", step: { kind: "rewriting" } }]
    const first = runRewriteIntent(SPEC, UNSTACK)
    await vi.waitFor(() => expect(getOperations()).toHaveLength(1))
    engine.events = [{ kind: "step", op: "op-2", step: { kind: "queued", behind: "op-1" } }]
    const move: RewriteIntent = { rewrite: "move", commitIds: ["c1"], targetBranch: "docs", witness: WITNESS }
    const second = runRewriteIntent({ id: "move-to", subject: "branch:docs", objects: ["docs"], repositoryPath: "/repo" }, move)
    await vi.waitFor(() => expect(getOperations()[1]?.waitsFor).toBe("Unstacking oauth"))
    proceed()
    await Promise.all([first, second])
    expect(engine.calls).toEqual(["execute unstack in /repo", "execute move in /repo"])
  })

  it("tells a failure in the words of the rewrite and runs the Undo in the repository of its token", async () => {
    engine.error = { code: "undo_stale", message: "HEAD moved", details: null, retry: "no", operation_id: "op-1" }
    expect(await undoRewrite("/first", { kind: "regroup", token: "a..b" }, "Undoing the move", "Cannot undo the move")).toBe(false)
    expect(engine.calls).toEqual(["execute undo in /first"])
    expect(engine.errors).toEqual(["Cannot undo the move"])

    engine.error = null
    expect(await undoRewrite("/first", { kind: "regroup", token: "a..b" }, "Undoing the move", "Cannot undo the move")).toBe(true)
  })
})
