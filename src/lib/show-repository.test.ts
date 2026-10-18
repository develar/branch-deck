// @spec spec/branch/branch-review.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { showRepository } from "@/lib/show-repository"
import { branchSyncStore } from "@/stores/branch-sync"

const { repository } = vi.hoisted(() => ({
  /** The repository the main window shows, the recent ones, and what it was asked to select */
  repository: { selected: "/app" as string, recent: ["/app", "/other"], selections: [] as string[] },
}))

vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: repository.selected }),
  selectProject: (project: { path: string }) => {
    repository.selections.push(project.path)
    repository.selected = project.path
  },
}))
vi.mock("@/stores/settings/repositorySettings", () => ({
  getRecentProjects: () => repository.recent.map(path => ({ path })),
}))

describe("showRepository", () => {
  beforeEach(() => {
    repository.selected = "/app"
    repository.selections = []
    branchSyncStore.setState({ syncedRepositoryPath: "/app", isSyncing: false })
  })

  it("does nothing for the repository the main window shows and synced", async () => {
    expect(await showRepository("/app")).toBeNull()
    expect(repository.selections).toEqual([])
  })

  it("switches the main window to another repository and waits for its first sync", async () => {
    const shown = showRepository("/other")
    expect(repository.selections).toEqual(["/other"])
    // the sync of the switch: the old branches are forgotten, the new ones synced
    branchSyncStore.setState({ syncedRepositoryPath: null, isSyncing: true })
    branchSyncStore.setState({ syncedRepositoryPath: "/other", isSyncing: false })
    expect(await shown).toBeNull()
  })

  it("says why a repository that is no recent one any more can't be shown", async () => {
    expect(await showRepository("/gone")).toBe("gone is no longer among the repositories of the main window")
    expect(repository.selections).toEqual([])
  })
})
