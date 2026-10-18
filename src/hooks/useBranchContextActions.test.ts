// @spec spec/branch/branch-review.spec.md
// @spec spec/branch/stacked-branches.spec.md
// @spec spec/branch/branch-row.spec.md
import { afterEach, describe, expect, it } from "vite-plus/test"
import { getBranchContextMenuItems, getSafePushBlocker } from "@/hooks/useBranchContextActions"
import type { MenuItem } from "@/lib/menu"
import type { Branch, RemoteStatus, SyncedCommit } from "@/stores/branch-sync"
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

  it("pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row", () => {
    const dropping = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, unpushedCommits: ["c"] })
    setBranches(synced("auth", null, dropping), synced("login", "auth", unpushed))
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", label: "Create Remote (1)" })])
    expect(pushItems("auth")).toEqual([expect.objectContaining({ id: "push", label: "Force Push (1)" })])
  })

  it("is disabled with the reason while the stack push runs", () => {
    setBranches(synced("auth", null, unpushed), synced("login", "auth", unpushed))
    branchSyncStore.setState({ stackPushes: { login: "auth" } })
    expect(pushItems("login")).toEqual([expect.objectContaining({ id: "push", disabled: true, description: "The stack is being pushed" })])
  })
})
