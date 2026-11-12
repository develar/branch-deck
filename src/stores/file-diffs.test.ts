// @spec openspec/specs/commit/commit-files/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type {
  DiffFileHeader,
  DiffMarks,
  FileDiffEvent,
  FileMarks,
  FileMergeEvent,
  FileMergeHeader,
  GetFileDiffsParams,
  GetFileMergesParams,
  GetRenameProofsParams,
  MergeContent,
  RenameProof,
} from "@/lib/bindings"
import type { DiffContent } from "@/lib/diff/file-diff-model"
import type { MergeSource } from "@/lib/diff/file-merge-model"
import { getFoldLevels, getOpenNotes, sectionFoldKey, setFoldLevels, setOpenNotes } from "@/lib/diff/rows"
import { dropFileDiffsOf, fileDiffOf, fileDiffsKey, fileDiffsStore, holdFileDiffs, loadFileDiffs, MAX_CACHED_SUBJECTS, whenFileDiffsEnded } from "@/stores/file-diffs"
import { fileMergeOf, fileMergesKey, fileMergesStore, loadFileMerges } from "@/stores/file-merges"
import { block, mergeBlock, run, textDiff, textMerge } from "@/test-support/diff-blocks"

const getFileDiffs = vi.hoisted(() => vi.fn())
const getFileMerges = vi.hoisted(() => vi.fn())
const getRenameProofs = vi.hoisted(() => vi.fn())
const logError = vi.hoisted(() => vi.fn())

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", () => ({ commands: { getFileDiffs, getFileMerges, getRenameProofs } }))
vi.mock("@/lib/errors", () => ({ logError }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/command", () => import("@/test-support/invoke-command-mock"))

type Events = { onmessage: (event: FileDiffEvent) => void }

function header(path: string): DiffFileHeader {
  return { path, oldPath: null, status: "modified", oldId: "1", newId: "2" }
}

const BINARY: DiffContent = { kind: "binary" }

const UNCOMMITTED = { kind: "uncommitted", repositoryPath: "/repo" } as const

/** The answer of a subject: its one file, binary, as the backend streams it */
async function answer(params: GetFileDiffsParams, events: Events) {
  const path = params.source.kind === "commit" ? params.source.id : params.source.kind
  events.onmessage({ kind: "files", files: [header(path)] })
  events.onmessage({ kind: "content", index: 0, content: BINARY, collapse: { kind: "noText" } })
  return { status: "ok", data: null }
}

beforeEach(() => {
  fileDiffsStore.setState({ byKey: new Map() })
  getFileDiffs.mockReset()
  getFileDiffs.mockImplementation(answer)
  getRenameProofs.mockReset()
  logError.mockReset()
})

const entryOf = (subject: Parameters<typeof fileDiffsKey>[0], policy: Parameters<typeof fileDiffsKey>[1] = "default") =>
  fileDiffsStore.getState().byKey.get(fileDiffsKey(subject, policy))

/** Holds the next `get_file_diffs`: the test sends its events through `events()` and ends it with `end()` */
function holdNextStream(): { events: () => Events; end: () => void } {
  let channel: Events | undefined
  let resolveCall = () => {}
  getFileDiffs.mockImplementationOnce((_params: GetFileDiffsParams, events: Events) => {
    channel = events
    return new Promise(resolve => (resolveCall = () => resolve({ status: "ok", data: null })))
  })
  return { events: () => channel!, end: () => resolveCall() }
}

/** The marks `marks` with what the other files of a subject say of the file, the name another file declares `proof` */
function subjectMarks(marks: DiffMarks, proof: RenameProof): DiffMarks {
  const renamed = { old: "openStore", new: "connectStore", sites: 1, local: false, declared: [], oldSites: [5, 2, 11], newSites: [5, 2, 14] }
  return {
    ...marks,
    runs: [run(5, 6, 5, 6, { kind: "renamed", quiet: false })],
    subject: {
      movedOut: [],
      movedIn: [{ start: 8, len: 1, file: 1, line: 3 }],
      declared: [],
      used: [{ rename: renamed, file: 1, line: 2, proof }],
      confirmed: proof === "confirmed" ? [0] : [],
    },
  }
}

/** The text of a file of a subject as the store has it */
function textOf(subject: Parameters<typeof fileDiffsKey>[0], path: string) {
  const file = fileDiffOf(entryOf(subject), path)
  return file !== undefined && file !== "pending" && file.content.kind === "text" ? file.content : null
}

/** The proof of the name `a.kt` of a subject uses */
const proofOf = (subject: Parameters<typeof fileDiffsKey>[0]) => textOf(subject, "a.kt")?.marks.subject?.used[0]?.proof

/** Reads the diffs of a subject of one file, `a.kt`, whose name another file declares is to be asked of the index */
async function loadAsked(subject: Parameters<typeof fileDiffsKey>[0]) {
  const text: DiffContent & { kind: "text" } = { kind: "text", ...textDiff(10, 10, [block(5, 6, 5, 6), block(8, 9, 8, 9)]) }
  text.marks = subjectMarks(text.marks, "asked")
  getFileDiffs.mockImplementationOnce(async (_params: GetFileDiffsParams, events: Events) => {
    events.onmessage({ kind: "files", files: [header("a.kt")] })
    events.onmessage({ kind: "content", index: 0, content: text, collapse: null })
    return { status: "ok", data: null }
  })
  loadFileDiffs(subject, "default")
  await vi.waitFor(() => expect(entryOf(subject)).toMatchObject({ kind: "loaded", complete: true }))
  return text
}

/** Holds the next `get_rename_proofs` until the test answers it */
function holdNextQuestion(): { answer: (data: FileMarks[] | null) => void } {
  let resolveCall: (data: FileMarks[] | null) => void = () => {}
  getRenameProofs.mockImplementationOnce(() => new Promise(resolve => (resolveCall = data => resolve({ status: "ok", data }))))
  return { answer: data => resolveCall(data) }
}

/** Three subjects read with a name to ask of the index, and four questions of it held: what was asked, in its order */
async function threeAsked(...ids: [string, string, string]) {
  const subjects = ids.map(id => ({ kind: "commit", id }) as const) as [{ kind: "commit"; id: string }, { kind: "commit"; id: string }, { kind: "commit"; id: string }]
  const texts = [await loadAsked(subjects[0]), await loadAsked(subjects[1]), await loadAsked(subjects[2])]
  return {
    subjects,
    key: (subject: Parameters<typeof fileDiffsKey>[0]) => fileDiffsKey(subject, "default"),
    texts,
    asked: () => getRenameProofs.mock.calls.map(([params]) => (params as GetRenameProofsParams).source),
    questions: [holdNextQuestion(), holdNextQuestion(), holdNextQuestion(), holdNextQuestion()],
  }
}

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
    expect(entryOf({ kind: "commit", id: "c1" }, "ignore")).toEqual({
      kind: "loaded",
      files: [{ ...header("c1"), content: BINARY, collapse: { kind: "noText" }, subjectPaths: ["c1"] }],
      complete: true,
    })
  })

  it("shows each file as its content arrives, and the late scopes of a text keep the lines it expanded", async () => {
    const { events, end } = holdNextStream()
    const subject = { kind: "commit", id: "c2" } as const
    loadFileDiffs(subject, "default")
    expect(entryOf(subject)?.kind).toBe("loading")
    events().onmessage({ kind: "files", files: [header("a.kt"), header("b.kt")] })
    expect([entryOf(subject), fileDiffOf(entryOf(subject), "a.kt")]).toEqual([{ kind: "loaded", files: [], complete: false }, "pending"])

    const text: DiffContent = { kind: "text", ...textDiff(10, 10, [block(5, 6, 5, 6)]) }
    events().onmessage({ kind: "content", index: 1, content: text, collapse: null })
    await vi.waitFor(() => expect(fileDiffOf(entryOf(subject), "b.kt")).toMatchObject({ path: "b.kt", content: text }))
    setFoldLevels(text, [3])
    const scopes = [{ startLine: 0, endLine: 10, label: "class B" }]
    events().onmessage({ kind: "scopes", index: 1, oldScopes: [], newScopes: scopes, importBlocks: [] })
    end()
    await settle()
    const labelled = fileDiffOf(entryOf(subject), "b.kt")
    expect(labelled !== "pending" && labelled?.content.kind === "text" ? [labelled.content.newScopes, getFoldLevels(labelled.content)] : null).toEqual([scopes, [3]])
    // the stream ended without the content of a.kt: the engine has none for it
    expect(fileDiffOf(entryOf(subject), "a.kt")).toBeUndefined()
  })

  it("gives each file the paths of the files of its subject in the order of the stream, whatever content came first and whatever came after it", async () => {
    const { events, end } = holdNextStream()
    const subject = { kind: "commit", id: "c15" } as const
    loadFileDiffs(subject, "default")
    events().onmessage({ kind: "files", files: [header("src/Ledger.kt"), header("src/Report.kt"), header("src/Store.kt")] })
    const text: DiffContent & { kind: "text" } = { kind: "text", ...textDiff(10, 10, [block(5, 6, 5, 6), block(8, 9, 8, 9)]) }
    // the second file first: the entry holds it alone, and it still names the first one by its place in the stream
    events().onmessage({ kind: "content", index: 1, content: text, collapse: null })
    const paths = ["src/Ledger.kt", "src/Report.kt", "src/Store.kt"]
    const pathsOf = (path: string) => {
      const file = fileDiffOf(entryOf(subject), path)
      return file === undefined || file === "pending" ? file : file.subjectPaths
    }
    await vi.waitFor(() => expect([pathsOf("src/Report.kt"), pathsOf("src/Ledger.kt")]).toEqual([paths, "pending"]))
    // its scopes and the marks of its subject replace its content, never the files it names
    events().onmessage({ kind: "scopes", index: 1, oldScopes: [], newScopes: [], importBlocks: [] })
    events().onmessage({ kind: "marks", index: 1, marks: subjectMarks(text.marks, "asked") })
    events().onmessage({ kind: "content", index: 0, content: BINARY, collapse: { kind: "noText" } })
    end()
    await settle()
    expect(textOf(subject, "src/Report.kt")?.marks.subject?.movedIn).toEqual([{ start: 8, len: 1, file: 1, line: 3 }])
    // one list for all the files of the stream
    expect(pathsOf("src/Report.kt")).toEqual(paths)
    expect(pathsOf("src/Ledger.kt")).toBe(pathsOf("src/Report.kt"))
  })

  it("keeps the notes a view opened across the late scopes of a text, and the folds and the notes of its sections", async () => {
    const { events, end } = holdNextStream()
    const subject = { kind: "commit", id: "c9" } as const
    loadFileDiffs(subject, "default")
    events().onmessage({ kind: "files", files: [header("a.kt")] })
    const text: DiffContent = { kind: "text", ...textDiff(10, 12, [block(5, 6, 5, 8)], { runs: [run(5, 6, 5, 8)] }) }
    events().onmessage({ kind: "content", index: 0, content: text, collapse: null })
    await vi.waitFor(() => expect(fileDiffOf(entryOf(subject), "a.kt")).toMatchObject({ content: text }))
    // the note of the quiet run opened before the scopes came, and a fold and the note in a section of a review
    setOpenNotes(text, new Set([5]))
    const section = sectionFoldKey(text, "a.kt#1")
    setFoldLevels(section, [2])
    setOpenNotes(section, new Set([5]))
    events().onmessage({ kind: "scopes", index: 0, oldScopes: [], newScopes: [{ startLine: 0, endLine: 12, label: "class A" }], importBlocks: [] })
    end()
    await settle()
    const labelled = fileDiffOf(entryOf(subject), "a.kt")
    const content = labelled !== "pending" && labelled?.content.kind === "text" ? labelled.content : null
    // another object, the same diff with its marks: the note is as open as it was
    expect(content).not.toBe(text)
    expect([content?.newScopes.length, content?.marks.runs.length, [...getOpenNotes(content!)]]).toEqual([1, 1, [5]])
    // its sections keep their keys, by which the views hold what they opened
    expect(sectionFoldKey(content!, "a.kt#1")).toBe(section)
    expect([getFoldLevels(sectionFoldKey(content!, "a.kt#1")), [...getOpenNotes(sectionFoldKey(content!, "a.kt#1"))]]).toEqual([[2], [5]])
    // a diff nothing was opened of has nothing to keep
    expect([getFoldLevels(textDiff(1, 1, [])), getOpenNotes(textDiff(1, 1, [])).size]).toEqual([[], 0])
  })

  it("takes the marks of a late marks event in place of those of the content, and keeps what a view opened of the file", async () => {
    const { events, end } = holdNextStream()
    const subject = { kind: "commit", id: "c10" } as const
    loadFileDiffs(subject, "default")
    events().onmessage({ kind: "files", files: [header("a.kt"), header("bin.dat")] })
    const text: DiffContent = { kind: "text", ...textDiff(10, 10, [block(5, 6, 5, 6), block(8, 9, 8, 9)]) }
    events().onmessage({ kind: "content", index: 0, content: text, collapse: null })
    events().onmessage({ kind: "content", index: 1, content: BINARY, collapse: { kind: "noText" } })
    await vi.waitFor(() => expect(fileDiffOf(entryOf(subject), "a.kt")).toMatchObject({ content: text }))
    setFoldLevels(text, [3])
    setOpenNotes(text, new Set([5]))
    // what the other files of the subject say of it: a run that is a label, a name another file declares, lines that
    // moved in from another file
    const marks = subjectMarks(text.marks, "unavailable")
    events().onmessage({ kind: "marks", index: 0, marks })
    // the marks of a file without a text, and of a file whose content never came, change nothing
    events().onmessage({ kind: "marks", index: 1, marks })
    events().onmessage({ kind: "marks", index: 7, marks })
    end()
    await settle()
    const content = textOf(subject, "a.kt")
    expect(content).not.toBe(text)
    expect(content?.marks).toEqual(marks)
    // the same blocks, so the stops and the rows are those of the content: the folds and the notes stay
    expect([content?.blocks, getFoldLevels(content!), [...getOpenNotes(content!)]]).toEqual([text.blocks, [3], [5]])
    expect(fileDiffOf(entryOf(subject), "bin.dat")).toMatchObject({ content: BINARY })
    // no name of it waits for the index: nothing is asked of a view that shows it
    holdFileDiffs(fileDiffsKey(subject, "default"))()
    expect(getRenameProofs).not.toHaveBeenCalled()
  })

  it("ends the read of a subject with its diffs, and asks the index of its renamed names once a view shows it", async () => {
    const subject = { kind: "commit", id: "c11" } as const
    const text = await loadAsked(subject)
    // the read ended with the names to be asked: what waits for its end goes on, and a subject no view shows asks nothing
    const ended: string[] = []
    whenFileDiffsEnded(subject, "default", () => ended.push("ended"))
    expect([entryOf(subject), ended, getRenameProofs.mock.calls.length]).toMatchObject([{ kind: "loaded", complete: true }, ["ended"], 0])
    setFoldLevels(textOf(subject, "a.kt")!, [3])

    // a view shows it: the index is asked of the subject, once however many views hold it
    const question = holdNextQuestion()
    const release = holdFileDiffs(fileDiffsKey(subject, "default"))
    const releaseOther = holdFileDiffs(fileDiffsKey(subject, "default"))
    expect(getRenameProofs.mock.calls.map(([params]) => params as GetRenameProofsParams)).toEqual([
      { repositoryPath: "/repo", source: { kind: "commit", id: "c11" }, policy: "default" },
    ])
    expect(proofOf(subject)).toBe("asked")
    // its answer takes the place of the marks of the files it names, in the entry as it is: complete, its folds kept
    const confirmed = subjectMarks(text.marks, "confirmed")
    question.answer([
      { path: "a.kt", marks: confirmed },
      { path: "gone.kt", marks: confirmed },
    ])
    await vi.waitFor(() => expect(proofOf(subject)).toBe("confirmed"))
    const content = textOf(subject, "a.kt")!
    expect([content.marks, content.blocks, getFoldLevels(content), entryOf(subject)]).toEqual([
      confirmed,
      text.blocks,
      [3],
      { kind: "loaded", files: [{ ...header("a.kt"), content, collapse: null, subjectPaths: ["a.kt"] }], complete: true },
    ])
    // answered: a view that shows it again asks nothing
    release()
    releaseOther()
    holdFileDiffs(fileDiffsKey(subject, "default"))()
    expect(getRenameProofs).toHaveBeenCalledTimes(1)
  })

  it("asks the index of one subject at a time, the one shown last first, and lets the question of a subject no view shows give way", async () => {
    const { subjects, key, texts, asked, questions } = await threeAsked("c12", "c13", "c14")
    const [first, second, third] = subjects

    // two views show two subjects: the question of the first runs, the second waits for it
    const releaseFirst = holdFileDiffs(key(first))
    const releaseSecond = holdFileDiffs(key(second))
    expect(asked()).toEqual([first])
    // the first view goes and another shows the third subject: the question of the first gives way to the one shown
    // last (the backend ends the query of a window that asks another), and answers nothing
    releaseFirst()
    const releaseThird = holdFileDiffs(key(third))
    expect(asked()).toEqual([first, third])
    questions[0]!.answer(null)
    await Promise.resolve()
    expect(asked()).toEqual([first, third])
    // the third answered: the second, still shown, is asked
    questions[1]!.answer([{ path: "a.kt", marks: subjectMarks(texts[2]!.marks, "confirmed") }])
    await vi.waitFor(() => expect(asked()).toEqual([first, third, second]))
    expect([proofOf(first), proofOf(second), proofOf(third)]).toEqual(["asked", "asked", "confirmed"])
    // an answer that leaves the name of a subject still shown asked: it is not asked again until a view shows it anew
    questions[2]!.answer([{ path: "a.kt", marks: { ...subjectMarks(texts[1]!.marks, "asked"), starts: [] } }])
    await vi.waitFor(() => expect(textOf(second, "a.kt")?.marks.starts).toEqual([]))
    await Promise.resolve()
    releaseThird()
    expect(asked()).toEqual([first, third, second])
    releaseSecond()
    // the first is shown again: asked again, as is the second then
    const againFirst = holdFileDiffs(key(first))
    expect(asked()).toEqual([first, third, second, first])
    getRenameProofs.mockImplementationOnce(async () => ({ status: "error", error: { message: "no such commit" } }))
    const againSecond = holdFileDiffs(key(second))
    questions[3]!.answer([{ path: "a.kt", marks: subjectMarks(texts[0]!.marks, "unavailable") }])
    await vi.waitFor(() => expect(logError).toHaveBeenCalledTimes(1))
    expect(asked()).toEqual([first, third, second, first, second])
    expect([proofOf(first), proofOf(second)]).toEqual(["unavailable", "asked"])
    againFirst()
    againSecond()
  })

  it("keeps the question of a subject shown again running when the question it gave way for ends after it started", async () => {
    const { subjects, key, texts, asked, questions } = await threeAsked("c15", "c16", "c17")
    const [first, second, third] = subjects

    // a view shows the first subject, then the second, then the first again, before the first question ended
    holdFileDiffs(key(first))()
    holdFileDiffs(key(second))()
    const releaseFirst = holdFileDiffs(key(first))
    expect(asked()).toEqual([first, second, first])
    // the questions that gave way end now, without an answer: the question of the first subject still runs, so the
    // subject another view shows waits for it
    questions[0]!.answer(null)
    questions[1]!.answer(null)
    await settle()
    const releaseThird = holdFileDiffs(key(third))
    expect(asked()).toEqual([first, second, first])
    // its answer is taken, and only then the third is asked
    questions[2]!.answer([{ path: "a.kt", marks: subjectMarks(texts[0]!.marks, "confirmed") }])
    await vi.waitFor(() => expect(asked()).toEqual([first, second, first, third]))
    expect([proofOf(first), proofOf(second), proofOf(third)]).toEqual(["confirmed", "asked", "asked"])
    questions[3]!.answer([{ path: "a.kt", marks: subjectMarks(texts[2]!.marks, "confirmed") }])
    await vi.waitFor(() => expect(proofOf(third)).toBe("confirmed"))
    releaseFirst()
    releaseThird()
  })

  it("tells the end of a subject's read only once its stream ended: the neighbours of a review are read after it", async () => {
    const { events, end } = holdNextStream()
    const shown = { kind: "commit", id: "c3" } as const
    const neighbour = { kind: "commit", id: "c4" } as const
    loadFileDiffs(shown, "default")
    const prefetch = () => loadFileDiffs(neighbour, "default")
    whenFileDiffsEnded(shown, "default", prefetch)
    // the files and a content arrived: the shown stream still runs, the neighbour waits
    events().onmessage({ kind: "files", files: [header("a.kt"), header("b.kt")] })
    events().onmessage({ kind: "content", index: 0, content: BINARY, collapse: { kind: "noText" } })
    await vi.waitFor(() => expect(fileDiffOf(entryOf(shown), "a.kt")).toMatchObject({ path: "a.kt" }))
    expect(getFileDiffs.mock.calls.map(([params]) => (params as GetFileDiffsParams).source)).toEqual([shown])
    end()
    await settle()
    expect(getFileDiffs.mock.calls.map(([params]) => (params as GetFileDiffsParams).source)).toEqual([shown, neighbour])
    // an ended read runs it at once; a cancel before the end runs nothing
    const ran: string[] = []
    whenFileDiffsEnded(shown, "default", () => ran.push("at once"))
    let endPending = () => {}
    getFileDiffs.mockImplementationOnce((params: GetFileDiffsParams, channel: Events) => new Promise(resolve => (endPending = () => resolve(answer(params, channel)))))
    const pending = { kind: "commit", id: "c5" } as const
    loadFileDiffs(pending, "default")
    whenFileDiffsEnded(pending, "default", () => ran.push("canceled"))()
    endPending()
    await settle()
    expect(ran).toEqual(["at once"])
  })

  it("reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends", async () => {
    loadFileDiffs(UNCOMMITTED, "default")
    await settle()
    let release = () => {}
    getFileDiffs.mockImplementationOnce(
      (_params: GetFileDiffsParams, events: Events) =>
        new Promise(resolve => {
          events.onmessage({ kind: "files", files: [header("next")] })
          release = () => {
            events.onmessage({ kind: "content", index: 0, content: BINARY, collapse: { kind: "noText" } })
            resolve({ status: "ok", data: null })
          }
        }),
    )
    loadFileDiffs(UNCOMMITTED, "default", { refresh: true })
    expect(entryOf(UNCOMMITTED)).toEqual({
      kind: "loaded",
      files: [{ ...header("uncommitted"), content: BINARY, collapse: { kind: "noText" }, subjectPaths: ["uncommitted"] }],
      complete: true,
    })
    release()
    await vi.waitFor(() =>
      expect(entryOf(UNCOMMITTED)).toEqual({
        kind: "loaded",
        files: [{ ...header("next"), content: BINARY, collapse: { kind: "noText" }, subjectPaths: ["next"] }],
        complete: true,
      }),
    )
  })

  it("reads the uncommitted changes once more after a refresh asked during a read, never dropping it", async () => {
    let release = () => {}
    getFileDiffs.mockImplementationOnce(
      (params: GetFileDiffsParams, events: Events) =>
        new Promise(resolve => {
          release = () => resolve(answer(params, events))
        }),
    )
    loadFileDiffs(UNCOMMITTED, "default")
    // the files change during the read: the form asks again, twice
    getFileDiffs.mockImplementationOnce(async (_params: GetFileDiffsParams, events: Events) => {
      events.onmessage({ kind: "files", files: [header("changed")] })
      events.onmessage({ kind: "content", index: 0, content: BINARY, collapse: { kind: "noText" } })
      return { status: "ok", data: null }
    })
    loadFileDiffs(UNCOMMITTED, "default", { refresh: true })
    loadFileDiffs(UNCOMMITTED, "default", { refresh: true })
    release()
    await vi.waitFor(() =>
      expect(entryOf(UNCOMMITTED)).toEqual({
        kind: "loaded",
        files: [{ ...header("changed"), content: BINARY, collapse: { kind: "noText" }, subjectPaths: ["changed"] }],
        complete: true,
      }),
    )
    // one read after the one that ran, for both refreshes
    expect(getFileDiffs).toHaveBeenCalledTimes(2)
  })

  it("keeps the uncommitted changes of two repositories apart, each read from its own working tree", async () => {
    const other = { kind: "uncommitted", repositoryPath: "/other" } as const
    loadFileDiffs(UNCOMMITTED, "default")
    loadFileDiffs(other, "default")
    await settle()
    expect(getFileDiffs.mock.calls.map(([params]) => (params as GetFileDiffsParams).repositoryPath)).toEqual(["/repo", "/other"])
    expect(fileDiffsKey(UNCOMMITTED, "default")).not.toBe(fileDiffsKey(other, "default"))
    expect(entryOf(other)?.kind).toBe("loaded")
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
  return { path, kind: "content", baseId: "1", leftId: "2", rightId: "3", resolvedId: null }
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
