// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ActionId, StackProblemView } from "@/lib/bindings"
import type { BranchDropFacts } from "@/lib/branch-drop"
import { moveDrop, parkDrop, showsParkDrop, showsUnparkDrop, stackOnDrop, unparkDrop, unstackDrop } from "@/lib/branch-drop"
import type { ParkedState } from "@/lib/parked"
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

/**
 * Why Rust refuses an action of a branch, as its menu words it: null while it can run, undefined while the branch doesn't
 * offer it. `viewer` failed for its stack problem: only its fixes run; a root offers no Unstack
 */
const RUST: Record<string, Partial<Record<ActionId, string | null>>> = {
  viewer: { "move-to": "viewer has conflicting parents", park: "viewer has conflicting parents", "stack-on": null, unstack: null },
  auth: { "move-to": null, park: null, "stack-on": null },
  docs: { "move-to": null, park: null, "stack-on": null },
  login: { "move-to": null, park: null, "stack-on": null, unstack: null },
  oauth: { "move-to": null, park: null, "stack-on": null, unstack: null },
}

/** The facts of a drop, the actions as Rust decides them (`RUST`), the operations of the session over them */
function facts(patch: Partial<Omit<BranchDropFacts, "blocker">> & { readonly rust?: typeof RUST } = {}): BranchDropFacts {
  const { rust = RUST, ...rest } = patch
  return {
    branch: name => branches[name],
    parked: name => parked[name] ?? null,
    blocker: (name, id) => rust[name]?.[id],
    descendants: name => descendants[name] ?? [],
    stacksEnabled: true,
    ...rest,
  }
}

/** The actions of `RUST` while the session unstacks login: its own item runs, a start of the others waits in the queue */
const UNSTACKING: typeof RUST = { ...RUST, login: { ...RUST.login, unstack: "Unstacking login…" } }

describe("branch drop", () => {
  it("parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and never waits for a sync", () => {
    expect(showsParkDrop(["docs"], facts())).toBe(true)
    // the Parked card is where they come from
    expect(showsParkDrop(["auth", "login"], facts())).toBe(false)
    expect(parkDrop(["docs"], facts())).toEqual({ names: ["docs"] })
    // a selection of both kinds: the parked ones stay as they are
    expect(parkDrop(["docs", "auth", "login"], facts())).toEqual({ names: ["docs"] })
    expect(parkDrop(["auth"], facts())).toEqual({ refusal: "Cannot park auth: It is parked already" })
    expect(parkDrop(["auth", "login"], facts())).toEqual({ refusal: "Cannot park: They are parked already" })
    // during a sync or a rewrite: the park goes to the session, which queues it
    expect(parkDrop(["docs"], facts({ rust: UNSTACKING }))).toEqual({ names: ["docs"] })

    // a robot lands it: as Park Branch… of its menu says it
    const held = "A Safe Push of docs is running: wait until it merges, or restart it on the current branch: it lands whatever a park says"
    expect(parkDrop(["docs"], facts({ rust: { ...RUST, docs: { park: held } } }))).toEqual({ refusal: `Cannot park docs: ${held}` })
  })

  it("unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land", () => {
    expect(showsUnparkDrop(["docs"], facts())).toBe(false)
    expect(showsUnparkDrop(["docs", "auth"], facts())).toBe(true)
    expect(unparkDrop(["auth"], facts())).toEqual({ names: ["auth"] })
    // the selection of the stack: only the root has a line
    expect(unparkDrop(["auth", "login", "oauth"], facts())).toEqual({ names: ["auth"] })
    expect(unparkDrop(["login"], facts())).toEqual({ refusal: "Cannot unpark login: It carries the commits of parked auth, drag auth here" })
    expect(unparkDrop(["auth", "docs"], facts())).toEqual({ names: ["auth"] })
  })

  it("stacks one dragged branch on another, never on itself, its parent or a branch stacked on it", () => {
    expect(stackOnDrop(["docs"], "oauth", facts())).toEqual({ names: ["docs"] })
    expect(stackOnDrop(["login"], "auth", facts())).toEqual({ refusal: "Cannot stack login on auth: It is stacked on auth already" })
    expect(stackOnDrop(["auth"], "oauth", facts())).toEqual({ refusal: "Cannot stack auth on oauth: oauth is stacked on it" })
    expect(stackOnDrop(["docs"], "auth", facts({ rust: UNSTACKING }))).toEqual({ names: ["docs"] })
    // no chip: its own row, two branches, stacked branches off
    expect(stackOnDrop(["docs"], "docs", facts())).toBeNull()
    expect(stackOnDrop(["docs", "auth"], "oauth", facts())).toBeNull()
    expect(stackOnDrop(["docs"], "auth", facts({ stacksEnabled: false }))).toBeNull()
  })

  it("blocks the rewrites of a branch with a stack problem by the reason Rust gives, and runs the fixes it offers", () => {
    expect(moveDrop(["viewer"], "docs", facts())).toEqual({ refusal: "Cannot move viewer to docs: viewer has conflicting parents" })
    // a fix of its problem runs
    expect(stackOnDrop(["viewer"], "tools", facts())).toEqual({ names: ["viewer"] })
    // no declared parent, yet the problem offers the unstack
    expect(unstackDrop(["viewer"], facts())).toEqual({ names: ["viewer"] })
  })

  it("unstacks one dragged branch that is stacked, and shows no zone for a root", () => {
    expect(unstackDrop(["login"], facts())).toEqual({ names: ["login"] })
    expect(unstackDrop(["docs"], facts())).toBeNull()
    expect(unstackDrop(["login", "oauth"], facts())).toBeNull()
    expect(unstackDrop(["login"], facts({ stacksEnabled: false }))).toBeNull()
    // Rust offers no Unstack of the branch: no zone
    expect(unstackDrop(["login"], facts({ rust: { login: {} } }))).toBeNull()
    // its own unstack runs in the session: as the menu says it
    expect(unstackDrop(["login"], facts({ rust: UNSTACKING }))).toEqual({ refusal: "Cannot unstack login from auth: Unstacking login…" })
  })

  it("moves dragged commits onto a branch, leaves a drag of branches that land to the Parked card from its rows, and waits as Move to… does", () => {
    expect(moveDrop([], "docs", facts())).toEqual({ names: [] })
    // whole branches: as Move to… of their menu, which a sync or a rewrite that runs leaves as it is
    expect(moveDrop(["docs"], "oauth", facts())).toBeNull()
    expect(moveDrop(["docs"], "login", facts({ parked: () => null }))).toEqual({ names: ["docs"] })
    expect(moveDrop(["docs"], "login", facts({ parked: () => null, rust: UNSTACKING }))).toEqual({ names: ["docs"] })
    expect(moveDrop(["viewer"], null, facts())).toEqual({ refusal: "Cannot move viewer back: viewer has conflicting parents" })
    // a row of the Parked card takes a drag of parked branches and of commits
    expect(moveDrop(["login"], "oauth", facts())).toEqual({ names: ["login"] })
    expect(moveDrop([], "oauth", facts())).toEqual({ names: [] })
  })
})
