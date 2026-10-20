// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { RunningMove } from "@/lib/running-move"
import { findRunningMove, robotRestartMove, safePushStartMoves } from "@/lib/running-move"

describe("findRunningMove", () => {
  const landsBeside = (key: string): readonly string[] => ({ oauth: ["auth"], "stack:air": ["air", "bazel"] })[key] ?? []

  it("finds the own move of the row, else a move of a row whose Safe Push lands it beside", () => {
    const request: RunningMove = { key: "auth", lands: false, label: "Requesting a review on #12" }
    const restart = robotRestartMove("oauth", "rebaseAutosquash")
    expect(findRunningMove("auth", [restart, request], landsBeside)).toEqual({ move: request, with: null })
    expect(findRunningMove("auth", [restart], landsBeside)).toEqual({ move: restart, with: "oauth" })
    const stack = safePushStartMoves({ "stack:air": { mode: "rebaseAutosquash", step: "starting" } })
    expect(findRunningMove("bazel", stack, landsBeside)).toEqual({ move: stack[0], with: "the whole air stack" })
    expect(findRunningMove("stack:air", stack, landsBeside)).toEqual({ move: stack[0], with: null })
  })

  it("finds none for a move that lands nothing beside, or of a row that doesn't land it", () => {
    expect(findRunningMove("auth", [robotRestartMove("oauth", "dryRun")], landsBeside)).toBeNull()
    expect(findRunningMove("auth", [{ key: "oauth", lands: false, label: "Requesting a review on #12" }], landsBeside)).toBeNull()
    expect(findRunningMove("login", [robotRestartMove("oauth", "rebaseAutosquash")], landsBeside)).toBeNull()
  })
})

describe("safePushStartMoves", () => {
  it("is a move from the target check on, Landing for a Safe Push, Waiting for a Dry Run; none at its checks", () => {
    expect(
      safePushStartMoves({
        auth: { mode: "rebaseAutosquash", step: "target" },
        login: { mode: "dryRun", step: "pushing" },
        oauth: { mode: "rebaseAutosquash", step: "checking" },
      }),
    ).toEqual([
      { key: "auth", lands: true, label: "Landing: Safe Push starting" },
      { key: "login", lands: false, label: "Dry Run starting: it checks the branch, it lands nothing" },
    ])
  })
})
