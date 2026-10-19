// @spec spec/branch/branch-review.spec.md
// @spec spec/branch/stacked-branches.spec.md
// @spec spec/branch/branch-row.spec.md
import { afterEach, describe, expect, it } from "vite-plus/test"
import { getBranchContextMenuItems, getSafePushBlocker } from "@/hooks/useBranchContextActions"
import type { MenuItem } from "@/lib/menu"
import type { Branch, RemoteStatus, StackBranch, SyncedCommit } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { inlineRowActionStore } from "@/stores/inline-row-action"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

describe("getSafePushBlocker", () => {
  afterEach(() => {
    inlineRowActionStore.setState({ activeInline: null })
  })

  it("says why a branch can't start a Safe Push, in the words of every surface that offers it", () => {
    expect(getSafePushBlocker(testBranch("login", null))).toBeNull()
    expect(getSafePushBlocker(testBranch("login", null, { hasError: true }))).toBe("The branch failed to sync")
    expect(getSafePushBlocker(testBranch("login", null, { commitCount: 0 }))).toBe("No commits to push")
    inlineRowActionStore.setState({ activeInline: { type: "rename", branchName: "login", processing: true } })
    expect(getSafePushBlocker(testBranch("login", null))).toBe("Wait for the action on the branch to finish")
  })
})

const unpushed = testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, unpushedCommits: ["c"] })

/** A synced branch of one commit that the menu can change */
function synced(name: string, parent: string | null, remoteStatus: RemoteStatus, overrides: Partial<Branch> = {}): Branch {
  const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
  return testBranch(name, parent, { commits: [tip], remoteStatus, ...overrides })
}

function setBranches(...branches: Branch[]): void {
  branchSyncStore.setState({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder: branches.map(branch => branch.name), stacksEnabled: true })
}

function menuOf(name: string): MenuItem[] {
  return getBranchContextMenuItems(branchSyncStore.getState().branches[name]!, () => {}).flat()
}

describe("the items of the branch menu", () => {
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false })
  })

  it("says why an item doesn't apply in its description, never in its name, and names every item by a stable id", () => {
    setBranches(synced("auth", null, unpushed, { allCommitsHaveIssueReferences: true }), synced("login", "auth", unpushed))
    const docs = synced("docs", null, unpushed)
    const items = menuOf("auth")
    expect(items.filter(item => item.id === undefined).map(item => item.label)).toEqual([])
    expect(items.find(item => item.id === "add-issue-reference")).toMatchObject({
      label: "Add Issue Reference…",
      disabled: true,
      description: "Every commit of the branch has an issue reference",
    })
    // login is stacked on auth: no other branch is left for auth
    expect(items.find(item => item.id === "stack-on")).toMatchObject({ disabled: true, description: "No other branch to stack on" })
    setBranches(branchSyncStore.getState().branches.auth!, branchSyncStore.getState().branches.login!, docs)
    expect(menuOf("auth").find(item => item.id === "stack-on")).toMatchObject({ disabled: false, children: [{ label: "docs" }] })
  })
})

describe("Whole Stack › of the branch menu", () => {
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false })
  })

  it("is the submenu of the root's whole stack on every branch of the stack, none on a branch of no stack", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed), synced("oauth", "login", unpushed), synced("docs", null, unpushed))
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
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, isSyncing: false })
  })

  it("keeps every item: the rewrites wait for the sync, disabled with the reason; the rest stays as it is", () => {
    setBranches(synced("auth", null, unpushed), synced("docs", null, unpushed))
    const before = menuOf("auth").map(item => item.id)
    branchSyncStore.setState({ isSyncing: true })
    const items = menuOf("auth")
    expect(items.map(item => item.id)).toEqual(before)
    const rewrites = ["rename", "move-to", "stack-on", "add-issue-reference", "amend-changes", "unapply"]
    for (const id of rewrites) {
      expect(items.find(item => item.id === id)).toMatchObject({ disabled: true, description: "Wait for the sync to finish" })
    }
    // they rewrite nothing here: Review Changes; a push waits for the sync itself
    expect(items.find(item => item.id === "review-changes")).toMatchObject({ disabled: false })
    expect(items.find(item => item.id === "push")).toMatchObject({ disabled: false })
  })
})

