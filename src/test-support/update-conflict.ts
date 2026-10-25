import type { UpdateCommit, UpdateState } from "@/lib/bindings"

/**
 * A conflicting update state (`UpdateState::Conflict`): the first conflict `commit` with the first `files` of
 * `fileCount`, which it changes, and the upstream commits of those files
 */
export function conflictState(files: string[], fileCount: number, commit: UpdateCommit = conflictCommit(), upstreamCommits: UpdateCommit[] = []): UpdateState {
  return { status: "conflict", firstConflict: { commit: { ...commit, files }, files, fileCount, upstreamCommits } }
}

/** The local commit of a conflict whose commit doesn't matter */
function conflictCommit(): UpdateCommit {
  return { id: "b".repeat(40), subject: "(auth) Change line 2", author: "Me", time: 1_700_000_000, committer: "Me", committerTime: 1_700_000_000, files: [] }
}
