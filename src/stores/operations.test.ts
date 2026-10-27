// @spec spec/branch/branch-row.spec.md
import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import type { MenuItem } from "@/lib/menu"
import type { OperationSpec } from "@/stores/operations"
import { getOperations, operationItem, operationLabel, operationsStore, runOperation, setSessionStep, whenRewritesDone } from "@/stores/operations"

const REPOSITORY = "/repo"

function rewriteOf(branch: string, id = "unstack"): OperationSpec {
  return { id, subject: `branch:${branch}`, objects: [branch], kind: "rewrite", label: `Unstacking ${branch}`, repositoryPath: REPOSITORY }
}

function remoteOf(subject: string, objects: string[], id = "restart-robot"): OperationSpec {
  return { id, subject, objects, kind: "remote", label: `Restarting ${subject}`, repositoryPath: REPOSITORY }
}

/** A promise the test settles */
function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void; reject: (error: unknown) => void } {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve
    reject = onReject
  })
  return { promise, resolve, reject }
}

const unstack: MenuItem = { id: "unstack", label: "Unstack from air", onSelect: () => {} }

describe("runOperation", () => {
  afterEach(() => {
    operationsStore.setState({ running: [], claims: {} })
  })

  it("runs a second start of the same action once: it joins the running one and resolves with it", async () => {
    const result = deferred<string>()
    const run = vi.fn(() => result.promise)
    const first = runOperation(rewriteOf("auth"), run)
    const second = runOperation(rewriteOf("auth"), run)
    expect(getOperations().map(operation => operation.label)).toEqual(["Unstacking auth"])
    result.resolve("done")
    await expect(first).resolves.toBe("done")
    await expect(second).resolves.toBe("done")
    expect(run).toHaveBeenCalledOnce()
    expect(getOperations()).toEqual([])
  })

  it("joins only a start with the same parameters and starts every other rewrite at once: the session of its repository queues it", async () => {
    const running = deferred<null>()
    void runOperation(rewriteOf("auth"), () => running.promise)
    const moveOf = (parameters: string) => ({ ...rewriteOf("docs", "move-to"), parameters })
    const moved: string[] = []
    const move = (commit: string) => () => {
      moved.push(commit)
      return Promise.resolve(commit)
    }
    const first = runOperation(moveOf("A"), move("A"))
    // the drop of A again joins the move of A; the drop of B is a move of its own
    const repeat = runOperation(moveOf("A"), move("A"))
    const second = runOperation(moveOf("B"), move("B"))
    expect(moved).toEqual(["A", "B"])
    expect(getOperations().map(operation => [operation.parameters, operation.waitsFor])).toEqual([
      [undefined, null],
      ["A", null],
      ["B", null],
    ])
    await expect(first).resolves.toBe("A")
    await expect(repeat).resolves.toBe("A")
    await expect(second).resolves.toBe("B")
    // a remote action of the branch the rewrite works on runs beside it
    await expect(runOperation(remoteOf("robot:7", ["auth"]), () => Promise.resolve(2))).resolves.toBe(2)

    running.resolve(null)
    await vi.waitFor(() => expect(getOperations()).toEqual([]))
  })

  it("lets a remote action wait for one of the same object or of a branch it works on, never for one of another field of a pull request", async () => {
    const running = deferred<null>()
    void runOperation(remoteOf("robot:7", ["auth"]), () => running.promise)

    const run = vi.fn(() => Promise.resolve())
    await expect(runOperation(remoteOf("robot:7", [], "cancel-robot"), run)).resolves.toBeUndefined()
    await expect(runOperation(remoteOf("pr:12", ["auth"], "retarget-pull-request"), run)).resolves.toBeUndefined()
    expect(run).not.toHaveBeenCalled()
    // another object
    await expect(runOperation(remoteOf("robot:8", ["login"]), () => Promise.resolve(3))).resolves.toBe(3)

    // a retarget and a review request of one pull request write other fields of it: side by side
    const review = deferred<null>()
    void runOperation({ ...remoteOf("pr:13", ["login"], "request-review"), resources: ["pr:13:reviewers"] }, () => review.promise)
    await expect(runOperation({ ...remoteOf("pr:13", ["login"], "retarget-pull-request"), resources: ["pr:13:base"] }, () => Promise.resolve(4))).resolves.toBe(4)
    running.resolve(null)
    review.resolve(null)
  })

  it("lets a push wait for the rewrites of its repository and the syncs they end with, the queued ones too", async () => {
    const first = deferred<null>()
    const second = deferred<null>()
    void runOperation(rewriteOf("auth"), () => first.promise)
    void runOperation(rewriteOf("login", "move-to"), () => second.promise)
    let done = false
    const waiting = whenRewritesDone(REPOSITORY).then(() => {
      done = true
    })
    first.resolve(null)
    await vi.waitFor(() => expect(getOperations()).toHaveLength(1))
    expect(done).toBe(false)
    second.resolve(null)
    await waiting
    expect(getOperations()).toEqual([])
    // nothing runs: at once
    await whenRewritesDone(REPOSITORY)
  })

  it("ends the operation when its action fails", async () => {
    await expect(
      runOperation(rewriteOf("auth"), () => {
        throw new Error("git failed")
      }),
    ).rejects.toThrow("git failed")
    expect(getOperations()).toEqual([])
  })
})

