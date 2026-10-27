// @spec spec/branch/patronus.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { LockHolderInfo, OperationStep, RunningOperation } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { actionOfLabel, canceledBeforeStart, presentLockHolder, presentLockWait, presentOperationLockWait, presentStopWaiting } from "@/presenters/lock-wait"

vi.mock("@/lib/bindings", async importOriginal => {
  const original = await importOriginal<typeof import("@/lib/bindings")>()
  return { ...original, commands: { ...original.commands, cancelOperation: vi.fn(async () => ({ status: "ok", data: true })) } }
})

const BD_PICK: LockHolderInfo = { tool: "bd", command: "bd pick --follow", pid: 4242, held_for: 12 }

/** A push of auth of the session in `step` */
function pushIn(step: OperationStep): RunningOperation {
  return {
    id: "op",
    intent: { kind: "push", subject: { kind: "branch", name: "auth" }, scope: { kind: "alone" }, force: false, restartRunning: false, expectRemote: null },
    resources: [],
    step,
    origin: { kind: "thisProcess" },
    moves: [],
  }
}

describe("presentLockWait", () => {
  it("says whom an operation waits for as the rewrites of the app say it: bd by its command line, the app by its name", () => {
    expect(presentLockWait(BD_PICK)).toBe("Waiting for `bd pick --follow` (pid 4242, for 12 s)")
    expect(presentLockWait({ ...BD_PICK, held_for: 723 })).toBe("Waiting for `bd pick --follow` (pid 4242, for 12 min 3 s)")
    expect(presentLockWait({ tool: "app", command: "branch-deck", pid: 17, held_for: 0 })).toBe("Waiting for Branch Deck (pid 17, for 0 s)")
    expect(presentLockWait(null)).toBe("Waiting for another holder of the repository lock")
  })

  it("names the holder a command was refused by, and how long it has held the repository", () => {
    expect(presentLockHolder({ ...BD_PICK, held_for: 40 })).toBe("`bd pick --follow` (pid 4242) has held the repository for 40 s")
  })

  it("reads whom an operation of the view waits for from its step, and nothing of another step", () => {
    expect(presentOperationLockWait(pushIn({ kind: "waitingForLock", holder: BD_PICK }))).toBe("Waiting for `bd pick --follow` (pid 4242, for 12 s)")
    expect(presentOperationLockWait(pushIn({ kind: "waitingForLock", holder: null }))).toBe("Waiting for another holder of the repository lock")
    expect(presentOperationLockWait(pushIn({ kind: "pushing" }))).toBeNull()
    expect(presentOperationLockWait(undefined)).toBeNull()
  })
})

describe("presentStopWaiting", () => {
  it("offers Stop Waiting of an operation of the session only while it waits for a holder, and stops it by its id", () => {
    const waiting = "Waiting for `bd pick --follow` (pid 4242, for 12 s)"
    expect(presentStopWaiting(undefined)).toBeNull()
    expect(presentStopWaiting({ op: "op-1", lockWait: null })).toBeNull()
    // the channel has not named the operation yet: nothing to stop
    expect(presentStopWaiting({ op: null, lockWait: waiting })).toBeNull()

    const stop = presentStopWaiting({ op: "op-1", lockWait: waiting })!
    expect(stop).toMatchObject({ id: "stop-waiting", label: "Stop Waiting", description: `${waiting}; stop and change nothing` })
    stop.onSelect({ surface: null })
    expect(vi.mocked(commands.cancelOperation)).toHaveBeenCalledExactlyOnceWith({ op: "op-1" })
  })
})

describe("canceledBeforeStart", () => {
  it("says that an action stopped while it waited for the repository lock changed nothing", () => {
    expect(canceledBeforeStart("the push of auth", "Nothing was pushed")).toEqual({
      type: "info",
      title: "Canceled the push of auth before it started",
      description: "Nothing was pushed",
    })
    expect(canceledBeforeStart(actionOfLabel("Squashing 2 commits"))).toEqual({
      type: "info",
      title: "Canceled squashing 2 commits before it started",
      description: "Nothing changed",
    })
  })
})
