import type { UpdateResolvedCommit, WordResolutionSummary } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"

/** "1 conflict", "3 conflicts" */
function formatConflictCount(count: number): string {
  return count === 1 ? "1 conflict" : `${count} conflicts`
}

/** "Resolved 3 conflicts by words": the notice on the row of a commit whose copy took the resolve by words */
export function resolvedNotice(resolved: WordResolutionSummary): string {
  return `Resolved ${formatConflictCount(resolved.conflicts)} by words`
}

/**
 * What an update resolved by words, lower case: "resolved 3 conflicts by words in “Capitalize line 2”", or "… in 2
 * commits" for more than one; null when it resolved none
 */
export function updateResolvedWords(resolved: readonly UpdateResolvedCommit[] | undefined): string | null {
  const first = resolved?.[0]
  if (resolved === undefined || first === undefined) {
    return null
  }
  const conflicts = formatConflictCount(resolved.reduce((sum, commit) => sum + commit.conflicts, 0))
  return `resolved ${conflicts} by words in ${resolved.length === 1 ? `“${first.subject}”` : formatCommitCount(resolved.length)}`
}

/** The conflicts an update resolves by words, for the words of its check: "resolving 3 conflicts by words" */
export function resolvingWords(resolved: readonly UpdateResolvedCommit[]): string {
  return `resolving ${formatConflictCount(resolved.reduce((sum, commit) => sum + commit.conflicts, 0))} by words`
}
