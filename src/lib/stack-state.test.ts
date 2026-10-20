import { describe, expect, it } from "vite-plus/test"
import type { LandingNodeKind } from "@/lib/landing-node"
import { firstBranchOfEachState, getStackStateChips, summarizeStackState } from "@/lib/stack-state"
import type { StackState } from "@/lib/stack-state"

/** The node of each row by name, as `getLandingNodeNow` gives it; none for a branch not listed */
function nodes(kinds: Record<string, LandingNodeKind>): (name: string) => LandingNodeKind | null {
  return name => kinds[name] ?? null
}

const NONE: StackState = { needsFix: 0, yourStep: 0, ready: 0, landing: 0, waiting: 0, parked: 0, inMaster: 0 }

describe("summarizeStackState", () => {
  it("counts the rows of a stack by their nodes, and nothing of a branch without one", () => {
    const kinds = nodes({ session: "needsFix", login: "yourStep", oauth: "ready", docs: "landing", api: "landing", web: "waiting", old: "inMaster" })
    const names = ["session", "login", "oauth", "docs", "api", "web", "old", "syncing"]
    expect(summarizeStackState(names, kinds)).toEqual({ needsFix: 1, yourStep: 1, ready: 1, landing: 2, waiting: 1, parked: 0, inMaster: 1 })
    expect(firstBranchOfEachState(names, kinds)).toEqual({ needsFix: "session", yourStep: "login", ready: "oauth", landing: "docs", waiting: "web", inMaster: "old" })
    expect(summarizeStackState([], kinds)).toEqual(NONE)
  })

  it("counts a fix of the whole stack, whose stack branch has no row, as the root's", () => {
    const kinds = nodes({ air: "yourStep", bazel: "yourStep" })
    expect(summarizeStackState(["air", "bazel"], kinds, "needsFix")).toEqual({ ...NONE, needsFix: 1, yourStep: 2 })
    expect(firstBranchOfEachState(["air", "bazel"], kinds, "needsFix", "air")).toEqual({ needsFix: "air", yourStep: "air" })
    // any other node of the whole stack is what its rows show already: a running robot of it makes them land
    expect(summarizeStackState(["air", "bazel"], kinds, "landing")).toEqual({ ...NONE, yourStep: 2 })
  })
})

describe("getStackStateChips", () => {
  it("puts the user's moves first, a fix before a step before ready, then what lands, waits and is in master, in the colours of the nodes", () => {
    const all: StackState = { needsFix: 2, yourStep: 1, ready: 3, landing: 1, waiting: 4, parked: 0, inMaster: 1 }
    expect(getStackStateChips(all)).toEqual([
      { state: "needsFix", text: "2 to fix", color: "error", ring: false },
      { state: "yourStep", text: "1 your step", color: "primary", ring: false },
      { state: "ready", text: "3 ready", color: "success", ring: false },
      // a Safe Push lands them: the ring of its robot
      { state: "landing", text: "1 landing", color: "primary", ring: true },
      { state: "waiting", text: "4 waiting", color: "neutral", ring: false },
      { state: "inMaster", text: "1 in master", color: "info", ring: false },
    ])
    expect(getStackStateChips({ ...NONE, waiting: 2 }).map(chip => chip.text)).toEqual(["2 waiting"])
    expect(getStackStateChips(NONE)).toEqual([])
  })
})
