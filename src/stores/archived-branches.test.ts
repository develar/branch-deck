import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { archivedBranchesStore, clearNewlyArchived, settleArchivedPlaceholders, updateFromArchivedNames, updateFromIntegrationInfo, updateFromIntegrationInfos } from "@/stores/archived-branches"

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

  it("marks the placeholders a finished sync left without a result as unknown", () => {
    const detected = "user/archived/2026-10-14/a"
    const failed = "user/archived/2026-10-14/b"
    updateFromArchivedNames([detected, failed], [])
    updateFromIntegrationInfo({ name: detected, summary: "", status: { kind: "notIntegrated", totalCommitCount: 2, integratedCount: 0, orphanedCount: 2, integratedAt: null } })

    settleArchivedPlaceholders()
    const { branches } = archivedBranchesStore.getState()
    expect(branches[detected]?.type).toBe("not-integrated")
    expect(branches[failed]?.type).toBe("unknown")

    // a later result still lands
    updateFromIntegrationInfo({ name: failed, summary: "", status: { kind: "partial", missing: 1 } })
    expect(archivedBranchesStore.getState().branches[failed]?.type).toBe("not-integrated")
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

  it("keeps the records and the order when a sync reports the same integration again", () => {
    const names = ["user/archived/2026-10-13/a", "user/archived/2026-10-14/b"]
    updateFromArchivedNames(names, [])
    const infos = names.map(name => ({ name, summary: "s", status: { kind: "integrated" as const, integratedAt: 10, confidence: "High" as const, commitCount: 1, commit: { id: name, author: "a", committer: "c", time: 10 } } }))
    updateFromIntegrationInfos(infos)
    const before = archivedBranchesStore.getState()
    expect(before.order).toEqual(names)

    updateFromIntegrationInfos(infos.map(info => ({ ...info, status: { ...info.status, commit: { ...info.status.commit } } })))
    expect(archivedBranchesStore.getState()).toBe(before)
  })
})
