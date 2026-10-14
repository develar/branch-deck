import type { UpstreamCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"

/** "Merged as abc12345 by Safe Push Robot, 2 hours ago": the baseline commit that brought an archived branch in */
export function describeIntegrationCommit(commit: UpstreamCommit): string {
  return `Merged as ${formatShortHash(commit.id)} by ${commit.committer}, ${formatRelativeTime(commit.time * 1000)}`
}

/**
 * The tooltip of "Merged" in the archived table: "3 commits, merged as abc12345 by Safe Push Robot, 2 hours ago"; the
 * count or the merge commit alone when the other is unknown, null when both are.
 */
export function describeMergedBranch(commitCount: number, commit: UpstreamCommit | undefined): string | null {
  const count = commitCount > 0 ? formatCommitCount(commitCount) : null
  if (commit === undefined) {
    return count
  }
  const merged = describeIntegrationCommit(commit)
  return count === null ? merged : `${count}, ${merged.charAt(0).toLowerCase()}${merged.slice(1)}`
}

/** The tooltip of "Partially merged · 1 of 2" and of "Not merged" in the archived table */
export function describeUnmergedBranch(integratedCount: number, commitCount: number): string {
  return integratedCount > 0
    ? `${integratedCount} of ${formatCommitCount(commitCount)} merged; the original commits are no longer in HEAD`
    : `${formatCommitCount(commitCount)}, none merged; the original commits are no longer in HEAD`
}

function formatCommitCount(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`
}
