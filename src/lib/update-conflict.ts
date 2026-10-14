import type { UpdateCommit } from "@/lib/bindings"

/** A likely conflicting file of an update, with the commits on both sides that change it */
export interface ConflictFile {
  path: string
  /** The local commits that change the file, oldest first */
  local: UpdateCommit[]
  /** The upstream commits that change the file, oldest first */
  upstream: UpdateCommit[]
}

/** Each likely conflicting file, in the order of `files`, with the local and upstream commits that change it */
export function groupConflictFiles(files: readonly string[], commits: readonly UpdateCommit[], upstreamCommits: readonly UpdateCommit[]): ConflictFile[] {
  return files.map(path => ({
    path,
    local: commits.filter(commit => commit.files.includes(path)),
    upstream: upstreamCommits.filter(commit => commit.files.includes(path)),
  }))
}

/** `src/lib/app.ts` as the name `app.ts` and the directory `src/lib/` (empty at the root) */
export function splitPath(path: string): { name: string, directory: string } {
  const slash = path.lastIndexOf("/")
  return { name: path.slice(slash + 1), directory: path.slice(0, slash + 1) }
}
