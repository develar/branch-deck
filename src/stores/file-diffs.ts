import { useStore } from "zustand"
import { commands } from "@/lib/bindings"
import type { DiffContent, DiffFileHeader, FileDiffEvent } from "@/lib/bindings"
import { withoutIds } from "@/lib/commit-files"
import { diffSourceOf, fileDiffSubjectId } from "@/lib/diff/file-diff-model"
import type { FileDiff, FileDiffSubject, WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { getFoldLevels, setFoldLevels } from "@/lib/diff/rows"
import { createFileStream, streamedFileOf } from "@/stores/file-stream"
import type { FileStreamEntry } from "@/stores/file-stream"
import { getDiffRepositoryPath } from "@/stores/repository"

export type FileDiffsEntry = FileStreamEntry<FileDiff>

/** The subjects kept: the commits and ranges a preview or a review shows and the few before them */
export const MAX_CACHED_SUBJECTS = 8

const stream = createFileStream<DiffFileHeader, DiffContent, FileDiff, FileDiffEvent>(MAX_CACHED_SUBJECTS, "Cannot read the diffs of the files", (file, event) => {
  if (file.content.kind !== "text") {
    return null
  }
  const content: DiffContent = { ...file.content, oldScopes: event.oldScopes, newScopes: event.newScopes }
  // the same diff with its labels: the lines it expanded stay expanded
  setFoldLevels(content, getFoldLevels(file.content))
  return { ...file, content }
})

/** By `fileDiffsKey`, the least recently loaded first */
export const fileDiffsStore = stream.store

/** The key of a subject's diffs: the engine's answer depends on the whitespace policy */
export function fileDiffsKey(subject: FileDiffSubject, policy: WhitespacePolicy): string {
  return `${policy}\n${fileDiffSubjectId(subject)}`
}

export function useFileDiffs(subject: FileDiffSubject | null, policy: WhitespacePolicy): FileDiffsEntry | undefined {
  const key = subject === null ? null : fileDiffsKey(subject, policy)
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
 * first, then the scopes of a text it parsed late. A file appears in the entry with its content; the scopes replace
 * the content of their file, which keeps its fold levels. A commit or a range never changes; the uncommitted changes
 * do, so `refresh` reads them again, and the loaded ones stay shown until the new read ends; a refresh asked while a
 * read runs reads once more after it.
 */
export function loadFileDiffs(subject: FileDiffSubject, policy: WhitespacePolicy, { refresh = false, priorityPaths = [] }: LoadFileDiffsOptions = {}): void {
  const repositoryPath = subject.kind === "uncommitted" ? subject.repositoryPath : getDiffRepositoryPath()
  if (!repositoryPath) {
    return
  }
  const source = diffSourceOf(subject)
  stream.load(fileDiffsKey(subject, policy), channel => commands.getFileDiffs({ repositoryPath, source, policy, priorityPaths: [...priorityPaths] }, channel), refresh)
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
