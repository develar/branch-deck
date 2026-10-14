import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { findBranchRobot, formatPatronusAttention } from "@/lib/patronus"
import { formatAuthorAttention } from "@/lib/pull-request"
import type { Branch, BranchTableRow } from "@/stores/branch-sync"

/** A branch that waits for its author: the outgoing indicator gets a dot, its overview a line */
export interface OutgoingAttention {
  readonly branchName: string
  /** "changes requested", "failing checks", "failed Safe Push", joined by ", " */
  readonly reason: string
}

/**
 * The branches of the table, in its order, whose open pull request has changes requested or failing checks, or whose
 * robot failed on the branch as it is (also rebuilt on a newer baseline since)
 */
export function collectOutgoingAttention(
  rows: readonly BranchTableRow[],
  branches: Readonly<Record<string, Branch>>,
  pullRequests: Readonly<Record<string, PullRequest>>,
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
): OutgoingAttention[] {
  const result: OutgoingAttention[] = []
  for (const row of rows) {
    if (row.kind === "issue") {
      continue
    }
    const branch = branches[row.name]
    const localTip = branch?.commits[0]?.hash
    const remote = branch?.remoteStatus ?? null
    const pullRequest = pullRequests[row.name]
    const robot = findBranchRobot(robots, row.name, localTip, remote?.remoteTip ?? null)
    const reasons = [
      pullRequest === undefined ? null : formatAuthorAttention(pullRequest),
      robot === undefined ? null : formatPatronusAttention(robot, localTip, remote),
    ].filter(reason => reason !== null)
    if (reasons.length > 0) {
      result.push({ branchName: row.name, reason: reasons.join(", ") })
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
