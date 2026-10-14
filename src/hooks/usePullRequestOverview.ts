import { useMemo } from "react"
import { useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryPullRequests } from "@/stores/pull-requests"

export interface PullRequestOverview {
  /** The branches of the table with an open pull request, in the table order */
  openBranches: string[]
}

/** The open pull requests of the branch table, for the header indicator; `useOutgoingAttention` says which wait for the author */
export function usePullRequestOverview(): PullRequestOverview {
  const rows = useBranchTableRows()
  const { pullRequests } = useRepositoryPullRequests()
  return useMemo(() => {
    const openBranches: string[] = []
    for (const row of rows) {
      if (row.kind !== "issue" && pullRequests[row.name]?.state === "open") {
        openBranches.push(row.name)
      }
    }
    return { openBranches }
  }, [rows, pullRequests])
}
