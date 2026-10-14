import { useMemo } from "react"
import { needsAuthorAttention } from "@/lib/pull-request"
import { useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryPullRequests } from "@/stores/pull-requests"

export interface PullRequestOverview {
  /** The branches of the table with an open pull request, in the table order */
  openBranches: string[]
  /** An open pull request has changes requested or failing checks */
  needsAttention: boolean
}

/** The open pull requests of the branch table, for the header indicator */
export function usePullRequestOverview(): PullRequestOverview {
  const rows = useBranchTableRows()
  const { pullRequests } = useRepositoryPullRequests()
  return useMemo(() => {
    const openBranches: string[] = []
    let needsAttention = false
    for (const row of rows) {
      if (row.kind === "issue") {
        continue
      }
      const pullRequest = pullRequests[row.name]
      if (pullRequest?.state === "open") {
        openBranches.push(row.name)
        needsAttention ||= needsAuthorAttention(pullRequest)
      }
    }
    return { openBranches, needsAttention }
  }, [rows, pullRequests])
}
