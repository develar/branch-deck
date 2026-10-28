// @spec spec/branch/branch-preview.spec.md
import { afterEach, describe, expect, it } from "vite-plus/test"
import { branchPreviewStore, closeBranchPreview, openBranchPreview } from "@/stores/branch-preview"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

function selectProject(path: string) {
  repositorySettingsStore.setState({ items: [{ path }] })
}

describe("branch preview", () => {
  afterEach(() => {
    closeBranchPreview()
  })

  it("closes when another repository is selected, and doesn't come back with the first one", () => {
    selectProject("/first")
    openBranchPreview("login")
    expect(branchPreviewStore.getState()).toMatchObject({ repositoryPath: "/first", name: "login" })
    selectProject("/second")
    expect(branchPreviewStore.getState().name).toBeNull()
    selectProject("/first")
    expect(branchPreviewStore.getState().name).toBeNull()
  })
})
