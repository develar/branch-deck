// @spec spec/repository/settings.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { ResolvedBranchPrefix } from "@/lib/bindings"
import { getEffectiveBranchPrefix, initRepository, repositoryStore } from "@/stores/repository"

const { resolves, projects, writerListeners, updates } = vi.hoisted(() => ({
  /** The `resolve_branch_prefix` calls, each answered when the test answers it */
  resolves: [] as { repositoryPath: string; answer: (prefix: ResolvedBranchPrefix) => Promise<void> }[],
  /** The recent projects: the selected one first, with the prefix the last resolve of an earlier run cached */
  projects: [{ path: "/repo", cachedBranchPrefix: { prefix: "cached", source: "app_setting" } as ResolvedBranchPrefix | undefined }],
  writerListeners: new Map<string, (value: unknown) => void>(),
  updates: [] as { path: string; update: unknown }[],
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    resolveBranchPrefix: ({ repositoryPath }: { repositoryPath: string }) =>
      new Promise(resolve => {
        resolves.push({
          repositoryPath,
          answer: async prefix => {
            resolve({ status: "ok", data: prefix })
            await Promise.resolve()
            await Promise.resolve()
          },
        })
      }),
    checkGitSetup: async () => ({ status: "ok", data: { git_error: null, identity_error: null, suggested_branch_prefix: "" } }),
  },
}))

vi.mock("@/stores/settings/persistent-store", () => ({
  // the app's prefix in the snapshot the window started from
  snapshotValueOf: () => ({ globalUserBranchPrefix: "jane" }),
  onWriterChange: (key: string, listener: (value: unknown) => void) => {
    writerListeners.set(key, listener)
    return () => writerListeners.delete(key)
  },
}))

vi.mock("@/stores/settings/repositorySettings", () => ({
  getRecentProjects: () => projects,
  setRecentProjects: () => {},
  updateProject: (path: string, update: unknown) => updates.push({ path, update }),
  useRecentProjects: () => undefined,
  repositorySettingsStore: { subscribe: () => () => {} },
}))

function lastResolve() {
  const call = resolves.at(-1)
  if (call === undefined) {
    throw new Error("nothing was resolved")
  }
  return call
}

describe("the branch prefix of the selected repository", () => {
  it("names the branches by the cached prefix until Rust answers, then by the answer, which it caches", async () => {
    initRepository()
    expect(repositoryStore.getState().branchPrefix).toBeNull()
    expect(getEffectiveBranchPrefix()).toBe("cached")

    const own: ResolvedBranchPrefix = { prefix: "own", source: "repository_config" }
    expect(lastResolve().repositoryPath).toBe("/repo")
    await lastResolve().answer(own)
    expect(repositoryStore.getState().branchPrefix).toEqual(own)
    expect(getEffectiveBranchPrefix()).toBe("own")
    expect(updates).toEqual([{ path: "/repo", update: { cachedBranchPrefix: own } }])
  })

  it("resolves again once the writer has another app prefix, not for a change of another field", () => {
    initRepository()
    const asked = resolves.length
    const appSettings = writerListeners.get("appSettings")!
    appSettings({ globalUserBranchPrefix: "jane", zoom: 1.2 })
    expect(resolves).toHaveLength(asked)
    appSettings({ globalUserBranchPrefix: "john" })
    expect(resolves).toHaveLength(asked + 1)
    // the shown prefix stays until the answer: no frame without one
    expect(repositoryStore.getState().branchPrefix).not.toBeNull()
    appSettings({})
    expect(resolves).toHaveLength(asked + 2)
  })
})
