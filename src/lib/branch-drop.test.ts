// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { BranchDropFacts } from "@/lib/branch-drop"
import { parkBlocker, parkDrop, showsParkDrop, showsUnparkDrop, stackOnDrop, unparkDrop, unstackDrop } from "@/lib/branch-drop"
import type { ParkedState } from "@/lib/parked"
import type { PushHold } from "@/lib/patronus"
import { testRobot } from "@/lib/test-robot"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch } from "@/stores/test-branch"

/** A branch of one commit, stacked on `parent` */
function branchOf(name: string, parent: string | null): Branch {
  const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
  return testBranch(name, parent, { commits: [tip] })
}

// auth <- login <- oauth, docs alone; auth is parked, login and oauth with it
const branches: Record<string, Branch> = {
  auth: branchOf("auth", null),
  login: branchOf("login", "auth"),
  oauth: branchOf("oauth", "login"),
  docs: branchOf("docs", null),
}
const parked: Record<string, ParkedState> = {
  auth: { note: "", with: null },
  login: { note: "", with: "auth" },
  oauth: { note: "", with: "auth" },
}
const descendants: Record<string, string[]> = { auth: ["login", "oauth"], login: ["oauth"] }

function facts(patch: Partial<BranchDropFacts> = {}): BranchDropFacts {
  return {
    branch: name => branches[name],
    parked: name => parked[name] ?? null,
    hold: () => null,
    descendants: name => descendants[name] ?? [],
    stacksEnabled: true,
    isSyncing: false,
    ...patch,
  }
}

describe("branch drop", () => {
  it("parks dragged branches that land, refuses one parked already or one a robot lands, and waits for the sync", () => {
    expect(showsParkDrop(["docs"], facts())).toBe(true)
    // the Parked card is where they come from
    expect(showsParkDrop(["auth", "login"], facts())).toBe(false)
    expect(parkDrop(["docs"], facts())).toEqual({ names: ["docs"] })
    expect(parkDrop(["docs", "auth"], facts())).toEqual({ refusal: "Cannot park auth: It is parked already" })
    expect(parkDrop(["docs", "login"], facts())).toEqual({ refusal: "Cannot park login: It is parked with auth" })
    expect(parkDrop(["docs"], facts({ isSyncing: true }))).toEqual({ refusal: "Cannot park docs: Wait for the sync to finish" })

    const own: PushHold = { kind: "own", branch: "docs", robot: testRobot("r1", "docs") }
    const refusal = parkDrop(["docs"], facts({ hold: name => (name === "docs" ? own : null) }))
    expect(refusal).toEqual({ refusal: expect.stringMatching(/^Cannot park docs: A .+ of docs is running.*: it lands whatever a park says$/) })
    // a landed hold parks: nothing of it is left to land anyway
    expect(parkBlocker(branches.docs!, { ...own, kind: "landed" }, false)).toBeNull()
    expect(parkBlocker(testBranch("empty", null, { commitCount: 0 }), null, false)).toBe("The branch has no commits")
  })

  it("unparks the dragged branches with a line of their own, the ones parked with them along, never one alone", () => {
    expect(showsUnparkDrop(["docs"], facts())).toBe(false)
    expect(showsUnparkDrop(["docs", "auth"], facts())).toBe(true)
    expect(unparkDrop(["auth"], facts())).toEqual({ names: ["auth"] })
    // the selection of the stack: only the root has a line
    expect(unparkDrop(["auth", "login", "oauth"], facts())).toEqual({ names: ["auth"] })
    expect(unparkDrop(["login"], facts())).toEqual({ refusal: "Cannot unpark login: It carries the commits of parked auth, drag auth here" })
    expect(unparkDrop(["auth", "docs"], facts())).toEqual({ refusal: "Cannot unpark docs: It is not parked" })
    expect(unparkDrop(["auth"], facts({ isSyncing: true }))).toEqual({ refusal: "Cannot unpark: Wait for the sync to finish" })
  })

  it("stacks one dragged branch on another, never on itself, its parent or a branch stacked on it", () => {
    expect(stackOnDrop(["docs"], "oauth", facts())).toEqual({ names: ["docs"] })
    expect(stackOnDrop(["login"], "auth", facts())).toEqual({ refusal: "Cannot stack login on auth: It is stacked on auth already" })
    expect(stackOnDrop(["auth"], "oauth", facts())).toEqual({ refusal: "Cannot stack auth on oauth: oauth is stacked on it" })
    expect(stackOnDrop(["docs"], "auth", facts({ isSyncing: true }))).toEqual({ refusal: "Cannot stack docs on auth: Wait for the sync to finish" })
    // no chip: its own row, two branches, stacked branches off
    expect(stackOnDrop(["docs"], "docs", facts())).toBeNull()
    expect(stackOnDrop(["docs", "auth"], "oauth", facts())).toBeNull()
    expect(stackOnDrop(["docs"], "auth", facts({ stacksEnabled: false }))).toBeNull()
  })

  it("unstacks one dragged branch that is stacked, and shows no zone for a root", () => {
    expect(unstackDrop(["login"], facts())).toEqual({ names: ["login"] })
    expect(unstackDrop(["docs"], facts())).toBeNull()
    expect(unstackDrop(["login", "oauth"], facts())).toBeNull()
    expect(unstackDrop(["login"], facts({ stacksEnabled: false }))).toBeNull()
    expect(unstackDrop(["login"], facts({ isSyncing: true }))).toEqual({ refusal: "Cannot unstack login from auth: Wait for the sync to finish" })
  })
})
