// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LockHolderInfo, OperationStep, RunningOperation } from "@/lib/bindings"
import { presentLockHolder, presentLockWait, presentOperationLockWait } from "@/presenters/lock-wait"

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
