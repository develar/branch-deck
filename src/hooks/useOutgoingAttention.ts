import { useMemo } from "react"
import type { OutgoingAttention } from "@/lib/outgoing-attention"
import { collectOutgoingAttention } from "@/lib/outgoing-attention"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryRobots } from "@/stores/patronus"
import { useRepositoryPullRequests } from "@/stores/pull-requests"

/** The branches that wait for their author (pull request, Patronus robot), in the table order, for the outgoing indicator */
export function useOutgoingAttention(): readonly OutgoingAttention[] {
  const rows = useBranchTableRows()
  const branches = useBranchSync(state => state.branches)
  const { pullRequests } = useRepositoryPullRequests()
  const robots = useRepositoryRobots()
  return useMemo(() => collectOutgoingAttention(rows, branches, pullRequests, robots), [rows, branches, pullRequests, robots])
}
