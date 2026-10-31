import { useStore } from "zustand"
import { commands } from "@/lib/bindings"
import type { FileMergeEvent, MergeContent } from "@/lib/bindings"
import type { WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { mergeSourceId } from "@/lib/diff/file-merge-model"
import type { FileMerge, FileMergeHeader, MergeSource } from "@/lib/diff/file-merge-model"
import { getFoldLevels, setFoldLevels } from "@/lib/diff/rows"
import { createFileStream, streamedFileOf } from "@/stores/file-stream"
import type { FileStreamEntry } from "@/stores/file-stream"
import { getDiffRepositoryPath } from "@/stores/repository"

export type FileMergesEntry = FileStreamEntry<FileMerge>

/** The conflicts kept: the ones the views show and the few before them */
export const MAX_CACHED_MERGES = 8

const stream = createFileStream<FileMergeHeader, MergeContent, FileMerge, FileMergeEvent>(
  MAX_CACHED_MERGES,
  "Cannot read the merges of the conflicting files",
  (header, event) => ({ ...header, content: event.content }),
  (file, event) => {
    if (file.content.kind !== "text") {
      return null
    }
    const content: MergeContent = { ...file.content, leftScopes: event.leftScopes, baseScopes: event.baseScopes, rightScopes: event.rightScopes }
    // the same merge with its labels: the lines it expanded stay expanded
    setFoldLevels(content, getFoldLevels(file.content))
    return { ...file, content }
  },
)

/** By `fileMergesKey`, the least recently loaded first */
export const fileMergesStore = stream.store

/** The key of the merges of a conflict in a repository: the engine's answer depends on the whitespace policy */
export function fileMergesKey(repositoryPath: string, source: MergeSource, policy: WhitespacePolicy): string {
  return `${policy}\n${repositoryPath}\0${mergeSourceId(source)}`
}

export function useFileMerges(source: MergeSource, policy: WhitespacePolicy): FileMergesEntry | undefined {
  const key = fileMergesKey(getDiffRepositoryPath() ?? "", source, policy)
  return useStore(fileMergesStore, state => state.byKey.get(key))
}

/**
 * Reads the merges of the conflicting files of a cherry-pick (`get_file_merges`) unless they are loaded or being read;
 * a failed read is tried again. The backend streams them as it streams the diffs: the files, the content of each, the
 * `priorityPaths` first, then the scopes of a text it parsed late. A merge of git's objects never changes.
 */
export function loadFileMerges(source: MergeSource, policy: WhitespacePolicy, priorityPaths: readonly string[] = []): void {
  const repositoryPath = getDiffRepositoryPath()
  if (!repositoryPath) {
    return
  }
  stream.load(fileMergesKey(repositoryPath, source, policy), channel => commands.getFileMerges({ repositoryPath, source, policy, priorityPaths: [...priorityPaths] }, channel))
}

/**
 * The merge of a file of an entry: undefined when the engine has none for it, "pending" while its conflict is read
 * and its content has not arrived
 */
export function fileMergeOf(entry: FileMergesEntry | undefined, path: string): FileMerge | "pending" | undefined {
  return streamedFileOf(entry, path)
}
