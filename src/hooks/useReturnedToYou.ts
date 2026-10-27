import { useMemo } from "react"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { collectReturnedToYou } from "@/lib/outgoing-attention"
import { useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"

/** The own pull requests of the table a reviewer handed back (`collectReturnedToYou`), in the table order */
export function useReturnedToYou(): readonly ReturnedPullRequest[] {
  const rows = useBranchTableRows()
  const pullRequests = useRepositoryPullRequestMap()
  return useMemo(() => collectReturnedToYou(rows, pullRequests), [rows, pullRequests])
}
