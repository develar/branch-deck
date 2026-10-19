// @spec spec/commit/commit-files.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { DiffFileHeader, FileDiffEvent, FileMergeEvent, FileMergeHeader, GetFileDiffsParams, GetFileMergesParams, MergeContent } from "@/lib/bindings"
import type { DiffContent } from "@/lib/diff/file-diff-model"
import type { MergeSource } from "@/lib/diff/file-merge-model"
import { getFoldLevels, setFoldLevels } from "@/lib/diff/rows"
import { dropFileDiffsOf, fileDiffOf, fileDiffsKey, fileDiffsStore, loadFileDiffs, MAX_CACHED_SUBJECTS } from "@/stores/file-diffs"
import { fileMergeOf, fileMergesKey, fileMergesStore, loadFileMerges } from "@/stores/file-merges"
import { block, mergeBlock, textDiff, textMerge } from "@/test-support/diff-blocks"

const getFileDiffs = vi.hoisted(() => vi.fn())
const getFileMerges = vi.hoisted(() => vi.fn())

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", () => ({ commands: { getFileDiffs, getFileMerges } }))
vi.mock("@/lib/errors", () => ({ logError: () => {} }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/command", () => import("@/test-support/invoke-command-mock"))

type Events = { onmessage: (event: FileDiffEvent) => void }

function header(path: string): DiffFileHeader {
  return { path, oldPath: null, status: "modified", oldId: "1", newId: "2" }
}

const BINARY: DiffContent = { kind: "binary" }

/** The answer of a subject: its one file, binary, as the backend streams it */
async function answer(params: GetFileDiffsParams, events: Events) {
  const path = params.source.kind === "commit" ? params.source.id : params.source.kind
  events.onmessage({ kind: "files", files: [header(path)] })
  events.onmessage({ kind: "content", index: 0, content: BINARY })
  return { status: "ok", data: null }
}

beforeEach(() => {
  fileDiffsStore.setState({ byKey: new Map() })
  getFileDiffs.mockReset()
  getFileDiffs.mockImplementation(answer)
})

const entryOf = (subject: Parameters<typeof fileDiffsKey>[0], policy: Parameters<typeof fileDiffsKey>[1] = "default") =>
  fileDiffsStore.getState().byKey.get(fileDiffsKey(subject, policy))

async function settle() {
  await vi.waitFor(() => {
    expect([...fileDiffsStore.getState().byKey.values()].every(entry => entry.kind !== "loading" && (entry.kind !== "loaded" || entry.complete))).toBe(true)
  })
}

describe("loadFileDiffs", () => {
  it("reads a subject once per policy, the files in view first", async () => {
    loadFileDiffs({ kind: "commit", id: "c1" }, "default", { priorityPaths: ["src/a.kt"] })
    loadFileDiffs({ kind: "commit", id: "c1" }, "default")
    await settle()
    loadFileDiffs({ kind: "commit", id: "c1" }, "default")
    loadFileDiffs({ kind: "commit", id: "c1" }, "ignore")
    await settle()
    expect(getFileDiffs.mock.calls.map(([params]) => params as GetFileDiffsParams)).toEqual([
      { repositoryPath: "/repo", source: { kind: "commit", id: "c1" }, policy: "default", priorityPaths: ["src/a.kt"] },
      { repositoryPath: "/repo", source: { kind: "commit", id: "c1" }, policy: "ignore", priorityPaths: [] },
    ])
    expect(entryOf({ kind: "commit", id: "c1" }, "ignore")).toEqual({ kind: "loaded", files: [{ ...header("c1"), content: BINARY }], complete: true })
  })

  it("shows each file as its content arrives, and the late scopes of a text keep the lines it expanded", async () => {
    let events: Events | undefined
    let end = () => {}
    getFileDiffs.mockImplementationOnce((_params: GetFileDiffsParams, channel: Events) => {
      events = channel
      return new Promise(resolve => (end = () => resolve({ status: "ok", data: null })))
    })
    const subject = { kind: "commit", id: "c2" } as const
    loadFileDiffs(subject, "default")
    expect(entryOf(subject)?.kind).toBe("loading")
    events!.onmessage({ kind: "files", files: [header("a.kt"), header("b.kt")] })
    expect([entryOf(subject), fileDiffOf(entryOf(subject), "a.kt")]).toEqual([{ kind: "loaded", files: [], complete: false }, "pending"])

    const text: DiffContent = { kind: "text", ...textDiff(10, 10, [block(5, 6, 5, 6)]) }
    events!.onmessage({ kind: "content", index: 1, content: text })
    await vi.waitFor(() => expect(fileDiffOf(entryOf(subject), "b.kt")).toMatchObject({ path: "b.kt", content: text }))
    setFoldLevels(text, [3])
    const scopes = [{ startLine: 0, endLine: 10, label: "class B" }]
    events!.onmessage({ kind: "scopes", index: 1, oldScopes: [], newScopes: scopes })
    end()
    await settle()
    const labelled = fileDiffOf(entryOf(subject), "b.kt")
    expect(labelled !== "pending" && labelled?.content.kind === "text" ? [labelled.content.newScopes, getFoldLevels(labelled.content)] : null).toEqual([scopes, [3]])
    // the stream ended without the content of a.kt: the engine has none for it
    expect(fileDiffOf(entryOf(subject), "a.kt")).toBeUndefined()
  })

  it("reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends", async () => {
    loadFileDiffs({ kind: "uncommitted" }, "default")
    await settle()
    let release = () => {}
    getFileDiffs.mockImplementationOnce(
      (_params: GetFileDiffsParams, events: Events) =>
        new Promise(resolve => {
          events.onmessage({ kind: "files", files: [header("next")] })
          release = () => {
            events.onmessage({ kind: "content", index: 0, content: BINARY })
            resolve({ status: "ok", data: null })
          }
        }),
    )
    loadFileDiffs({ kind: "uncommitted" }, "default", { refresh: true })
    expect(entryOf({ kind: "uncommitted" })).toEqual({ kind: "loaded", files: [{ ...header("uncommitted"), content: BINARY }], complete: true })
    release()
    await vi.waitFor(() => expect(entryOf({ kind: "uncommitted" })).toEqual({ kind: "loaded", files: [{ ...header("next"), content: BINARY }], complete: true }))
  })

  it("says why a first read failed", async () => {
    getFileDiffs.mockImplementationOnce(async () => ({ status: "error", error: { message: "no such commit" } }))
    loadFileDiffs({ kind: "commit", id: "bad" }, "default")
    await vi.waitFor(() => expect(entryOf({ kind: "commit", id: "bad" })).toEqual({ kind: "error", message: "no such commit" }))
  })

  it("keeps the last subjects only, and forgets the subjects of a closed review under every policy", async () => {
    for (let index = 0; index <= MAX_CACHED_SUBJECTS; index++) {
      loadFileDiffs({ kind: "commit", id: `c${index}` }, "default")
    }
    await settle()
    expect(fileDiffsStore.getState().byKey.size).toBe(MAX_CACHED_SUBJECTS)
    expect(fileDiffsStore.getState().byKey.has(fileDiffsKey({ kind: "commit", id: "c0" }, "default"))).toBe(false)
    loadFileDiffs({ kind: "range", base: "a", head: "b" }, "trim")
    await settle()
    dropFileDiffsOf(["a..b"])
    expect([...fileDiffsStore.getState().byKey.keys()].some(key => key.endsWith("a..b"))).toBe(false)
  })
})

type MergeEvents = { onmessage: (event: FileMergeEvent) => void }

const SOURCE: MergeSource = { kind: "cherryPick", parent: "p", target: "t", commit: "c" }

function mergeHeader(path: string): FileMergeHeader {
  return { path, kind: "content", baseId: "1", leftId: "2", rightId: "3" }
}

const mergeEntryOf = (policy: Parameters<typeof fileMergesKey>[2] = "default") => fileMergesStore.getState().byKey.get(fileMergesKey("/repo", SOURCE, policy))

beforeEach(() => {
  fileMergesStore.setState({ byKey: new Map() })
  getFileMerges.mockReset()
})

describe("loadFileMerges", () => {
  it("reads the merges of a conflict once per policy, the open file first, and the late scopes keep the lines it expanded", async () => {
    let events: MergeEvents | undefined
    let end = () => {}
    getFileMerges.mockImplementation((_params: GetFileMergesParams, channel: MergeEvents) => {
      events = channel
      return new Promise(resolve => (end = () => resolve({ status: "ok", data: null })))
    })
    loadFileMerges(SOURCE, "default", ["a.kt"])
    loadFileMerges(SOURCE, "default")
    expect(getFileMerges.mock.calls.map(([params]) => params as GetFileMergesParams)).toEqual([
      { repositoryPath: "/repo", source: SOURCE, policy: "default", priorityPaths: ["a.kt"] },
    ])
    events!.onmessage({ kind: "files", files: [mergeHeader("a.kt")] })
    expect(fileMergeOf(mergeEntryOf(), "a.kt")).toBe("pending")
    const text: MergeContent = { kind: "text", ...textMerge([3, 3, 3], [mergeBlock([1, 2], [1, 2], [1, 2])]) }
    events!.onmessage({ kind: "content", index: 0, content: text })
    await vi.waitFor(() => expect(fileMergeOf(mergeEntryOf(), "a.kt")).toMatchObject({ path: "a.kt", content: text }))
    setFoldLevels(text, [3])
    const scopes = [{ startLine: 0, endLine: 3, label: "class A" }]
    events!.onmessage({ kind: "scopes", index: 0, leftScopes: scopes, baseScopes: [], rightScopes: [] })
    end()
    await vi.waitFor(() => expect(mergeEntryOf()).toMatchObject({ kind: "loaded", complete: true }))
    const labelled = fileMergeOf(mergeEntryOf(), "a.kt")
    expect(labelled !== "pending" && labelled?.content.kind === "text" ? [labelled.content.leftScopes, getFoldLevels(labelled.content)] : null).toEqual([scopes, [3]])
    // a merge of git's objects never changes: no second read
    loadFileMerges(SOURCE, "default")
    expect(getFileMerges).toHaveBeenCalledTimes(1)
  })

  it("says why a read failed", async () => {
    getFileMerges.mockImplementation(async () => ({ status: "error", error: { message: "no such commit" } }))
    loadFileMerges(SOURCE, "trim")
    await vi.waitFor(() => expect(mergeEntryOf("trim")).toEqual({ kind: "error", message: "no such commit" }))
    expect(fileMergeOf(mergeEntryOf("trim"), "a.kt")).toBeUndefined()
  })
})
