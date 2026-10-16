import { useMemo } from "react"
import { useShallow } from "zustand/react/shallow"
import { useIsFullyLanded } from "@/hooks/useLandedIn"
import type { PushSummary } from "@/lib/push-summary"
import { summarizePushState } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"
import { usePushHolds } from "@/stores/patronus"

/**
 * What is waiting to be pushed, in the order of the branch table, without the branches the fetched baseline has all of
 * and those a running Safe Push holds; recomputed only when a branch record, the landing or a hold changes.
 */
export function usePushSummary(): PushSummary {
  const rows = useBranchTableRows()
  const branches = useBranchSync(useShallow(state => rows.flatMap((row): Branch[] => {
    const branch = row.kind === "issue" ? undefined : state.branches[row.name]
    return branch === undefined ? [] : [branch]
  })))
  const isLanded = useIsFullyLanded()
  const holds = usePushHolds()
  return useMemo(() => summarizePushState(branches, isLanded, name => holds.has(name)), [branches, isLanded, holds])
}
