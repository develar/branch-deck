// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { Intent, RunningOperation, TableWitness } from "@/lib/bindings"
import type { OperationNames } from "@/presenters/operations"
import { presentOperationBusy, presentOperationLabel, presentOtherProcess } from "@/presenters/operations"

/** `intent` that `bd` with pid 4242 runs, as the session shows it from the operations registry of the repository */
function ofBd(intent: Intent): RunningOperation {
  return { id: "op", intent, resources: [], step: { kind: "checking" }, origin: { kind: "otherProcess", tool: "bd", pid: 4242 }, moves: [] }
}

const WITNESS: TableWitness = { head: "h1", storeTips: { issueGroups: null, stackParents: null, parked: null } }

const SAFE_PUSH: Intent = { kind: "land", subject: { kind: "branch", name: "auth" }, mode: "rebaseAutosquash", profile: { kind: "default" } }

describe("presentOtherProcess", () => {
  it("words an operation of another process as its row says it and as the hover names its pid", () => {
    expect(presentOtherProcess(ofBd(SAFE_PUSH))).toEqual({
      label: "bd: Safe Push of auth",
      description: "`bd` (pid 4242) runs the Safe Push of auth, which holds what it needs until it ends",
      unknown: false,
    })
    expect(presentOtherProcess(ofBd({ ...SAFE_PUSH, mode: "dryRun", subject: { kind: "wholeStack", root: "air" } }))?.label).toBe("bd: Dry Run of air")
    const push: Intent = { kind: "push", subject: { kind: "branch", name: "auth" }, scope: { kind: "alone" }, force: false, restartRunning: false, expectRemote: null }
    expect(presentOtherProcess(ofBd(push))?.label).toBe("bd: Pushing branch auth")
    expect(presentOtherProcess({ ...ofBd({ kind: "retarget", number: 12 }), origin: { kind: "otherProcess", tool: "app", pid: 7 } })).toEqual({
      label: "Branch Deck: Retargeting #12",
      description: "Branch Deck (pid 7) runs a retarget of #12, which holds what it needs until it ends",
      unknown: false,
    })
  })

  it("words a start whose answer was lost: whether it started is not known, and how it lets go", () => {
    const lost = { ...ofBd(SAFE_PUSH), step: { kind: "startUnknown" as const, sentAt: 1_800_000_000, url: "https://patronus.example", pullRequest: null } }
    expect(presentOtherProcess(lost)).toEqual({
      label: "Start Unknown",
      description:
        "Cannot tell whether the Safe Push of auth started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start; Patronus may still run it",
      unknown: true,
    })
  })

  it("words nothing of an operation of this app", () => {
    expect(presentOtherProcess({ ...ofBd(SAFE_PUSH), origin: { kind: "thisProcess" } })).toBeNull()
  })
})

describe("presentOperationBusy", () => {
  it("says that an operation waits for the sync, behind another one, or what it does", () => {
    const unstack: RunningOperation = { ...ofBd(SAFE_PUSH), id: "unstack", origin: { kind: "thisProcess" }, step: { kind: "waitingForSync" } }
    const named: OperationNames = { unstack: { label: "Unstacking oauth" } }
    expect(presentOperationBusy(unstack, { operations: { unstack }, named })).toBe("Unstacking oauth after the sync")
    const squash: RunningOperation = { ...unstack, id: "squash", step: { kind: "queued", behind: "unstack" } }
    expect(presentOperationBusy(squash, { operations: { unstack, squash }, named })).toBe("Waits for Unstacking oauth")
    expect(presentOperationBusy({ ...unstack, step: { kind: "checking" } }, { operations: { unstack }, named })).toBe("Unstacking oauth")
    expect(presentOperationBusy({ ...squash, step: { kind: "queued", behind: "gone" } }, { operations: { squash }, named })).toBe("Waits for another operation")
  })

  it("names an operation by the one rule of its items: of another process as its row, of this window as it named it, else as the view", () => {
    const rename: Intent = { kind: "rewrite", rewrite: "rename", branchName: "auth", newName: "login", witness: WITNESS }
    const ofThisApp: RunningOperation = { ...ofBd(rename), id: "mine", origin: { kind: "thisProcess" }, step: { kind: "rewriting" } }
    const ofAgent: RunningOperation = { ...ofBd(rename), id: "agent", step: { kind: "rewriting" } }
    const named: OperationNames = { mine: { label: "Renaming auth to login from the row" }, agent: { label: "never shown" } }
    expect(presentOperationLabel(ofThisApp, named)).toBe("Renaming auth to login from the row")
    expect(presentOperationLabel(ofThisApp, {})).toBe("Renaming auth to login")
    // the "⋯" of the row of a rewrite of `bd` says it as `bd`'s, as an item an operation of `bd` keeps busy names it
    expect(presentOperationLabel(ofAgent, named)).toBe("bd: Renaming auth to login")
    expect(presentOperationBusy(ofAgent, { operations: { agent: ofAgent }, named })).toBe("bd: Renaming auth to login")
  })
})
