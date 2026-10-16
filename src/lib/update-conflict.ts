import type { UpdateCommit } from "@/lib/bindings"
import { formatRelativeTime } from "@/lib/time"

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
  const local = commitsByFile(commits)
  const upstream = commitsByFile(upstreamCommits)
  return files.map(path => ({ path, local: local.get(path) ?? [], upstream: upstream.get(path) ?? [] }))
}

/** Path -> the commits that change it, in their order: one pass over the files of the commits, not one per file */
function commitsByFile(commits: readonly UpdateCommit[]): Map<string, UpdateCommit[]> {
  const byFile = new Map<string, UpdateCommit[]>()
  for (const commit of commits) {
    for (const path of commit.files) {
      const list = byFile.get(path)
      if (list === undefined) {
        byFile.set(path, [commit])
      } else {
        list.push(commit)
      }
    }
  }
  return byFile
}

/** `src/lib/app.ts` as the name `app.ts` and the directory `src/lib/` (empty at the root) */
export function splitPath(path: string): { name: string; directory: string } {
  const slash = path.lastIndexOf("/")
  return { name: path.slice(slash + 1), directory: path.slice(0, slash + 1) }
}

/** An upstream commit whose committer time is further than this from its author time shows both */
const LANDED_LATER_SECONDS = 60 * 60

/**
 * Who wrote an upstream commit and when, and when it landed in the upstream (its committer time) when that differs by
 * more than an hour: "Dana Lee · authored 13 days ago · landed 2 hours ago", else "Dana Lee, 13 days ago".
 */
export function describeUpstreamCommit(commit: UpdateCommit): string {
  const authored = formatRelativeTime(commit.time * 1000)
  if (Math.abs(commit.committerTime - commit.time) <= LANDED_LATER_SECONDS) {
    return `${commit.author}, ${authored}`
  }
  return `${commit.author} · authored ${authored} · landed ${formatRelativeTime(commit.committerTime * 1000)}`
}
