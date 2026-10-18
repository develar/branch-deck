import { useMemo } from "react"
import { fullyLandedNames, isFullyLanded } from "@/lib/landed"
import { isLandedWhole } from "@/lib/patronus"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useBranchLanded, useLanding } from "@/stores/landed"
import { useLandedBy } from "@/stores/patronus"

/** The baseline the update brings in, as the remote names it: "origin/master" */
export function useUpstreamName(): string {
  const status = useBaselineStatus()
  return status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
}

/**
 * The baseline ("origin/master") while the fetched baseline has every commit of the branch and the update is still to
 * come: the update archives the branch, there is nothing to push. Before the fetch, the target ("master") of a merged
 * Safe Push that landed the branch with a branch built on it or with the whole stack (`useLandedBy`). Null otherwise.
 */
export function useLandedIn(name: string): string | null {
  const landed = useBranchLanded(name)
  const upstream = useUpstreamName()
  const landedBy = useLandedBy(name)
  if (landed !== null && isFullyLanded(landed.branch, landed.incoming)) {
    return upstream
  }
  // its own robot's branch shows the robot ("Merged"); one with commits after the landed ones has work left
  return isLandedWhole(landedBy) && landedBy.own !== true ? landedBy.robot.targetBranch : null
}

/** Whether the fetched baseline has every commit of a branch, for the push summary */
export function useIsFullyLanded(): (name: string) => boolean {
  const landing = useLanding()
  return useMemo(() => {
    const names = fullyLandedNames(landing)
    return (name: string) => names.has(name)
  }, [landing])
}
