import { useMemo } from "react"
import { fullyLandedNames, isFullyLanded } from "@/lib/landed"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useBranchLanded, useLanding } from "@/stores/landed"

/** The baseline the update brings in, as the remote names it: "origin/master" */
export function useUpstreamName(): string {
  const status = useBaselineStatus()
  return status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
}

/**
 * The baseline ("origin/master") while the fetched baseline has every commit of the branch and the update is still to
 * come: the update archives the branch, there is nothing to push. Null otherwise.
 */
export function useLandedIn(name: string): string | null {
  const landed = useBranchLanded(name)
  const upstream = useUpstreamName()
  return landed !== null && isFullyLanded(landed.branch, landed.incoming) ? upstream : null
}

/** Whether the fetched baseline has every commit of a branch, for the push summary */
export function useIsFullyLanded(): (name: string) => boolean {
  const landing = useLanding()
  return useMemo(() => {
    const names = fullyLandedNames(landing)
    return (name: string) => names.has(name)
  }, [landing])
}
