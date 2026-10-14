import { useMemo } from "react"
import { useShallow } from "zustand/react/shallow"
import type { PushSummary } from "@/lib/push-summary"
import { summarizePushState } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"

/** What is waiting to be pushed, in the order of the branch table; recomputed only when a branch record changes. */
export function usePushSummary(): PushSummary {
  const rows = useBranchTableRows()
  const branches = useBranchSync(useShallow(state => rows.flatMap((row): Branch[] => {
    const branch = row.startsWith("i:") ? undefined : state.branches[row.slice(2)]
    return branch === undefined ? [] : [branch]
  })))
  return useMemo(() => summarizePushState(branches), [branches])
}
