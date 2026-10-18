import type { CommitFile } from "@/lib/bindings"
import { matchesAllWords, searchWords } from "@/lib/text-search"

/** The search of a review's files shows from this many files on, or while it has a text: fewer are read in one look */
export const FILE_FILTER_MIN_FILES = 8

/** The files with every word of `text` in their path or the old path of a rename, ignoring case; all for a blank text */
export function filterFilesByText(files: readonly CommitFile[], text: string): readonly CommitFile[] {
  const words = searchWords(text)
  if (words.length === 0) {
    return files
  }
  return files.filter(file => matchesAllWords(file.oldPath === null ? [file.path] : [file.path, file.oldPath], words))
}

/** What a search that matched no file says */
export function formatNoFileMatch(text: string): string {
  return `No files match “${text.trim()}”`
}
