// @spec spec/branch/branch-review.spec.md
// @spec spec/branch/stacked-branches.spec.md
// @spec spec/branch/branch-row.spec.md
// The branch menu presents the actions Rust decides (`RepositoryView.actions`, `domain_policy::actions`, its table test):
// these tests set what Rust answers, the operations of the session laid over it, and check how the menu lays it out and
// names it
import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import { getBranchContextMenuItems, getBranchReviewItems } from "@/hooks/useBranchContextActions"
import type { ActionId, ActionState, BranchPush, ChildrenPush } from "@/lib/bindings"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { findMenuItem } from "@/lib/menu"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { CREATE, forcePush, PUSH, placed, testBranch, testRemoteStatus, testTip } from "@/stores/test-branch"
import { disabled, enabled, LANDING_BRANCH, setActions, withActions } from "@/test-support/actions"
import { pendingStep, setNextSteps, testStep } from "@/test-support/next-step"

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

const unpushed = testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, unpushedCommits: ["c"], action: CREATE })

/** A synced branch of one commit that the menu can change */
function synced(name: string, parent: string | null, remoteStatus: RemoteStatus, overrides: Partial<Branch> = {}): Branch {
  const tip = testTip(name)
  return testBranch(name, parent, { commits: [tip], remoteStatus, ...overrides })
}

/** The branches of the table, each with the actions of a branch that lands, as Rust offers them on Space */
function setBranches(...branches: Branch[]): void {
  const branchOrder = branches.map(branch => branch.name)
  branchSyncStore.setState({
    ...placed({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder }),
    branchOrder,
    stacksEnabled: true,
    pushes: {},
    actions: Object.fromEntries(branches.map(branch => [branch.name, enabled(...LANDING_BRANCH)])),
  })
}

/** The actions of the branch `name` as Rust offers them: those `setBranches` set, with `changes` */
function setBranchActions(name: string, ...changes: ActionState[]): void {
  const actions = branchSyncStore.getState().actions
  setActions({ ...actions, [name]: withActions(actions[name] ?? [], ...changes) })
}

/** The actions of the subject `key` as Rust offers them: exactly `ids` */
function offerOnly(key: string, ...ids: ActionId[]): void {
  setActions({ ...branchSyncStore.getState().actions, [key]: enabled(...ids) })
}

/** What the pushes of the branches send, as Rust orders them over the table the test set (`RepositoryView.pushes`) */
function setPushes(pushes: Record<string, BranchPush>): void {
  branchSyncStore.setState({ pushes })
}

/** Push with Children as Rust orders it: what it pushes (`order`), whether it pushes more than the branch, the rest */
function withChildren(order: readonly string[], offers: boolean, more: Omit<ChildrenPush, "order" | "offers"> = {}): BranchPush {
  return { children: { order: [...order], offers, ...more } }
}

/** The groups of the menu of `name` */
function groupsOf(name: string): MenuItemGroups {
  return getBranchContextMenuItems(branchSyncStore.getState().branches[name]!, () => {})
}

/** The items of the menu of `name`, the actions of the branch in "Patronus ›" and "Copy ›" too, after their submenu */
function menuOf(name: string): MenuItem[] {
  return groupsOf(name)
    .flat()
    .flatMap(item => (item.id === "patronus" || item.id === "copy" ? [item, ...item.children!] : [item]))
}

function itemOf(name: string, id: string): MenuItem | undefined {
  return menuOf(name).find(item => item.id === id)
}

afterEach(() => {
  branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, nextSteps: {}, actions: {}, isSyncing: false, operations: {}, stackBranches: {}, pushes: {} })
})

/** The push of the stack started from `startedFrom` of the branches `names` runs, an operation of the session */
function setStackPush(startedFrom: string, names: readonly string[]): void {
  const intent = {
    kind: "push",
    subject: { kind: "branch", name: startedFrom },
    scope: { kind: "withAncestors" },
    expectRemote: null,
    force: false,
    restartRunning: false,
  } as const
  const resources = names.map(name => ({ kind: "ref" as const, name: `user/virtual/${name}` }))
  branchSyncStore.setState({ operations: { push: { id: "push", intent, resources, step: { kind: "pushing" }, origin: { kind: "thisProcess" }, moves: [] } } })
}