describe("the push of the branch menu", () => {
  /** The items of the menu of `name` that push the branch, or its ancestors with it */
  function pushItems(name: string): MenuItem[] {
    return menuOf(name).filter(item => item.id === "push" || item.label.startsWith("Push Stack"))
  }

  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackPushes: {} })
  })

  it("is one item named as the push button, pushing the stack as far as the button does", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    expect(pushItems("login")).toEqual([
      expect.objectContaining({ id: "push", label: "Push Stack (2 branches)", description: "Pushes auth and login, parents first", disabled: false }),
    ])
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", label: "Create Remote (1)", disabled: false })])
  })

  it("offers the push of the branch alone after the push of the stack, as the description of the button does", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    const items = menuOf("login")
    const push = items.findIndex(item => item.id === "push")
    expect(items[push + 1]).toMatchObject({ id: "push-branch-only", label: "Push login Only", disabled: false })
    expect(items[push + 1]?.kbds).toBeUndefined()
    // a branch whose button pushes it alone has no second way in the menu
    expect(menuOf("auth").some(item => item.id === "push-branch-only")).toBe(false)
  })

  it("pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row", () => {
    const dropping = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, unpushedCommits: ["c"] })
    setBranches(synced("auth", null, dropping), synced("login", "auth", unpushed))
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", label: "Create Remote (1)" })])
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", label: "Force Push (1)" })])
  })

  it("is disabled with the reason while the stack push runs", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    branchSyncStore.setState({ stackPushes: { login: ["auth", "login"] } })
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", disabled: true, description: "The stack is being pushed" })])
  })
})

describe("Push with Children in the branch menu", () => {
  const pushed = testRemoteStatus({ remoteTip: "tip" })
  const dropping = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, unpushedCommits: ["c"], remoteTip: "theirs" })

  function pushWithChildren(name: string): MenuItem | undefined {
    return menuOf(name).find(item => item.id === "push-with-children")
  }

  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackPushes: {}, isSyncing: false, stackBranches: {} })
  })

  it("takes the stack branch of the whole stack of a root along once it is on the remote, counted and named, and says why while it doesn't", () => {
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
    branchSyncStore.setState({ stackBranches: { auth: stackBranch(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null })) } })
    // not on the remote and no pull request: the branches are, and "Nothing to push" would not be true
    expect(pushWithChildren("auth")).toMatchObject({
      label: "Push with Children",
      disabled: true,
      description: "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack",
    })
    branchSyncStore.setState({ stackBranches: { auth: stackBranch(testRemoteStatus({ commitsAhead: 1, unpushedCommits: ["c"], remoteTip: "old" })) } })
    expect(pushWithChildren("auth")).toMatchObject({
      label: "Push with Children (the whole stack branch)",
      disabled: false,
      description: "Also pushes the whole stack branch user/stack/auth",
    })
    setBranches(synced("auth", null, pushed), synced("login", "auth", unpushed), synced("session", "auth", pushed))
    expect(pushWithChildren("auth")).toMatchObject({ label: "Push with Children (2 branches)", disabled: false })
    // a branch in the middle heads no whole stack
    setBranches(synced("auth", null, pushed), synced("login", "auth", pushed), synced("oauth", "login", unpushed), synced("session", "auth", pushed))
    expect(pushWithChildren("login")).toMatchObject({ label: "Push with Children (oauth)" })
    expect(pushWithChildren("login")?.description).toBeUndefined()
    branchSyncStore.setState({ stackBranches: { auth: stackBranch(testRemoteStatus({ remoteTip: "tip" })) } })
    setBranches(synced("auth", null, pushed), synced("login", "auth", pushed), synced("session", "auth", pushed))
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "Nothing to push: the stack is on the remote" })
  })

  it("is offered beside the push on every branch with children, disabled with the reason while it would push nothing beyond the branch", () => {
    setBranches(synced("auth", null, pushed), synced("login", "auth", pushed), synced("docs", null, unpushed))
    // the remote group: the push of the branch, then the push with its children
    const items = menuOf("auth")
    expect(items[items.findIndex(item => item.id === "push") + 1]).toMatchObject({
      id: "push-with-children",
      label: "Push with Children",
      disabled: true,
      description: "Nothing to push: the stack is on the remote",
    })
    // a branch without children has nothing to push with them
    expect(menuOf("login").some(item => item.id === "push-with-children")).toBe(false)
    expect(menuOf("docs").some(item => item.id === "push-with-children")).toBe(false)

    setBranches(synced("auth", null, unpushed), synced("login", "auth", pushed))
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "Only auth has commits to push: its own push sends them" })
    setBranches(synced("auth", null, pushed), synced("login", "auth", dropping))
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "login asks first in its row: the push drops remote commits" })
    setBranches(synced("auth", null, pushed), synced("login", "auth", unpushed))
    expect(pushWithChildren("auth")).toMatchObject({ label: "Push with Children (login)", disabled: false })
    branchSyncStore.setState({ stackPushes: { auth: ["login"] } })
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "The stack is being pushed" })
  })

  it("is offered while a sync runs and while the root is on the remote: the push waits for the sync", () => {
    setBranches(synced("auth", null, pushed), synced("login", "auth", unpushed))
    branchSyncStore.setState({ isSyncing: true })
    expect(pushWithChildren("auth")).toMatchObject({ label: "Push with Children (login)", disabled: false })
    // the sync computes what is left to push
    setBranches(synced("auth", null, pushed), testBranch("login", "auth"))
    expect(pushWithChildren("auth")).toMatchObject({ disabled: true, description: "Wait for the sync to finish" })
  })
})
