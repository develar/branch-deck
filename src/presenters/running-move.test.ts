// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { RunningOperation } from "@/lib/bindings"
import type { RunningMove } from "@/presenters/running-move"
import { findRunningMove, presentOperationMoves, presentRunningMove, robotRestartMove } from "@/presenters/running-move"

describe("findRunningMove", () => {
  const landsBeside = (key: string): readonly string[] => ({ oauth: ["auth"], "stack:air": ["air", "bazel"] })[key] ?? []

  it("finds the own move of the row, else a move of a row whose Safe Push lands it beside", () => {
    const request: RunningMove = { key: "auth", lands: false, label: "Requesting a review on #12" }
    const restart = robotRestartMove("oauth", "rebaseAutosquash")
    expect(findRunningMove("auth", [restart, request], landsBeside)).toEqual({ move: request, with: null })
    expect(findRunningMove("auth", [restart], landsBeside)).toEqual({ move: restart, with: "oauth" })
    const stack = [presentRunningMove({ kind: "starting", subject: "stack:air", mode: "rebaseAutosquash" })]
    expect(findRunningMove("bazel", stack, landsBeside)).toEqual({ move: stack[0], with: "the whole air stack" })
    expect(findRunningMove("stack:air", stack, landsBeside)).toEqual({ move: stack[0], with: null })
  })

  it("finds none for a move that lands nothing beside, or of a row that doesn't land it", () => {
    expect(findRunningMove("auth", [robotRestartMove("oauth", "dryRun")], landsBeside)).toBeNull()
    expect(findRunningMove("auth", [{ key: "oauth", lands: false, label: "Requesting a review on #12" }], landsBeside)).toBeNull()
    expect(findRunningMove("login", [robotRestartMove("oauth", "rebaseAutosquash")], landsBeside)).toBeNull()
  })
})

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
