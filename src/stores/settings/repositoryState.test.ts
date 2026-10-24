// @spec spec/repository/settings.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { SettingsPatch } from "@/lib/bindings"
import { getRepositoryState, repositoryStateStore, updateRepositoryState } from "@/stores/settings/repositoryState"

const { patches, writer } = vi.hoisted(() => {
  // the main window starts with repositories the writer moved out of the recent projects, two of them malformed
  const repositoryState: Record<string, unknown> = {
    "/a": { lastSyncTime: 10, lastBranchCount: 2, updateTimings: { sync: [5] } },
    "/b": { lastSyncTime: "yesterday", updateTimings: { sync: "fast" } },
    "/c": "broken",
  }
  vi.stubGlobal("window", { __TAURI_STORE__: { repositoryState: { ...repositoryState } }, __TAURI_STORE_VERSION__: 1 })
  // the writer's copy, which each patch changes as the writer applies it
  return { patches: [] as SettingsPatch[], writer: { repositoryState, version: 1 } }
})

vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }))
vi.mock("@/lib/window-label", () => ({ windowLabel: "main", isMainWindow: true }))
vi.mock("@/lib/errors", () => ({ logError: () => {}, notifyError: () => {} }))
vi.mock("@/lib/bindings", () => ({
  commands: {
    getSettingsChanges: async () => ({ status: "ok", data: [] }),
    setSetting: async ({ patch }: { patch: SettingsPatch }) => {
      patches.push(patch)
      if (patch.kind === "fields") {
        Object.assign(writer.repositoryState, patch.fields)
      }
      writer.version += 1
      return { status: "ok", data: { version: writer.version, key: patch.key, value: { ...writer.repositoryState }, origin: "main" } }
    },
  },
}))

describe("repositoryState", () => {
  it("drops a malformed field or repository, never the others", () => {
    expect(getRepositoryState("/a")).toEqual({ lastSyncTime: 10, lastBranchCount: 2, updateTimings: { sync: [5] } })
    expect(getRepositoryState("/b")).toEqual({})
    expect(getRepositoryState("/c")).toEqual({})
    expect(getRepositoryState("/never-synced")).toBeUndefined()
  })

  it("sends the writer only the repository a sync changed, as one field of repositoryState", async () => {
    updateRepositoryState("/a", state => ({ ...state, lastSyncTime: 20, lastBranchCount: 3 }))
    // the next one waits for the answer of the first
    updateRepositoryState("/new", state => ({ ...state, lastSyncTime: 30 }))

    await vi.waitFor(() => expect(patches).toHaveLength(2))
    expect(patches).toEqual([
      { kind: "fields", key: "repositoryState", fields: { "/a": { lastSyncTime: 20, lastBranchCount: 3, updateTimings: { sync: [5] } } } },
      { kind: "fields", key: "repositoryState", fields: { "/new": { lastSyncTime: 30 } } },
    ])
    await vi.waitFor(() => expect(writer.version).toBe(3))
    expect(getRepositoryState("/a")?.lastSyncTime).toBe(20)
    expect(getRepositoryState("/new")).toEqual({ lastSyncTime: 30 })
    expect(Object.keys(repositoryStateStore.getState()).toSorted()).toEqual(["/a", "/b", "/c", "/new"])
  })
})
