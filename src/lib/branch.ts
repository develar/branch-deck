// Shared branch utilities
import type { IssueLead } from "@/lib/bindings"
import type { SplitSubject } from "@/lib/commit"
import { formatCommitCount, splitSubjectPrefix } from "@/lib/commit"

export function getSimpleBranchName(fullName: string): string {
  const lastSlash = fullName.lastIndexOf("/")
  return lastSlash === -1 ? fullName : fullName.slice(lastSlash + 1)
}

/**
 * What a branch row and the branch preview show after the name, null for nothing: the title of the branch's description
 * as written, else its summary (a subject) with the prefix split off, to be dimmed. `shownIssueKey` is the issue key the
 * row shows already (in the name or as its chip): a title Rust split at that key (`descriptionLead`) shows the rest, as
 * the summary of the sync leaves the key out (`compute_branch_summary`).
 */
export function getBranchSummary(
  summary: string,
  descriptionTitle: string | null,
  shownIssueKey: string | null = null,
  descriptionLead: IssueLead | null = null,
): SplitSubject | null {
  if (descriptionTitle !== null) {
    const text = shownIssueKey !== null && descriptionLead?.key === shownIssueKey ? descriptionLead.text : descriptionTitle
    return text === "" ? null : { prefix: null, text }
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
