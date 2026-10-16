// @spec spec/branch/branch-review.spec.md
import { afterEach, describe, expect, it } from "vite-plus/test"
import { branchPreviewStore, closeBranchPreview, openBranchReviewDialog } from "@/stores/branch-preview"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

function selectProject(path: string) {
  repositorySettingsStore.setState({ items: [{ path, cachedBranchPrefix: undefined }] })
}

describe("branch preview", () => {
  afterEach(() => {
    closeBranchPreview()
  })

  it("closes when another repository is selected, and doesn't come back with the first one", () => {
    selectProject("/first")
    openBranchReviewDialog("login", { range: "all" }, false)
    expect(branchPreviewStore.getState()).toMatchObject({ repositoryPath: "/first", name: "login", review: { focus: { range: "all" }, fromPreview: false } })
    selectProject("/second")
    expect(branchPreviewStore.getState()).toMatchObject({ name: null, review: null })
    selectProject("/first")
    expect(branchPreviewStore.getState().name).toBeNull()
  })
})
