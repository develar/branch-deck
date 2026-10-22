// @spec spec/branch/move-commits.spec.md
import { beforeEach, describe, expect, it } from "vite-plus/test"
import type { Commit } from "@/lib/bindings"
import {
  BRANCH_TABLE_SOURCE,
  claimSelection,
  clearBranchSelection,
  commitMoveStore,
  isDroppableOn,
  requestCommitForm,
  requestReveal,
  selectBranches,
  setBranchTargetOpen,
} from "@/stores/commit-move"
import { mainViewStore, showMainView } from "@/stores/main-view"

const order = ["auth", "IJPL-1", "IJPL-2", "docs"]

describe("commit move store", () => {
  beforeEach(() => {
    commitMoveStore.setState({ selectionOwner: null, selectedBranches: [], branchAnchor: null, isBranchTargetOpen: false, drag: null })
  })

  it("selects branches with a click, Cmd-click and Shift-click, and takes the selection from the commit lists", () => {
    claimSelection("auth")
    selectBranches("IJPL-1", "only", order)
    expect(commitMoveStore.getState().selectionOwner).toBe(BRANCH_TABLE_SOURCE)
    selectBranches("docs", "range", order)
    expect(commitMoveStore.getState().selectedBranches).toEqual(["IJPL-1", "IJPL-2", "docs"])
    selectBranches("IJPL-2", "toggle", order)
    expect(commitMoveStore.getState().selectedBranches).toEqual(["IJPL-1", "docs"])
  })

  it("clears the selected branches and their form when a commit list takes the selection", () => {
    selectBranches("IJPL-1", "only", order)
    setBranchTargetOpen(true)
    expect(commitMoveStore.getState().isBranchTargetOpen).toBe(true)
    claimSelection("auth")
    expect(commitMoveStore.getState()).toMatchObject({ selectedBranches: [], isBranchTargetOpen: false })
  })

  it("opens the form only for a selection", () => {
    setBranchTargetOpen(true)
    expect(commitMoveStore.getState().isBranchTargetOpen).toBe(false)
    selectBranches("auth", "only", order)
    setBranchTargetOpen(true)
    clearBranchSelection()
    expect(commitMoveStore.getState().isBranchTargetOpen).toBe(false)
  })

  it("shows the branches for a reveal and a form of commits, and keeps the view when a request is consumed", () => {
    showMainView("landing")
    requestReveal(null)
    expect(mainViewStore.getState().view).toBe("landing")
    requestReveal({ branchName: "auth", commitIds: [] })
    expect(mainViewStore.getState().view).toBe("branches")
    showMainView("landing")
    requestCommitForm({ branchName: "auth", commitIds: ["a"], form: "move" })
    expect(mainViewStore.getState().view).toBe("branches")
  })

  it("drops anywhere but on the one list a drag comes from", () => {
    const commits: Commit[] = []
    expect(isDroppableOn({ sourceKeys: ["auth"], branches: [], commits }, "auth")).toBe(false)
    expect(isDroppableOn({ sourceKeys: ["auth"], branches: [], commits }, "docs")).toBe(true)
    // a drag of two branches merges the other one into either
    expect(isDroppableOn({ sourceKeys: ["IJPL-1", "IJPL-2"], branches: ["IJPL-1", "IJPL-2"], commits }, "IJPL-1")).toBe(true)
  })
})
