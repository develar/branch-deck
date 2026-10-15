import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { CommitFiles } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, withoutIds } from "@/lib/commit-files"
import { logError } from "@/lib/errors"
import { getDiffRepositoryPath } from "@/stores/repository"

export interface CommitFilesState {
  /**
   * By commit id, or `<base>..<head>` for a range (`diffSubjectId`). The files of a commit never change: an entry is
   * never stale, also after a repository switch; a range is dropped when its review closes (`dropFilesOf`)
   */
  byCommit: ReadonlyMap<string, CommitFiles>
  /** The commits whose files are being read: a list is busy until its commits are done */
  loading: ReadonlySet<string>
  /** The Files section of the commit preview is shown; `F` toggles it */
  isPreviewSectionOpen: boolean
}

export const commitFilesStore = createStore<CommitFilesState>(() => ({
  byCommit: new Map(),
  loading: new Set(),
  isPreviewSectionOpen: true,
}))

/** The loads in flight, by commit id: a second request for an id waits for the first one */
const inFlight = new Map<string, Promise<void>>()

/** The files of a commit once they are loaded */
export function useFilesOfCommit(commitId: string): CommitFiles | undefined {
  return useStore(commitFilesStore, state => state.byCommit.get(commitId))
}

export function getFilesOfCommit(commitId: string): CommitFiles | undefined {
  return commitFilesStore.getState().byCommit.get(commitId)
}

/** Whether the files of one of the commits are being read */
export function useIsLoadingCommitFiles(commitIds: readonly string[]): boolean {
  return useStore(commitFilesStore, state => state.loading.size > 0 && commitIds.some(id => state.loading.has(id)))
}

/**
 * Loads the files of the commits not known yet with one command (`get_commits_files`: one git process for the ids the
 * disk cache misses). A failure is logged, not shown: the rows go without counts, and a later call tries again.
 */
export function loadCommitFiles(commitIds: readonly string[]): Promise<void> {
  return load(commitIds, (repositoryPath, missing) => commands.getCommitsFiles({ repositoryPath, commitIds: missing }))
}

/** Loads the files of one commit, the one of a preview that no list loaded (`get_commit_files`) */
export function loadFilesOfCommit(commitId: string): Promise<void> {
  return load([commitId], async (repositoryPath) => {
    const result = await commands.getCommitFiles({ repositoryPath, commitId })
    return result.status === "ok" ? { status: "ok", data: [result.data] } : result
  })
}

/**
 * Loads the files of a subject of a review: a commit as `loadFilesOfCommit`, a range with `get_range_files` (one
 * `diff-tree` of the two commits, never cached on disk)
 */
export function loadFilesOfSubject(subject: DiffSubject): Promise<void> {
  if (subject.kind === "commit") {
    return loadFilesOfCommit(subject.id)
  }
  return load([diffSubjectId(subject)], async (repositoryPath) => {
    const result = await commands.getRangeFiles({ repositoryPath, base: subject.base, head: subject.head })
    return result.status === "ok" ? { status: "ok", data: [result.data] } : result
  })
}

/** Forgets the files of these ids: the ranges of a closed review */
export function dropFilesOf(ids: readonly string[]): void {
  commitFilesStore.setState(state => ({ byCommit: withoutIds(state.byCommit, ids) }))
}

function load(commitIds: readonly string[], fetch: (repositoryPath: string, missing: string[]) => Promise<CommandResult<CommitFiles[]>>): Promise<void> {
  const { byCommit } = commitFilesStore.getState()
  const waiting: Promise<void>[] = []
  const missing: string[] = []
  for (const id of new Set(commitIds)) {
    const pending = inFlight.get(id)
    if (pending !== undefined) {
      waiting.push(pending)
    }
    else if (!byCommit.has(id)) {
      missing.push(id)
    }
  }
  const repositoryPath = getDiffRepositoryPath()
  if (missing.length > 0 && repositoryPath) {
    const request = fetchInto(repositoryPath, missing, fetch)
    for (const id of missing) {
      inFlight.set(id, request)
    }
    commitFilesStore.setState(state => ({ loading: new Set([...state.loading, ...missing]) }))
    waiting.push(request)
  }
  return Promise.all(waiting).then(() => {})
}

async function fetchInto(repositoryPath: string, missing: string[], fetch: (repositoryPath: string, missing: string[]) => Promise<CommandResult<CommitFiles[]>>): Promise<void> {
  let loaded: readonly CommitFiles[] = []
  try {
    const result = await fetch(repositoryPath, missing)
    if (result.status === "error") {
      logError("Cannot read the files of commits", result.error)
    }
    else {
      loaded = result.data
    }
  }
  catch (error) {
    // the command did not run (IPC)
    logError("Cannot read the files of commits", error)
  }
  for (const id of missing) {
    inFlight.delete(id)
  }
  // one update: the counts appear as the list stops being busy
  commitFilesStore.setState((state) => {
    const byCommit = new Map(state.byCommit)
    for (const files of loaded) {
      byCommit.set(files.commitId, files)
    }
    const loading = new Set(state.loading)
    for (const id of missing) {
      loading.delete(id)
    }
    return { byCommit, loading }
  })
}

export function useIsCommitFilesSectionOpen(): boolean {
  return useStore(commitFilesStore, state => state.isPreviewSectionOpen)
}

export function toggleCommitFilesSection(): void {
  commitFilesStore.setState(state => ({ isPreviewSectionOpen: !state.isPreviewSectionOpen }))
}
