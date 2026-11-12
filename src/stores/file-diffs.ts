import { useEffect } from "react"
import { useStore } from "zustand"
import { commands } from "@/lib/bindings"
import type { DiffContent, DiffFileHeader, DiffMarks, FileDiffEvent, GetRenameProofsParams } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { withoutIds } from "@/lib/commit-files"
import { diffSourceOf, fileDiffSubjectId } from "@/lib/diff/file-diff-model"
import type { FileDiff, FileDiffSubject, WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { keepOpened } from "@/lib/diff/rows"
import { logError } from "@/lib/errors"
import { createFileStream, streamedFileOf } from "@/stores/file-stream"
import type { FileStreamEntry } from "@/stores/file-stream"
import { getDiffRepositoryPath } from "@/stores/repository"

export type FileDiffsEntry = FileStreamEntry<FileDiff>

/** The subjects kept: the commits and ranges a preview or a review shows and the few before them */
export const MAX_CACHED_SUBJECTS = 8

/**
 * The file with `marks` in place of the marks of its text, null for a file without one. The marks of a file whole, as
 * the other files of its subject or the index of the repository changed them after its content: they add runs that
 * are labels, words and what a reader may pass, never a row, so what the views opened of the diff stays open
 */
function withMarks(file: FileDiff, marks: DiffMarks): FileDiff | null {
  if (file.content.kind !== "text") {
    return null
  }
  const content: DiffContent = { ...file.content, marks }
  keepOpened(file.content, content)
  return { ...file, content }
}

/** The paths of the headers of a stream, one list for all its files (`FileDiff.subjectPaths`) */
const pathsOfHeaders = new WeakMap<readonly DiffFileHeader[], readonly string[]>()

function subjectPathsOf(headers: readonly DiffFileHeader[]): readonly string[] {
  let paths = pathsOfHeaders.get(headers)
  if (paths === undefined) {
    paths = headers.map(header => header.path)
    pathsOfHeaders.set(headers, paths)
  }
  return paths
}

const stream = createFileStream<DiffFileHeader, DiffContent, FileDiff, FileDiffEvent>(
  MAX_CACHED_SUBJECTS,
  "Cannot read the diffs of the files",
  // with the files of its subject: the marks of the subject name another file by its place among them
  (header, event, headers) => ({ ...header, content: event.content, collapse: event.collapse, subjectPaths: subjectPathsOf(headers) }),
  (file, event) => {
    if (file.content.kind !== "text") {
      return null
    }
    const content: DiffContent = { ...file.content, oldScopes: event.oldScopes, newScopes: event.newScopes, importBlocks: event.importBlocks }
    // the same diff with its labels: what the views opened of it stays open, its folds and its notes
    keepOpened(file.content, content)
    return { ...file, content }
  },
  (file, event) => withMarks(file, event.marks),
)

/** By `fileDiffsKey`, the least recently loaded first */
export const fileDiffsStore = stream.store

/** The key of a subject's diffs: the engine's answer depends on the whitespace policy */
export function fileDiffsKey(subject: FileDiffSubject, policy: WhitespacePolicy): string {
  return `${policy}\n${fileDiffSubjectId(subject)}`
}

/**
 * The diffs of a subject, for a view that shows them. While a view holds them, the index of the repository is asked
 * of the names the subject renamed (`holdFileDiffs`); a subject read ahead of its view (`loadFileDiffs` alone) asks
 * nothing.
 */
export function useFileDiffs(subject: FileDiffSubject | null, policy: WhitespacePolicy): FileDiffsEntry | undefined {
  const key = subject === null ? null : fileDiffsKey(subject, policy)
  useEffect(() => (key === null ? undefined : holdFileDiffs(key)), [key])
  return useStore(fileDiffsStore, state => (key === null ? undefined : state.byKey.get(key)))
}

export interface LoadFileDiffsOptions {
  /** Read the uncommitted changes again while the loaded ones stay shown */
  refresh?: boolean
  /** The files in view: the backend sends their diffs first */
  priorityPaths?: readonly string[]
}

/**
 * Reads the diffs of every file of a subject (`get_file_diffs`) unless they are loaded or being read; a failed read is
 * tried again. The backend streams them: the files, then the content of each as it is known, the `priorityPaths`
 * first, then the scopes of a text it parsed late, then the marks of the files that the others of the subject say
 * something of (the lines that moved between two files, the names another file declares). A file appears in the
 * entry with its content; the scopes and the marks replace those of the content of their file, which keeps its fold
 * levels. A commit or a range never changes; the uncommitted changes do, so `refresh` reads them again, and the loaded
 * ones stay shown until the new read ends; a refresh asked while a read runs reads once more after it.
 */
export function loadFileDiffs(subject: FileDiffSubject, policy: WhitespacePolicy, { refresh = false, priorityPaths = [] }: LoadFileDiffsOptions = {}): void {
  const repositoryPath = subject.kind === "uncommitted" ? subject.repositoryPath : getDiffRepositoryPath()
  if (!repositoryPath) {
    return
  }
  const source = diffSourceOf(subject)
  const key = fileDiffsKey(subject, policy)
  questions.set(key, { repositoryPath, source, policy })
  if (refresh) {
    // read again, its names may be asked again
    unanswered.delete(key)
  }
  stream.load(key, channel => commands.getFileDiffs({ repositoryPath, source, policy, priorityPaths: [...priorityPaths] }, channel), refresh)
  // what is known of the questions of the subjects the store keeps no more
  const kept = fileDiffsStore.getState().byKey
  for (const asked of questions.keys()) {
    if (!kept.has(asked)) {
      questions.delete(asked)
      unanswered.delete(asked)
    }
  }
}

/*
 * What the index of the repository says of the names a subject renamed (`get_rename_proofs`): a query of seconds on a
 * large repository, so the read of the diffs never waits for it. The diffs say that it is there to be asked (a proof
 * `asked` in the marks of a file), and the page asks it of the subjects a view shows: one question at a time, of the
 * subject held last. A subject read ahead of its view asks nothing; a question whose subject no view shows any more
 * gives way to the next (the backend kills its query), and a subject shown again is asked again. The answer is the
 * marks of the files it says something of, which take the place of theirs. A question that gave way ends later than
 * the one that took its place starts, maybe of the same subject shown again: its end is told from the end of the
 * running one by the question (`question`), never by its key, and says nothing of what runs.
 */

/** What `get_file_diffs` was asked of each key the store keeps: what the index is asked of its subject by */
const questions = new Map<string, GetRenameProofsParams>()
/** The views that hold each key, the key held last last */
const holders = new Map<string, number>()
/** The key whose question runs */
let asking: string | null = null
/** The number of the question that runs, or of the last one that ran: each question has its own */
let question = 0
/** The keys whose question ended without an answer for every name: not asked again until a view holds them anew */
const unanswered = new Set<string>()
/** Whether a complete entry has a name the index is there to be asked of, by the entry */
const asksIndex = new WeakMap<FileDiffsEntry, boolean>()

/** Whether the diffs of an entry say that the index is to be asked: the read ended, and a name of a file is `asked` */
function wantsIndex(entry: FileDiffsEntry | undefined): boolean {
  if (entry?.kind !== "loaded" || !entry.complete) {
    return false
  }
  let wants = asksIndex.get(entry)
  if (wants === undefined) {
    wants = entry.files.some(file => {
      const subject = file.content.kind === "text" ? file.content.marks.subject : null
      return subject != null && (subject.declared.some(check => check.proof === "asked") || subject.used.some(used => used.proof === "asked"))
    })
    asksIndex.set(entry, wants)
  }
  return wants
}

/**
 * A view shows the diffs of `key` (`fileDiffsKey`) until the returned release: the index is asked of the names its
 * subject renamed, now or once its diffs are read.
 */
export function holdFileDiffs(key: string): () => void {
  const held = holders.get(key) ?? 0
  if (held === 0) {
    unanswered.delete(key)
  }
  // held last: asked first
  holders.delete(key)
  holders.set(key, held + 1)
  askIndex()
  let released = false
  return () => {
    if (released) {
      return
    }
    released = true
    const left = (holders.get(key) ?? 1) - 1
    if (left > 0) {
      holders.set(key, left)
    } else {
      holders.delete(key)
    }
  }
}

/** Asks the index of the subject held last that waits for it, unless the question of a subject still shown runs */
function askIndex(): void {
  if (asking !== null && holders.has(asking)) {
    return
  }
  const entries = fileDiffsStore.getState().byKey
  let next: string | null = null
  for (const key of holders.keys()) {
    if (key !== asking && !unanswered.has(key) && wantsIndex(entries.get(key))) {
      next = key
    }
  }
  const params = next === null ? undefined : questions.get(next)
  if (next === null || params === undefined) {
    return
  }
  const key = next
  // the question of a subject no view shows gives way: the backend ends it as this one starts
  asking = key
  question += 1
  const asked = question
  void invokeCommand(() => commands.getRenameProofs(params)).then(result => {
    if (result.status === "error") {
      logError("Cannot ask the index of the renames of the files", result.error)
    } else if (result.data !== null) {
      const marks = new Map(result.data.map(file => [file.path, file.marks]))
      stream.update(key, file => {
        const answered = marks.get(file.path)
        return answered === undefined ? null : withMarks(file, answered)
      })
    }
    if (asked !== question) {
      // it gave way to a later question, maybe of this subject again: that one runs, and says what it leaves asked
      return
    }
    // an answer that left a name asked is none to ask for again: not before a view shows the subject anew
    if (wantsIndex(fileDiffsStore.getState().byKey.get(key))) {
      unanswered.add(key)
    }
    // the question ends here, after its answer is in the store: the store says each change to `askIndex`
    asking = null
    askIndex()
  })
}

// a read that ended may have a name to ask
fileDiffsStore.subscribe(askIndex)

/**
 * Runs `run` once the read of the subject's diffs ended (complete, or failed), at once if it has: the review reads the
 * neighbouring commits then, so their streams never compete with the shown one for the workers of the backend and
 * share the pairs it already diffed. Returns the cancel.
 */
export function whenFileDiffsEnded(subject: FileDiffSubject, policy: WhitespacePolicy, run: () => void): () => void {
  const key = fileDiffsKey(subject, policy)
  const ended = () => {
    const entry = fileDiffsStore.getState().byKey.get(key)
    return entry?.kind === "error" || (entry?.kind === "loaded" && entry.complete)
  }
  if (ended()) {
    run()
    return () => {}
  }
  const unsubscribe = fileDiffsStore.subscribe(() => {
    if (ended()) {
      unsubscribe()
      run()
    }
  })
  return unsubscribe
}

/**
 * The diff of a file of an entry: undefined when the engine has none for it, "pending" while its subject is read and
 * its content has not arrived
 */
export function fileDiffOf(entry: FileDiffsEntry | undefined, path: string): FileDiff | "pending" | undefined {
  return streamedFileOf(entry, path)
}

/** Forgets the diffs of these subject ids (`fileDiffSubjectId`) under every policy: the ranges of a closed review */
export function dropFileDiffsOf(subjectIds: readonly string[]): void {
  const ids = new Set(subjectIds)
  fileDiffsStore.setState(state => ({
    byKey: withoutIds(
      state.byKey,
      [...state.byKey.keys()].filter(key => ids.has(key.slice(key.indexOf("\n") + 1))),
    ),
  }))
}
