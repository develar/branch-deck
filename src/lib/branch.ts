// Shared branch utilities
import type { SubjectParts } from "@/lib/commit"
import { formatCommitCount, splitSubjectPrefix } from "@/lib/commit"

export function getSimpleBranchName(fullName: string): string {
  const lastSlash = fullName.lastIndexOf("/")
  return lastSlash === -1 ? fullName : fullName.slice(lastSlash + 1)
}

/**
 * What a branch row and the branch preview show after the name, null for nothing: the title of the branch's description
 * as written, else its summary (a subject) with the prefix split off, to be dimmed
 */
export function getBranchSummary(summary: string, descriptionTitle: string | null): SubjectParts | null {
  if (descriptionTitle !== null) {
    return { prefix: null, text: descriptionTitle }
  }
  return summary === "" ? null : splitSubjectPrefix(summary)
}

/** "1 branch", "3 branches" */
export function formatBranchCount(branchCount: number): string {
  return branchCount === 1 ? "1 branch" : `${branchCount} branches`
}

/** "22 branches · 152 commits": the size of a stack, an issue family, a selection, the part of a stack a Safe Push lands */
export function formatBranchesAndCommits(branchCount: number, commitCount: number): string {
  return `${formatBranchCount(branchCount)} · ${formatCommitCount(commitCount)}`
}
