// @spec spec/branch/branch-review.spec.md
// @spec spec/branch/stacked-branches.spec.md
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

describe("the push of the branch menu", () => {
  const unpushed = testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, unpushedCommits: ["c"] })

  function synced(name: string, parent: string | null, remoteStatus: RemoteStatus): Branch {
    const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
    return testBranch(name, parent, { commits: [tip], remoteStatus })
  }

  function setBranches(...branches: Branch[]): void {
    branchSyncStore.setState({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder: branches.map(branch => branch.name), stacksEnabled: true })
  }

  /** The items of the menu of `name` that push the branch, or its ancestors with it */
  function pushItems(name: string): MenuItem[] {
    return getBranchContextMenuItems(branchSyncStore.getState().branches[name]!, () => {})
      .flat()
      .filter(item => item.id === "push" || item.label.startsWith("Push Stack"))
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
