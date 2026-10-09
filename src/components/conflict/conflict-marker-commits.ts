import type { ConflictMarkerCommitInfo, MergeConflictInfo } from "@/lib/bindings"

/** The commits referenced in the conflict markers, without the absent entries of the partial record. */
export function getConflictMarkerCommits(conflict: MergeConflictInfo | null | undefined): Record<string, ConflictMarkerCommitInfo> {
  const result: Record<string, ConflictMarkerCommitInfo> = {}
  const commits = conflict?.conflictMarkerCommits
  if (commits) {
    for (const [key, value] of Object.entries(commits)) {
      if (value) {
        result[key] = value
      }
    }
  }
  return result
}
