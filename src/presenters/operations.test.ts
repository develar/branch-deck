// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { Intent, RunningOperation } from "@/lib/bindings"
import { presentOperationHolder, presentOtherProcess } from "@/presenters/operations"

/** `intent` that `bd` with pid 4242 runs, as the session shows it from the operations registry of the repository */
function ofBd(intent: Intent): RunningOperation {
  return { id: "op", intent, resources: [], step: { kind: "checking" }, origin: { kind: "otherProcess", tool: "bd", pid: 4242 }, moves: [] }
}

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
    expect(presentOtherProcess(ofBd(push))?.label).toBe("bd: Pushing auth")
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

describe("presentOperationHolder", () => {
  it("names the holder that made an operation busy, what it runs and for how long", () => {
    expect(presentOperationHolder({ op: "01K", tool: "bd", pid: 4242, kind: "safe_push", subject: "auth", held_for: 40 })).toBe(
      "`bd` (pid 4242) has run the Safe Push of auth for 40 s",
    )
    expect(presentOperationHolder({ op: "01K", tool: "app", pid: 7, kind: "close_landed_pull_requests", subject: null, held_for: 75 })).toBe(
      "Branch Deck (pid 7) has run the close of the pull requests a robot landed for 1 min 15 s",
    )
    expect(presentOperationHolder({ op: "01K", tool: "bd", pid: 4242, kind: "safe_push", subject: "auth", held_for: 90, unknown: true })).toBe(
      "Cannot tell whether the Safe Push of auth that `bd` (pid 4242) sent started: it holds until Patronus lists its robot, or until Forget This Start",
    )
  })
})
