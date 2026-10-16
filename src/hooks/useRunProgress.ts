import { useStore } from "zustand"
import { useNow } from "@/hooks/useNow"
import { overallProgress } from "@/lib/update-progress"
import { branchSyncStore, selectSyncFraction } from "@/stores/branch-sync"
import type { RunningUpdate } from "@/stores/update-project"

/** How often the progress of a run renews: the time-based part of a step moves smoothly, the fill eases in between */
const PROGRESS_TICK_MS = 250

/**
 * The progress of the shown run of Update Project, 0 to 1, renewed every `PROGRESS_TICK_MS`; 1 once it ended. Use it
 * only in a component that a run mounts: each tick re-renders it.
 */
export function useRunProgress(shown: RunningUpdate): number {
  const now = useNow(PROGRESS_TICK_MS)
  const syncFraction = useStore(branchSyncStore, state => (shown.syncRun === null ? null : selectSyncFraction(state, shown.syncRun)))
  return shown.endedAt === null ? overallProgress(shown.run, now, syncFraction) : 1
}
