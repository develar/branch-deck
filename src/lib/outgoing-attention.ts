import type { PullRequest } from "@/lib/bindings"
import { formatAuthorAttention } from "@/lib/pull-request"
import type { BranchTableRow } from "@/stores/branch-sync"

/** A branch that waits for its author: the outgoing indicator gets a dot, its overview a line */
export interface OutgoingAttention {
  readonly branchName: string
  /** "changes requested", "failing checks", joined by ", " */
  readonly reason: string
}

/**
 * The branches of the table, in its order, whose open pull request has changes requested or failing checks. A robot
 * that failed is the robots indicator's (`collectRobotOverview`): it is told once.
 */
export function collectOutgoingAttention(
  rows: readonly BranchTableRow[],
  pullRequests: Readonly<Record<string, PullRequest>>,
): OutgoingAttention[] {
  const result: OutgoingAttention[] = []
  for (const row of rows) {
    if (row.kind === "issue") {
      continue
    }
    const pullRequest = pullRequests[row.name]
    const reason = pullRequest === undefined ? null : formatAuthorAttention(pullRequest)
    if (reason !== null) {
      result.push({ branchName: row.name, reason })
    }
  }
  return result
}

/** "ABC-123 needs attention: changes requested" */
export function formatAttentionLine(attention: OutgoingAttention): string {
  return `${attention.branchName} needs attention: ${attention.reason}`
}

/** The accessible name of the indicator: the title of the overview, then ", 1 needs attention" for the dot */
export function formatIndicatorLabel(title: string, attention: readonly OutgoingAttention[]): string {
  const count = attention.length
  return count === 0 ? title : `${title}, ${count} ${count === 1 ? "needs" : "need"} attention`
}