describe("Safe Push of the branch menu", () => {
  it("says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it", () => {
    setBranches(synced("login", null, unpushed))
    const step = disabled("safe-push", { code: "step", blocker: { code: "step" } })
    // what the start decides on is not known yet (Rust's row 0): Patronus, then GitHub, then the readiness
    for (const [fact, reason] of [
      ["patronus", "Waits for the answer of Patronus"],
      ["pullRequests", "Waits for GitHub to list the pull requests"],
      ["readiness", "Waits for Patronus to check the approvals"],
    ] as const) {
      setNextSteps({ login: pendingStep(fact) })
      setBranchActions("login", step)
      expect(itemOf("login", "safe-push")).toMatchObject({ disabled: true, description: reason })
    }
    setNextSteps({ login: testStep({ kind: "settled" }, { safePush: { code: "syncFailed", problem: null } }) })
    setBranchActions("login", disabled("safe-push", { code: "step", blocker: { code: "syncFailed", problem: null } }))
    expect(itemOf("login", "safe-push")).toMatchObject({ disabled: true, description: "The branch failed to sync" })
    setNextSteps({ login: testStep({ kind: "safePush" }) })
    setBranchActions("login", enabled("safe-push")[0]!)
    expect(itemOf("login", "safe-push")?.disabled).toBeUndefined()
    const witness = { head: "h", storeTips: { issueGroups: null, stackParents: null, parked: null } }
    branchSyncStore.setState({
      operations: {
        rename: {
          id: "rename",
          intent: { kind: "rewrite", rewrite: "rename", branchName: "login", newName: "auth", witness },
          resources: [{ kind: "head" }],
          step: { kind: "rewriting" },
          origin: { kind: "thisProcess" },
          moves: [],
        },
      },
    })
    // a rewrite of the row in the session: the step waits for it
    expect(itemOf("login", "safe-push")).toMatchObject({ disabled: true, description: "Renaming login to auth…" })
  })
})

describe("the items of the branch menu", () => {
  it("says why an item doesn't apply in its description, never in its name, and names every item by a stable id", () => {
    setBranches(synced("auth", null, unpushed, { allCommitsHaveIssueReferences: true }), synced("login", "auth", unpushed))
    setBranchActions("auth", disabled("add-issue-reference", { code: "allReferenced" }))
    const items = menuOf("auth")
    expect(items.filter(item => item.id === undefined).map(item => item.label)).toEqual([])
    expect(items.find(item => item.id === "add-issue-reference")).toMatchObject({
      label: "Add Issue Reference…",
      disabled: true,
      description: "Every commit of the branch has an issue reference",
    })
    // login is stacked on auth: no other branch is left for auth
    expect(items.find(item => item.id === "stack-on")).toMatchObject({ disabled: true, description: "No other branch to stack on" })
    const docs = synced("docs", null, unpushed)
    setBranches(branchSyncStore.getState().branches.auth!, branchSyncStore.getState().branches.login!, docs)
    expect(menuOf("auth").find(item => item.id === "stack-on")).toMatchObject({ children: [{ label: "docs" }] })
    expect(menuOf("auth").find(item => item.id === "stack-on")?.disabled).toBeUndefined()
  })

  it("offers only what Rust offers: no item of an action it doesn't offer, none hidden it does", () => {
    setBranches(synced("auth", null, unpushed))
    offerOnly("auth", "push", "review-changes")
    expect(groupsOf("auth").map(group => group.map(item => item.id))).toEqual([["push"], ["review-changes"], ["copy"]])
  })
})

describe("Whole Stack › of the branch menu", () => {
  it("is the submenu of the root's whole stack on every branch of the stack, none on a branch of no stack", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed), synced("oauth", "login", unpushed), synced("docs", null, unpushed))
    offerOnly("stack:auth", "review-whole-stack")
    const reviewOf = (name: string) =>
      menuOf(name)
        .find(item => item.id === "whole-stack")
        ?.children?.find(item => item.id === "review-whole-stack")
    for (const name of ["auth", "login", "oauth"]) {
      expect(reviewOf(name)).toMatchObject({ label: "Review Whole Stack" })
    }
    expect(menuOf("docs").some(item => item.id === "whole-stack")).toBe(false)
  })
})

describe("the branch menu during a sync", () => {
  it("keeps every item during a sync, the rewrites enabled: the session admits them at once; the rest stays as it is", () => {
    setBranches(synced("auth", null, unpushed), synced("docs", null, unpushed))
    const before = menuOf("auth")
    branchSyncStore.setState({ isSyncing: true })
    const items = menuOf("auth")
    expect(items.map(item => [item.id, item.disabled])).toEqual(before.map(item => [item.id, item.disabled]))
    for (const id of ["rename", "move-to", "stack-on", "add-issue-reference", "amend-changes", "unapply", "park", "review-changes", "push"]) {
      expect(items.find(item => item.id === id)?.disabled, id).toBeUndefined()
    }
  })

  it("shows the rewrite that runs in the session running, and says what the start of every other rewrite waits behind", () => {
    setBranches(synced("auth", null, unpushed), synced("docs", null, unpushed))
    const witness = { head: "h", storeTips: { issueGroups: null, stackParents: null, parked: null } }
    branchSyncStore.setState({
      operations: {
        u1: {
          id: "u1",
          intent: { kind: "rewrite", rewrite: "unstack", branchName: "docs", witness },
          resources: [{ kind: "head" }],
          step: { kind: "rewriting" },
          origin: { kind: "thisProcess" },
          moves: [],
        },
      },
    })
    setBranchActions("auth", { id: "rename", availability: { kind: "waits", op: "u1" } }, { id: "park", availability: { kind: "running", op: "u1" } })
    expect(itemOf("auth", "rename")).toMatchObject({ description: "Waits for Unstacking docs…" })
    expect(itemOf("auth", "rename")?.disabled).toBeUndefined()
    expect(itemOf("auth", "park")).toMatchObject({ running: true, disabled: true, description: "Unstacking docs…" })
  })
})

