import type { PullRequest } from "@/lib/bindings"
import type { BranchTableRow } from "@/stores/branch-sync"

/** An own pull request a reviewer handed back: a row of the Your Turn card */
export interface ReturnedPullRequest {
  readonly branchName: string
  readonly pullRequest: PullRequest
}

/**
 * The branches of the table, in its order, whose open pull request a reviewer handed back (changes requested): the
 * rows of the Your Turn card after the review requests. Failing checks are the chip's, not repeated there.
 */
export function collectReturnedToYou(rows: readonly BranchTableRow[], pullRequests: Readonly<Record<string, PullRequest>>): ReturnedPullRequest[] {
  return collectAttention(rows, pullRequests, pullRequest => (pullRequest.state === "open" && pullRequest.reviewDecision === "changesRequested" ? "changes requested" : null)).map(
    item => ({ branchName: item.branchName, pullRequest: pullRequests[item.branchName]! }),
  )
}

/** "#812 · changes requested by alice, bob": the meta of a returned pull request, the reviewers by their latest review */
export function formatReturnedMeta(pullRequest: PullRequest): string {
  const reviewers = pullRequest.reviewers.filter(it => it.state === "changesRequested").map(it => it.login)
  return reviewers.length === 0 ? `#${pullRequest.number} · changes requested` : `#${pullRequest.number} · changes requested by ${reviewers.join(", ")}`
}

/** A branch of the table whose open pull request waits for its author, and why */
interface Attention {
  readonly branchName: string
  readonly reason: string
}

function collectAttention(
  rows: readonly BranchTableRow[],
  pullRequests: Readonly<Record<string, PullRequest>>,
  reasonOf: (pullRequest: PullRequest) => string | null,
): Attention[] {
  const result: Attention[] = []
  for (const row of rows) {
    if (row.kind === "issue") {
      continue
    }
    const pullRequest = pullRequests[row.name]
    const reason = pullRequest === undefined ? null : reasonOf(pullRequest)
    if (reason !== null) {
      result.push({ branchName: row.name, reason })
    }
  }
  return result
}
