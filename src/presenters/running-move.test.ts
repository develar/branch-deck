// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { RunningOperation } from "@/lib/bindings"
import { presentOperationMoves } from "@/presenters/running-move"

/** An operation of the session that makes `moves` */
function operation(id: string, moves: RunningOperation["moves"]): RunningOperation {
  return {
    id,
    intent: { kind: "retarget", number: 12 },
    resources: [],
    step: { kind: "checking" },
    origin: { kind: "thisProcess" },
    moves,
  }
}

describe("presentOperationMoves", () => {
  it("words the moves of the operations of the session as the node of each row says them", () => {
    const operations = {
      a: operation("a", [{ kind: "starting", subject: "stack:air", mode: "rebaseAutosquash" }]),
      b: operation("b", [
        { kind: "restarting", subject: "auth", mode: "dryRun" },
        { kind: "creatingPullRequest", subject: "stack:air" },
        { kind: "requestingReview", subject: "login", number: 1078 },
      ]),
      c: operation("c", []),
    }
    expect(presentOperationMoves(operations)).toEqual([
      { key: "stack:air", lands: true, label: "Landing: Safe Push starting" },
      { key: "auth", lands: false, label: "Dry Run restarting: it checks the branch, it lands nothing" },
      { key: "stack:air", lands: false, label: "Creating the pull request of the whole air stack" },
      { key: "login", lands: false, label: "Requesting a review on #1078" },
    ])
  })
})