describe("the push of the branch menu", () => {
  /** The items of the menu of `name` that push the branch, or its ancestors with it */
  function pushItems(name: string): MenuItem[] {
    return menuOf(name).filter(item => item.id === "push" || item.label.startsWith("Push Stack"))
  }

  it("is one item named as the push button, pushing the stack as far as the button does", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    setPushes({ login: { button: true }, auth: { children: { offers: true } } })
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", label: "Push Stack (2 branches)", description: "Pushes auth and login, parents first" })])
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", label: "Push (1)" })])
    expect(pushItems("auth")[0]?.disabled).toBeUndefined()
  })

  it("offers the push of the branch alone after the push of the stack, as the description of the button does", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    setPushes({ login: { button: true }, auth: { children: { offers: true } } })
    const items = menuOf("login")
    const push = items.findIndex(item => item.id === "push")
    expect(items[push + 1]).toMatchObject({ id: "push-branch-only", label: "Push login Only", disabled: false })
    expect(items[push + 1]?.kbds).toBeUndefined()
    // a branch whose button pushes it alone has no second way in the menu
    expect(menuOf("auth").some(item => item.id === "push-branch-only")).toBe(false)
  })

  it("pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row", () => {
    const dropping = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, unpushedCommits: ["c"], action: forcePush({ drops: true }) })
    setBranches(synced("auth", null, dropping), synced("login", "auth", unpushed))
    setPushes({ login: { action: [] } })
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", label: "Push (1)" })])
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", label: "Force Push (1)" })])
  })

  it("runs while its push of the stack runs in the session, and waits for another push that holds the branch", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    setStackPush("login", ["auth", "login"])
    setBranchActions("login", { id: "push", availability: { kind: "running", op: "push" } })
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", running: true, disabled: true, description: "Pushing login…" })])
    setBranchActions("auth", disabled("push", { code: "busy", op: "push" }))
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", disabled: true, description: "Pushing login…" })])
  })

  it("is disabled with the reason Rust gives: a branch the last sync did not build names the status of its row, not up to date", () => {
    setBranches({ ...synced("auth", null, unpushed), status: "MergeConflict", hasError: true, statusText: "merge conflict" })
    setBranchActions("auth", disabled("push", { code: "syncFailed", problem: null }))
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", disabled: true, description: "The branch failed to sync: merge conflict" })])
    setBranchActions("auth", disabled("push", { code: "upToDate" }))
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", disabled: true, description: "The remote branch is up to date" })])
  })
})

describe("Push with Children in the branch menu", () => {
  const pushed = testRemoteStatus({ remoteTip: "tip" })

  function pushWithChildren(name: string): MenuItem | undefined {
    return itemOf(name, "push-with-children")
  }

  it("takes the stack branch of the whole stack of a root along, counted and named, and says why it doesn't in Rust's words", () => {
    const stackBranch = (remoteStatus: RemoteStatus): StackBranch => ({
      root: "auth",
      branchName: "user/stack/auth",
      members: ["auth", "login", "session"],
      commitCount: 3,
      state: "Built",
      tip: "stack-tip",
      blockedBy: null,
      conflict: null,
      error: null,
      remoteStatus,
    })
    setBranches(synced("auth", null, pushed), synced("login", "auth", pushed), synced("session", "auth", pushed))
    branchSyncStore.setState({ stackBranches: { auth: stackBranch(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null, action: CREATE })) } })
    setPushes({ auth: withChildren([], false, { stackBranch: { kind: "unpublished" } }) })
    setBranchActions("auth", disabled("push-with-children", { code: "nothingBeyond", why: { kind: "stackBranch", along: { kind: "unpublished" } } }))
    // not on the remote and no pull request: the branches are, and "Nothing to push" would not be true
    expect(pushWithChildren("auth")).toMatchObject({
      label: "Push with Children",
      disabled: true,
      description: "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack",
    })
    branchSyncStore.setState({ stackBranches: { auth: stackBranch(testRemoteStatus({ commitsAhead: 1, unpushedCommits: ["c"], remoteTip: "old", action: PUSH })) } })
    setPushes({ auth: withChildren([], true, { stackBranch: { kind: "along" } }) })
    setBranchActions("auth", enabled("push-with-children")[0]!)
    expect(pushWithChildren("auth")).toMatchObject({ label: "Push with Children (the whole stack branch)", description: "Also pushes the whole stack branch user/stack/auth" })
    setPushes({ auth: withChildren(["login"], true, { stackBranch: { kind: "along" } }) })
    expect(pushWithChildren("auth")).toMatchObject({ label: "Push with Children (2 branches)" })
    expect(pushWithChildren("auth")?.disabled).toBeUndefined()
  })

  it("is offered beside the push where Rust offers it, with its label of what it pushes, and the stack push over it", () => {
    setBranches(synced("auth", null, pushed), synced("login", "auth", unpushed), synced("docs", null, unpushed))
    setPushes({ auth: withChildren(["login"], true) })
    setBranchActions("auth", enabled("push-with-children")[0]!)
    // the remote group: the push of the branch, then the push with its children
    const items = menuOf("auth")
    expect(items[items.findIndex(item => item.id === "push") + 1]).toMatchObject({ id: "push-with-children", label: "Push with Children (login)" })
    // a branch without children has nothing to push with them
    expect(menuOf("login").some(item => item.id === "push-with-children")).toBe(false)
    setStackPush("login", ["login"])
    setBranchActions("auth", disabled("push-with-children", { code: "busy", op: "push" }))
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "Pushing login…" })
  })
})

