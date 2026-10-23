// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { StackProblemView } from "@/lib/bindings"
import type { BranchDropFacts } from "@/lib/branch-drop"
import { moveDrop, parkBlocker, parkDrop, rewriteBlocker, showsParkDrop, showsUnparkDrop, stackOnDrop, unparkDrop, unstackDrop } from "@/lib/branch-drop"
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
// `viewer` declares two parents: the problem Rust reports, with its fixes
const conflict: StackProblemView = {
  problem: { ConflictingParents: [] },
  label: "conflicting parents",
  reason: "viewer has conflicting parents",
  message: "Commits declare different parents: ci (c1), tools (its stack parent)",
  commits: [],
  fixes: [{ StackOn: "ci" }, { StackOn: "tools" }, "Unstack"],
}
const base = branchOf("viewer", null)
branches.viewer = { ...base, status: "Error", hasError: true, stack: { ...base.stack!, declaredParent: null, effectiveBase: null, problems: [conflict] } }
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
  it("parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and waits for the sync", () => {
    expect(showsParkDrop(["docs"], facts())).toBe(true)
    // the Parked card is where they come from
    expect(showsParkDrop(["auth", "login"], facts())).toBe(false)
    expect(parkDrop(["docs"], facts())).toEqual({ names: ["docs"] })
    // a selection of both kinds: the parked ones stay as they are
    expect(parkDrop(["docs", "auth", "login"], facts())).toEqual({ names: ["docs"] })
    expect(parkDrop(["auth"], facts())).toEqual({ refusal: "Cannot park auth: It is parked already" })
    expect(parkDrop(["auth", "login"], facts())).toEqual({ refusal: "Cannot park: They are parked already" })
    expect(parkDrop(["docs"], facts({ isSyncing: true }))).toEqual({ refusal: "Cannot park docs: Wait for the sync to finish" })

    const own: PushHold = { kind: "own", branch: "docs", robot: testRobot("r1", "docs") }
    const refusal = parkDrop(["docs"], facts({ hold: name => (name === "docs" ? own : null) }))
    expect(refusal).toEqual({ refusal: expect.stringMatching(/^Cannot park docs: A .+ of docs is running.*: it lands whatever a park says$/) })
    // a landed hold parks: nothing of it is left to land anyway
    expect(parkBlocker(branches.docs!, { ...own, kind: "landed" }, false)).toBeNull()
    expect(parkBlocker(testBranch("empty", null, { commitCount: 0 }), null, false)).toBe("The branch has no commits")
  })

  it("unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land", () => {
    expect(showsUnparkDrop(["docs"], facts())).toBe(false)
    expect(showsUnparkDrop(["docs", "auth"], facts())).toBe(true)
    expect(unparkDrop(["auth"], facts())).toEqual({ names: ["auth"] })
    // the selection of the stack: only the root has a line
    expect(unparkDrop(["auth", "login", "oauth"], facts())).toEqual({ names: ["auth"] })
    expect(unparkDrop(["login"], facts())).toEqual({ refusal: "Cannot unpark login: It carries the commits of parked auth, drag auth here" })
    expect(unparkDrop(["auth", "docs"], facts())).toEqual({ names: ["auth"] })
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

  it("blocks the rewrites of a branch with a stack problem by its reason, and runs the fixes Rust offers for it", () => {
    const viewer = branches.viewer!
    expect(rewriteBlocker(viewer, false)).toBe("viewer has conflicting parents")
    expect(moveDrop(["viewer"], "docs", facts())).toEqual({ refusal: "Cannot move viewer to docs: viewer has conflicting parents" })
    // a fix runs, another parent is no fix
    expect(stackOnDrop(["viewer"], "tools", facts())).toEqual({ names: ["viewer"] })
    expect(stackOnDrop(["viewer"], "docs", facts())).toEqual({ refusal: "Cannot stack viewer on docs: viewer has conflicting parents" })
    expect(stackOnDrop(["viewer"], "ci", facts({ isSyncing: true }))).toEqual({ refusal: "Cannot stack viewer on ci: Wait for the sync to finish" })
    // no declared parent, yet the problem offers the unstack
    expect(unstackDrop(["viewer"], facts())).toEqual({ names: ["viewer"] })
    // another failure says so
    expect(rewriteBlocker({ ...viewer, stack: { ...viewer.stack!, problems: [] } }, false)).toBe("The branch failed to sync")
  })

  it("unstacks one dragged branch that is stacked, and shows no zone for a root", () => {
    expect(unstackDrop(["login"], facts())).toEqual({ names: ["login"] })
    expect(unstackDrop(["docs"], facts())).toBeNull()
    expect(unstackDrop(["login", "oauth"], facts())).toBeNull()
    expect(unstackDrop(["login"], facts({ stacksEnabled: false }))).toBeNull()
    expect(unstackDrop(["login"], facts({ isSyncing: true }))).toEqual({ refusal: "Cannot unstack login from auth: Wait for the sync to finish" })
  })

  it("moves dragged commits onto a branch, leaves a drag of branches that land to the Parked card from its rows, and waits as Move to… does", () => {
    // commit rows: during a sync too, as Move to… of a commit
    expect(moveDrop([], "docs", facts({ isSyncing: true }))).toEqual({ names: [] })
    // whole branches: as Move to… of their menu
    expect(moveDrop(["docs"], "oauth", facts())).toBeNull()
    expect(moveDrop(["docs"], "login", facts({ parked: () => null }))).toEqual({ names: ["docs"] })
    expect(moveDrop(["docs"], "login", facts({ parked: () => null, isSyncing: true }))).toEqual({ refusal: "Cannot move docs to login: Wait for the sync to finish" })
    expect(moveDrop(["docs"], null, facts({ isSyncing: true }))).toEqual({ refusal: "Cannot move docs back: Wait for the sync to finish" })
    // a row of the Parked card takes a drag of parked branches and of commits
    expect(moveDrop(["login"], "oauth", facts())).toEqual({ names: ["login"] })
    expect(moveDrop([], "oauth", facts())).toEqual({ names: [] })
  })
})
