import { useMemo } from "react"
import type { OutgoingAttention } from "@/lib/outgoing-attention"
import { collectOutgoingAttention } from "@/lib/outgoing-attention"
import { useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryPullRequests } from "@/stores/pull-requests"

/**
 * The branches whose pull request waits for their author, in the table order, for the outgoing indicator; a failed
 * robot is the robots indicator's
 */
export function useOutgoingAttention(): readonly OutgoingAttention[] {
  const rows = useBranchTableRows()
  const { pullRequests } = useRepositoryPullRequests()
  return useMemo(() => collectOutgoingAttention(rows, pullRequests), [rows, pullRequests])
}