describe("the order of the branch menu", () => {
  /** The ids of each group, a submenu as its id */
  function idsOf(name: string): (string | undefined)[][] {
    return groupsOf(name).map(group => group.map(item => item.id))
  }

  it("leads with Safe Push, keeps every keyed item on top and puts the rarer starts and the copies into submenus", () => {
    setBranches(synced("auth", null, unpushed), synced("docs", null, unpushed))
    expect(idsOf("auth")).toEqual([
      ["safe-push", "patronus", "push"],
      ["review-changes", "create-pull-request"],
      ["rename", "move-to", "stack-on"],
      ["amend-changes", "add-issue-reference"],
      ["park", "unapply"],
      ["copy"],
    ])
    const patronus = groupsOf("auth")
      .flat()
      .find(item => item.id === "patronus")
    expect(patronus?.children?.map(item => item.id)).toEqual(["safe-push-with-profile", "dry-run", "check-approvals"])
    const copy = groupsOf("auth")
      .flat()
      .find(item => item.id === "copy")
    expect(copy?.children?.map(item => item.id)).toEqual(["copy-full-branch-name", "copy-branch-name"])
    // the keys of the row run what the menu lists, a submenu's item too
    for (const id of ["safe-push", "push", "review-changes", "move-to", "copy-full-branch-name", "dry-run"]) {
      expect(findMenuItem(groupsOf("auth"), id)?.id).toBe(id)
    }
    expect(findMenuItem(groupsOf("auth"), "patronus")).toBeUndefined()
  })

  it("starts with the push off Space, where Rust offers no start of Patronus", () => {
    setBranches(synced("auth", null, unpushed))
    offerOnly("auth", ...LANDING_BRANCH.filter(id => !["safe-push", "safe-push-with-profile", "dry-run", "check-approvals"].includes(id)))
    expect(idsOf("auth")[0]).toEqual(["push"])
    expect(menuOf("auth").some(item => item.id === "patronus" || item.id === "safe-push")).toBe(false)
  })

  it("offers Safe Push disabled until Patronus answers, and Create Pull Request… until GitHub lists the pull requests, as the review window does", () => {
    setBranches(synced("auth", null, unpushed))
    setNextSteps({ auth: pendingStep("patronus") })
    setBranchActions("auth", disabled("safe-push", { code: "step", blocker: { code: "step" } }), disabled("create-pull-request", { code: "pending", fact: "pullRequests" }))
    expect(itemOf("auth", "safe-push")).toMatchObject({ disabled: true, description: "Waits for the answer of Patronus" })
    expect(getBranchReviewItems(branchSyncStore.getState().branches["auth"]!).find(item => item.id === "safe-push")).toMatchObject({
      disabled: true,
      description: "Waits for the answer of Patronus",
    })
    expect(itemOf("auth", "create-pull-request")).toMatchObject({ disabled: true, description: "Waits for GitHub to list the pull requests" })
  })

  it("moves a branch with Move to… alone: the form offers the existing branches", () => {
    setBranches(synced("auth", null, unpushed), synced("docs", null, unpushed))
    expect(itemOf("auth", "move-to")).toMatchObject({ label: "Move to…", kbds: ["m"] })
    expect(menuOf("auth").some(item => item.id === "move-to-existing")).toBe(false)
  })
})
