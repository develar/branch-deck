// @spec spec/commit/commit-files.spec.md
import { beforeEach, describe, expect, it, vi } from "vitest"
import type { CommitFiles } from "@/lib/bindings"
import { commitFilesStore, getFilesOfCommit, loadCommitFiles, loadFilesOfCommit } from "@/stores/commit-files"

const { calls } = vi.hoisted(() => ({
  calls: {
    batches: [] as string[][],
    singles: [] as string[],
    fail: false,
    /** Resolves the batch in flight; null answers at once */
    release: null as (() => void) | null,
    hold: false,
  },
}))

function files(commitId: string): CommitFiles {
  return { commitId, files: [{ path: `${commitId}.txt`, status: "added", oldPath: null, additions: 1, deletions: 0, binary: false }], additions: 1, deletions: 0 }
}

vi.mock("@/lib/bindings", () => ({
  commands: {
    getCommitsFiles: async ({ commitIds }: { commitIds: string[] }) => {
      calls.batches.push(commitIds)
      if (calls.hold) {
        await new Promise<void>((resolve) => {
          calls.release = resolve
        })
      }
      return calls.fail ? { status: "error", error: { message: "boom" } } : { status: "ok", data: commitIds.map(files) }
    },
    getCommitFiles: async ({ commitId }: { commitId: string }) => {
      calls.singles.push(commitId)
      return { status: "ok", data: files(commitId) }
    },
  },
}))

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/errors", () => ({ logError: () => {} }))

beforeEach(() => {
  commitFilesStore.setState({ byCommit: new Map(), loading: new Set() })
  calls.batches = []
  calls.singles = []
  calls.fail = false
  calls.hold = false
  calls.release = null
})

describe("loadCommitFiles", () => {
  it("loads the commits of a list with one call, and only the ones not known yet", async () => {
    await loadCommitFiles(["a", "b", "a"])
    expect(calls.batches).toEqual([["a", "b"]])
    expect(getFilesOfCommit("b")).toEqual(files("b"))

    await loadCommitFiles(["b", "c"])
    expect(calls.batches).toEqual([["a", "b"], ["c"]])
    await loadCommitFiles(["a", "c"])
    expect(calls.batches).toHaveLength(2)
  })

  it("waits for a load in flight instead of asking again", async () => {
    calls.hold = true
    const first = loadCommitFiles(["a", "b"])
    const second = loadCommitFiles(["b"])
    const single = loadFilesOfCommit("a")
    // the lists of these commits are busy meanwhile
    expect([...commitFilesStore.getState().loading]).toEqual(["a", "b"])
    calls.release?.()
    await Promise.all([first, second, single])
    expect(calls.batches).toEqual([["a", "b"]])
    expect(calls.singles).toEqual([])
    expect(getFilesOfCommit("a")).toEqual(files("a"))
    expect(commitFilesStore.getState().loading.size).toBe(0)
  })

  it("tries again after a failed load", async () => {
    calls.fail = true
    await loadCommitFiles(["a"])
    expect(getFilesOfCommit("a")).toBeUndefined()
    expect(commitFilesStore.getState().loading.size).toBe(0)
    calls.fail = false
    await loadCommitFiles(["a"])
    expect(calls.batches).toEqual([["a"], ["a"]])
    expect(getFilesOfCommit("a")).toEqual(files("a"))
  })
})

describe("loadFilesOfCommit", () => {
  it("loads the one commit a preview shows with the single command", async () => {
    await loadFilesOfCommit("a")
    expect(calls.singles).toEqual(["a"])
    expect(calls.batches).toEqual([])
    expect(getFilesOfCommit("a")).toEqual(files("a"))
  })
})
