// @spec spec/branch/branch-row.spec.md
import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import type { MenuItem } from "@/lib/menu"
import type { OperationSpec } from "@/stores/operations"
import { getOperationMoves, getOperations, operationItem, operationsStore, runOperation } from "@/stores/operations"

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

  it("lets a rewrite wait for any rewrite of the repository, never for a remote action", async () => {
    const running = deferred<null>()
    void runOperation(rewriteOf("auth"), () => running.promise)

    const blocked = vi.fn()
    const otherRewrite = vi.fn(() => Promise.resolve())
    await expect(runOperation(rewriteOf("login", "move-to"), otherRewrite, blocked)).resolves.toBeUndefined()
    expect(otherRewrite).not.toHaveBeenCalled()
    expect(blocked).toHaveBeenCalledWith("Unstacking auth…")

    // a rewrite of another repository runs
    await expect(runOperation({ ...rewriteOf("login"), repositoryPath: "/other" }, () => Promise.resolve(1))).resolves.toBe(1)
    // a remote action of the branch the rewrite works on runs beside it
    await expect(runOperation(remoteOf("robot:7", ["auth"]), () => Promise.resolve(2))).resolves.toBe(2)
    running.resolve(null)
  })

  it("lets a remote action wait for one of the same object or of a branch it works on", async () => {
    const running = deferred<null>()
    void runOperation(remoteOf("robot:7", ["auth"]), () => running.promise)

    const run = vi.fn(() => Promise.resolve())
    await expect(runOperation(remoteOf("robot:7", [], "cancel-robot"), run)).resolves.toBeUndefined()
    await expect(runOperation(remoteOf("pr:12", ["auth"], "retarget-pull-request"), run)).resolves.toBeUndefined()
    expect(run).not.toHaveBeenCalled()
    // another object
    await expect(runOperation(remoteOf("robot:8", ["login"]), () => Promise.resolve(3))).resolves.toBe(3)
    running.resolve(null)
  })

  it("tells the moves of the running operations of the repository until they end", async () => {
    const request = deferred<null>()
    const move = { key: "auth", lands: false, label: "Requesting a review on #12" }
    void runOperation({ ...remoteOf("pr:12", ["auth"], "request-review"), move }, () => request.promise)
    void runOperation({ ...remoteOf("pr:13", ["login"], "request-review"), repositoryPath: "/other", move: { ...move, key: "login" } }, () => request.promise)
    // an operation that is no move of a row
    void runOperation(rewriteOf("auth"), () => request.promise)
    expect(getOperationMoves(REPOSITORY)).toEqual([move])
    request.resolve(null)
    await vi.waitFor(() => expect(getOperationMoves(REPOSITORY)).toEqual([]))
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

describe("operationItem", () => {
  afterEach(() => {
    operationsStore.setState({ running: [], claims: {} })
  })

  it("marks the item of the running action running, the items that wait for it disabled with its busy state", async () => {
    const running = deferred<null>()
    void runOperation(rewriteOf("auth"), () => running.promise)
    const operations = getOperations()

    expect(operationItem(operations, unstack, rewriteOf("auth"))).toMatchObject({ running: true, disabled: true, description: "Unstacking auth…" })
    const move: MenuItem = { id: "move-to", label: "Move to…", onSelect: () => {} }
    expect(operationItem(operations, move, rewriteOf("login", "move-to"))).toMatchObject({ disabled: true, description: "Unstacking auth…" })
    expect(operationItem(operations, move, rewriteOf("login", "move-to")).running).toBeUndefined()
    // an item disabled for its own reason keeps it
    const disabled: MenuItem = { ...move, disabled: true, description: "No other branch" }
    expect(operationItem(operations, disabled, rewriteOf("login", "move-to"))).toBe(disabled)
    // a remote action is no rewrite's to wait for
    const restart: MenuItem = { id: "restart-robot", label: "Restart Safe Push", onSelect: () => {} }
    expect(operationItem(operations, restart, remoteOf("robot:7", ["auth"]))).toBe(restart)
    running.resolve(null)
  })
})
