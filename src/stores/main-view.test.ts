// @spec openspec/specs/repository/main-views/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { commitMoveStore, REVEAL_BRANCH_EVENT, requestReveal } from "@/stores/commit-move"
import { isInAnotherTab, mainViewStore, showMainView, showMainViewNow, tabView } from "@/stores/main-view"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn((_cmd: string, _args?: unknown) => Promise.resolve(null)) }))

vi.mock("@tauri-apps/api/core", () => ({ invoke }))
// the page of the tab of Reviews of the IDE, whose tool window shows each view in a tab of its own
vi.mock("@/lib/title-bar", () => ({ hasViewTabs: true }))
vi.mock("@/lib/window-label", () => ({ windowLabel: "reviews", isMainWindow: false, REVIEW_WINDOW: "review" }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

describe("the views of the main window as tabs of the IDE", () => {
  beforeEach(() => {
    invoke.mockClear()
    mainViewStore.setState({ view: "branches" })
    commitMoveStore.setState({ reveal: null })
  })

  it("asks brd for the tab of a view of another tab, with the event for its page, and shows its own view itself", () => {
    expect(tabView).toBe("reviews")
    expect([isInAnotherTab("branches"), isInAnotherTab("reviews"), isInAnotherTab("landing")]).toEqual([true, false, true])

    showMainView("landing")
    showMainViewNow("branches")
    showMainView("branches", { event: "focus-branch", payload: "auth" })
    expect(invoke.mock.calls).toEqual([
      ["plugin:air|show_view", { view: "landing" }],
      ["plugin:air|show_view", { view: "main" }],
      ["plugin:air|show_view", { view: "main", event: "focus-branch", payload: "auth" }],
    ])
    // no view of another tab shows in this page
    expect(mainViewStore.getState().view).toBe("branches")

    invoke.mockClear()
    showMainView("reviews")
    expect(invoke).not.toHaveBeenCalled()
    expect(mainViewStore.getState().view).toBe("reviews")
  })

  it("reveals a branch in the tab of the branches, whose page reveals it, and asks no reveal of this page", () => {
    requestReveal({ branchName: "auth", commitIds: ["c1"] })
    expect(invoke.mock.calls).toEqual([["plugin:air|show_view", { view: "main", event: REVEAL_BRANCH_EVENT, payload: { repositoryPath: "/repo", branchName: "auth" } }]])
    expect(commitMoveStore.getState().reveal).toBeNull()
  })
})
