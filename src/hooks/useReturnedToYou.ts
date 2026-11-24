import { useMemo } from "react"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { collectReturnedToYou } from "@/lib/outgoing-attention"
import { useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryPullRequestMap, useRepositoryShownReviews } from "@/stores/pull-requests"

/**
 * The own pull requests of the table a reviewer handed back (`collectReturnedToYou`), in the table order, then the
 * reviews after landing handed back
 */
export function useReturnedToYou(): readonly ReturnedPullRequest[] {
  const rows = useBranchTableRows()
  const pullRequests = useRepositoryPullRequestMap()
  const shownReviews = useRepositoryShownReviews()
  return useMemo(() => collectReturnedToYou(rows, pullRequests, shownReviews), [rows, pullRequests, shownReviews])
}
