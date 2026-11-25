// @spec openspec/specs/branch/landing-plan/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingEdge, TogetherPrice } from "@/lib/bindings"
import { describeTogetherPrice, formatCheckCount, formatEdgeEvidence, formatEdgeState, formatTogetherPrice, formatUnitSize, formatWaveTitle } from "@/presenters/landing-plan"

const PRICE: TogetherPrice = {
  robots: 2,
  together: 1,
  checks: { kind: "same", count: 7 },
  failureHoldsBoth: true,
  approvalsIn: null,
  ownersAdded: [],
  githubStack: null,
}

const EDGE: LandingEdge = {
  from: "air-home-watch",
  needed: "acp-load-replay",
  kind: "build",
  state: { kind: "waits", robot: { branch: "acp-load-replay", robotId: "r1", wholeStack: false } },
  names: ["Effect", "Input", "ReplayRecord", "StepError", "step"],
  files: ["plugins/air/acp_load_replay.rs"],
  refusals: 15,
  refuses: ["air-home-watch", "hosted-exit", "claude-folds"],
  fix: null,
  actions: [],
}

describe("the words of the landing plan", () => {
  it("names the waves, the size and the checks of a unit", () => {
    expect(formatWaveTitle({ kind: { kind: "landing" }, units: ["genui-host-crate"] })).toBe("Landing now")
    expect(formatWaveTitle({ kind: { kind: "next" }, units: ["agent-id"] })).toBe("Next")
    expect(formatWaveTitle({ kind: { kind: "after", after: ["acp-load-replay", "genui-host-crate"] }, units: ["air-home-watch"] })).toBe(
      "After acp-load-replay and genui-host-crate",
    )
    expect(formatUnitSize(1, 3)).toBe("3 commits")
    expect(formatUnitSize(3, 12)).toBe("3 branches · 12 commits")
    expect(formatCheckCount(["Fast Tests"])).toBe("1 check")
    expect(formatCheckCount(Array.from({ length: 7 }, (_, index) => `Check ${index}`))).toBe("7 checks")
    expect(formatCheckCount(null)).toBeNull()
  })

  it("says an edge by its evidence in a few words and its state", () => {
    expect(formatEdgeEvidence(EDGE)).toBe("Effect, Input +3")
    expect(formatEdgeEvidence({ ...EDGE, names: ["Codec"] })).toBe("Codec")
    expect(formatEdgeEvidence({ ...EDGE, kind: "declared", names: [] })).toBe("declared")
    expect(formatEdgeState(EDGE)).toBe("landing")
    expect(formatEdgeState({ ...EDGE, state: { kind: "open" } })).toBe("nothing lands it")
    expect(formatEdgeState({ ...EDGE, state: { kind: "neededFailed", robot: { branch: "acp-load-replay", robotId: "r1", wholeStack: false } } })).toBe("robot failed")
  })

  it("words the price of Land Together as its risks, one by one", () => {
    expect(formatTogetherPrice(PRICE)).toBe("1 robot instead of 2 · same 7 checks · a failure holds both")
    // a stack that forks lands from the Safe Push of its whole stack
    expect(formatTogetherPrice(PRICE, true)).toBe("1 robot instead of 2, the whole stack · same 7 checks · a failure holds both")
    const wide = describeTogetherPrice({
      ...PRICE,
      checks: { kind: "added", count: 9, added: ["AI Assistant Tests", "Air Tests", "Fleet Tests"] },
      approvalsIn: 12,
      ownersAdded: ["IJPL Code Platform", "Air"],
      githubStack: [12, 13],
    })
    expect(wide.map(risk => risk.text)).toEqual([
      "1 robot instead of 2",
      "+ AI Assistant Tests +2",
      "a failure holds both",
      "approvals count in #12",
      "asks IJPL Code Platform +1 too",
      "#12 and #13 become one GitHub stack",
    ])
    expect(wide[1]!.title).toBe("9 checks together; only one of them starts AI Assistant Tests, Air Tests and Fleet Tests")
    const unknown = describeTogetherPrice({ ...PRICE, checks: { kind: "unknown", reason: "no .patronus/config.yaml in master" } })
    expect(unknown[1]).toEqual({ text: "checks unknown", title: "no .patronus/config.yaml in master" })
  })
})
