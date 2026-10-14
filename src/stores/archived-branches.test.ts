import { beforeEach, describe, expect, it, vi } from "vitest"
import { archivedBranchesStore, clearNewlyArchived, updateFromArchivedNames, updateFromIntegrationInfo } from "@/stores/archived-branches"

vi.mock("@/lib/bindings", () => ({ commands: {} }))
vi.mock("@/stores/repository", async () => (await import("@/stores/test-store-mocks")).repositoryMock)

const newlyArchived = () => [...archivedBranchesStore.getState().newlyArchived].toSorted()

describe("archived branches", () => {
  beforeEach(() => {
    updateFromArchivedNames([], [])
  })

  it("marks the branches archived in this session until cleared", () => {
    updateFromArchivedNames(["user/archived/2026-10-14/a", "user/archived/2026-10-01/old"], ["user/archived/2026-10-14/a"])
    // a later sync archives another one; the earlier mark stays
    updateFromArchivedNames(["user/archived/2026-10-14/b", "user/archived/2026-10-14/a", "user/archived/2026-10-01/old"], ["user/archived/2026-10-14/b"])
    expect(newlyArchived()).toEqual(["user/archived/2026-10-14/a", "user/archived/2026-10-14/b"])

    // a deleted branch loses its mark
    updateFromArchivedNames(["user/archived/2026-10-14/b", "user/archived/2026-10-01/old"], [])
    expect(newlyArchived()).toEqual(["user/archived/2026-10-14/b"])

    clearNewlyArchived()
    expect(newlyArchived()).toEqual([])
  })

  it("keeps the commit that merged the branch", () => {
    const name = "user/archived/2026-10-14/a"
    updateFromArchivedNames([name], [name])
    const commit = { id: "0123456789abcdef", author: "Jane Doe", committer: "Safe Push Robot", time: 1_760_000_000 }
    updateFromIntegrationInfo({ name, summary: "", status: { kind: "integrated", integratedAt: commit.time, confidence: "High", commitCount: 2, commit } })
    expect(archivedBranchesStore.getState().branches[name]).toMatchObject({ type: "integrated", integratedAt: commit.time, integrationCommit: commit })

    updateFromIntegrationInfo({ name, summary: "", status: { kind: "integrated", integratedAt: null, confidence: "High", commitCount: 2, commit: null } })
    expect(archivedBranchesStore.getState().branches[name]?.integrationCommit).toBeUndefined()
  })
})
