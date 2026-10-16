// @spec spec/commit/commit-files.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { CommitPatch } from "@/lib/bindings"
import { commitPatchStore, dropPatchesOf, loadCommitPatch, loadSubjectPatch, MAX_CACHED_PATCHES } from "@/stores/commit-patch"

const getCommitPatch = vi.hoisted(() => vi.fn())
const getRangePatch = vi.hoisted(() => vi.fn())

vi.mock("@/lib/bindings", () => ({ commands: { getCommitPatch, getRangePatch } }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/command", () => import("@/test-support/invoke-command-mock"))
vi.mock("@/lib/errors", () => ({ logError: () => {} }))

function patch(commitId: string): CommitPatch {
  return { commitId, files: [{ patch: `diff --git a/${commitId} b/${commitId}\n`, omittedLines: null }] }
}

const answer = async ({ commitId }: { commitId: string }) => ({ status: "ok", data: patch(commitId) })

beforeEach(() => {
  commitPatchStore.setState({ byCommit: new Map() })
  getCommitPatch.mockReset()
  getCommitPatch.mockImplementation(answer)
  getRangePatch.mockReset()
  getRangePatch.mockImplementation(async ({ base, head }: { base: string; head: string }) => ({ status: "ok", data: patch(`${base}..${head}`) }))
})

function requestedIds(): string[] {
  return getCommitPatch.mock.calls.map(([params]) => (params as { commitId: string }).commitId)
}

async function settle() {
  await vi.waitFor(() => {
    expect([...commitPatchStore.getState().byCommit.values()].some(entry => entry.kind === "loading")).toBe(false)
  })
}

describe("loadCommitPatch", () => {
  it("reads a commit once, also when asked again while it loads", async () => {
    let release = () => {}
    getCommitPatch.mockImplementationOnce(
      params =>
        new Promise(resolve => {
          release = () => resolve(answer(params))
        }),
    )
    loadCommitPatch("a")
    loadCommitPatch("a")
    expect(commitPatchStore.getState().byCommit.get("a")).toEqual({ kind: "loading" })
    release()
    await settle()
    loadCommitPatch("a")
    expect(requestedIds()).toEqual(["a"])
    expect(commitPatchStore.getState().byCommit.get("a")).toEqual({ kind: "loaded", patch: patch("a") })
  })

  it("keeps a failure as an error and tries again on the next load", async () => {
    getCommitPatch.mockResolvedValueOnce({ status: "error", error: { message: "boom" } })
    loadCommitPatch("a")
    await settle()
    expect(commitPatchStore.getState().byCommit.get("a")).toEqual({ kind: "error", message: "boom" })
    loadCommitPatch("a")
    await settle()
    expect(requestedIds()).toEqual(["a", "a"])
    expect(commitPatchStore.getState().byCommit.get("a")?.kind).toBe("loaded")
  })

  it("evicts the least recently loaded patch over the limit", async () => {
    for (let i = 0; i < MAX_CACHED_PATCHES; i++) {
      loadCommitPatch(`c${i}`)
      await settle()
    }
    // asked again, so the newest
    loadCommitPatch("c0")
    loadCommitPatch("new")
    await settle()
    const ids = [...commitPatchStore.getState().byCommit.keys()]
    expect(ids).toHaveLength(MAX_CACHED_PATCHES)
    expect(ids).not.toContain("c1")
    expect(ids.slice(-2)).toEqual(["c0", "new"])
  })
})

describe("loadSubjectPatch", () => {
  it("reads the range of a review with its own command, keyed base..head, until its review drops it", async () => {
    loadSubjectPatch({ kind: "range", base: "b", head: "h" })
    await settle()
    expect(getRangePatch).toHaveBeenCalledTimes(1)
    expect(getCommitPatch).not.toHaveBeenCalled()
    expect(commitPatchStore.getState().byCommit.get("b..h")).toEqual({ kind: "loaded", patch: patch("b..h") })
    dropPatchesOf(["b..h"])
    expect(commitPatchStore.getState().byCommit.has("b..h")).toBe(false)
  })
})