describe("the steps of an operation of the session", () => {
  afterEach(() => {
    operationsStore.setState({ running: [], claims: {} })
  })

  it("says whom the operation behind a record waits for, or which rewrite it waits behind, and its own state again once it runs", async () => {
    const result = deferred<null>()
    let key = 0
    const done = runOperation(rewriteOf("auth"), started => {
      key = started
      return result.promise
    })
    const running = () => getOperations()[0]!
    expect([operationLabel(running()), running().op]).toEqual(["Unstacking auth", null])

    setSessionStep(key, { op: "op-1", lockWait: null, waitsFor: "Moving 1 commit to docs" })
    expect(operationLabel(running())).toBe("Waits for Moving 1 commit to docs")
    setSessionStep(key, { op: "op-1", lockWait: "Waiting for `bd pick` (pid 4242, for 12 s)", waitsFor: null })
    expect([operationLabel(running()), running().op]).toEqual(["Waiting for `bd pick` (pid 4242, for 12 s)", "op-1"])

    // the holder released the lock: the rewrite runs
    setSessionStep(key, { op: "op-1", lockWait: null, waitsFor: null })
    expect(operationLabel(running())).toBe("Unstacking auth")
    result.resolve(null)
    await done
  })
})

describe("operationItem", () => {
  afterEach(() => {
    operationsStore.setState({ running: [], claims: {} })
  })

  it("marks the item of the running action running, and leaves the items that would wait for it enabled: their start queues", async () => {
    const running = deferred<null>()
    void runOperation(rewriteOf("auth"), () => running.promise)
    const operations = getOperations()

    expect(operationItem(operations, unstack, rewriteOf("auth"))).toMatchObject({ running: true, disabled: true, description: "Unstacking auth…" })
    const move: MenuItem = { id: "move-to", label: "Move to…", onSelect: () => {} }
    expect(operationItem(operations, move, rewriteOf("login", "move-to"))).toBe(move)
    // an item disabled for its own reason keeps it
    const disabled: MenuItem = { ...move, disabled: true, description: "No other branch" }
    expect(operationItem(operations, disabled, rewriteOf("login", "move-to"))).toBe(disabled)
    // a remote action is no rewrite's to wait for
    const restart: MenuItem = { id: "restart-robot", label: "Restart Safe Push", onSelect: () => {} }
    expect(operationItem(operations, restart, remoteOf("robot:7", ["auth"]))).toBe(restart)
    running.resolve(null)
  })

  it("disables the item of a remote action that must wait with the busy state of the one it waits for", async () => {
    const running = deferred<null>()
    void runOperation(remoteOf("robot:7", ["auth"]), () => running.promise)
    const cancel: MenuItem = { id: "cancel-robot", label: "Cancel Safe Push", onSelect: () => {} }
    expect(operationItem(getOperations(), cancel, remoteOf("robot:7", [], "cancel-robot"))).toMatchObject({ disabled: true, description: "Restarting robot:7…" })
    running.resolve(null)
  })
})
