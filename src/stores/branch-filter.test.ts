import { beforeEach, describe, expect, it, vi } from "vitest"
import { branchFilterStore, clearBranchFilter, getBranchFilter, getShownBranches, rememberShown, setFilterText, toggleFilterChip } from "@/stores/branch-filter"

const selected = vi.hoisted(() => ({ path: "/repo/a" }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selected.path }),
  useSelectedProject: () => ({ path: selected.path }),
}))

describe("branch filter store", () => {
  beforeEach(() => {
    selected.path = "/repo/a"
    branchFilterStore.setState({ repositoryPath: null, filter: { text: "", chip: "all" }, shown: null })
  })

  it("chooses one chip at a time, and the chosen one again goes back to All", () => {
    toggleFilterChip("toPush")
    expect(getBranchFilter().chip).toBe("toPush")
    toggleFilterChip("running")
    expect(getBranchFilter().chip).toBe("running")
    toggleFilterChip("running")
    expect(getBranchFilter().chip).toBe("all")
  })

  it("keeps the text with the chip and clears both", () => {
    setFilterText("auth")
    toggleFilterChip("attention")
    expect(getBranchFilter()).toEqual({ text: "auth", chip: "attention" })
    clearBranchFilter()
    expect(getBranchFilter()).toEqual({ text: "", chip: "all" })
  })

  it("remembers the shown branches until the filter changes", () => {
    rememberShown(["auth"])
    // nothing to remember while the filter shows all
    expect(getShownBranches()).toBeNull()

    toggleFilterChip("toPush")
    rememberShown(["auth", "api"])
    rememberShown(["api"])
    expect([...getShownBranches()!]).toEqual(["auth", "api"])
    setFilterText("a")
    expect(getShownBranches()).toBeNull()
  })

  it("has no filter in another repository: a repository change starts without one", () => {
    setFilterText("auth")
    rememberShown(["auth"])
    selected.path = "/repo/b"
    expect(getBranchFilter()).toEqual({ text: "", chip: "all" })
    expect(getShownBranches()).toBeNull()
    // the first change there starts from no filter
    toggleFilterChip("running")
    expect(getBranchFilter()).toEqual({ text: "", chip: "running" })
  })
})
