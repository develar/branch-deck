import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { CommitPatch } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, withoutIds } from "@/lib/commit-files"
import { logError } from "@/lib/errors"
import { getDiffRepositoryPath } from "@/stores/repository"

export type CommitPatchEntry = { kind: "loading" } | { kind: "error"; message: string } | { kind: "loaded"; patch: CommitPatch }

export interface CommitPatchState {
  /**
   * By commit id, or `<base>..<head>` for a range (`diffSubjectId`), the least recently loaded first. A patch never
   * changes, so an entry is never stale
   */
  byCommit: ReadonlyMap<string, CommitPatchEntry>
}

/** The patches kept: the commits (and ranges) a preview or a review shows and the few before them */
export const MAX_CACHED_PATCHES = 8
/** The patch text kept, in UTF-16 code units: a few huge commits must not keep the memory */
export const MAX_CACHED_PATCH_LENGTH = 32 * 1024 * 1024

export const commitPatchStore = createStore<CommitPatchState>(() => ({ byCommit: new Map() }))

export function useCommitPatch(commitId: string): CommitPatchEntry | undefined {
  return useStore(commitPatchStore, state => state.byCommit.get(commitId))
}

/**
 * Reads the patch of a commit (`get_commit_patch`: one `diff-tree -p`) unless it is loaded or being loaded; a failed
 * load is tried again. A loaded patch moves to the end of the cache, so the shown commits are evicted last.
 */
export function loadCommitPatch(commitId: string): void {
  loadSubjectPatch({ kind: "commit", id: commitId })
}

/**
 * Reads the patch of a subject of a review: a commit with `get_commit_patch`, a range with `get_range_patch` (one
 * `diff-tree -p` of the two commits), cached as a commit's
 */
export function loadSubjectPatch(subject: DiffSubject): void {
  const commitId = diffSubjectId(subject)
  const entry = commitPatchStore.getState().byCommit.get(commitId)
  if (entry?.kind === "loading") {
    return
  }
  if (entry?.kind === "loaded") {
    commitPatchStore.setState(state => ({ byCommit: withEntry(state.byCommit, commitId, entry) }))
    return
  }
  const repositoryPath = getDiffRepositoryPath()
  if (!repositoryPath) {
    return
  }
  commitPatchStore.setState(state => ({ byCommit: withEntry(state.byCommit, commitId, { kind: "loading" }) }))
  const read =
    subject.kind === "commit"
      ? () => commands.getCommitPatch({ repositoryPath, commitId })
      : () => commands.getRangePatch({ repositoryPath, base: subject.base, head: subject.head })
  void invokeCommand(read).then(result => {
    if (result.status === "error") {
      logError(subject.kind === "commit" ? "Cannot read the changes of a commit" : "Cannot read the changes of a pull request", result.error)
    }
    const loaded: CommitPatchEntry = result.status === "ok" ? { kind: "loaded", patch: result.data } : { kind: "error", message: result.error.message }
    commitPatchStore.setState(state => ({ byCommit: evict(withEntry(state.byCommit, commitId, loaded)) }))
  })
}

/** Forgets the patches of these ids: the ranges of a closed review */
export function dropPatchesOf(ids: readonly string[]): void {
  commitPatchStore.setState(state => ({ byCommit: withoutIds(state.byCommit, ids) }))
}

function withEntry(byCommit: ReadonlyMap<string, CommitPatchEntry>, commitId: string, entry: CommitPatchEntry): Map<string, CommitPatchEntry> {
  const next = new Map(byCommit)
  // re-inserted, so the entry is the most recent
  next.delete(commitId)
  next.set(commitId, entry)
  return next
}

/** Drops the oldest loaded patches over the count or the length; the newest one stays whatever its size */
function evict(byCommit: Map<string, CommitPatchEntry>): Map<string, CommitPatchEntry> {
  const loaded = [...byCommit].filter(([, entry]) => entry.kind === "loaded")
  let length = loaded.reduce((sum, [, entry]) => sum + (entry.kind === "loaded" ? patchLength(entry.patch) : 0), 0)
  let count = loaded.length
  for (const [commitId, entry] of loaded) {
    if (count <= 1 || (count <= MAX_CACHED_PATCHES && length <= MAX_CACHED_PATCH_LENGTH)) {
      break
    }
    byCommit.delete(commitId)
    count--
    length -= entry.kind === "loaded" ? patchLength(entry.patch) : 0
  }
  return byCommit
}

function patchLength(patch: CommitPatch): number {
  return patch.files.reduce((sum, file) => sum + file.patch.length, 0)
}
