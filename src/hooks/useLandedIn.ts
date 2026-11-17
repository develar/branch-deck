import type { Landed } from "@/lib/bindings"
import { useRowPart } from "@/model/store"
import { useBaselineStatus } from "@/stores/branch-sync"

/** The baseline the update brings in, as the remote names it: "origin/master" */
export function useUpstreamName(): string {
  const status = useBaselineStatus()
  return status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
}

/**
 * Where all of the branch is, as Rust decides what landed it (`landed` of its next step, `domain_policy::landed`): the
 * baseline ("origin/master") while the fetched baseline has every commit of it and the update is still to come (the
 * update archives the branch, there is nothing to push); before the fetch, the target ("master") of a merged Safe Push
 * that landed it with a branch built on it or with the whole stack. Null otherwise: its own robot's branch shows the
 * robot ("Merged"), a merged pull request its chip
 */
export function useLandedIn(name: string): string | null {
  // the words alone, a string: the next step is a new object with every patch of it
  return useRowPart(name, "landing", step => landedInOf(step?.step.landed ?? null))
}

/** The words of where `landed` put all of a branch (`useLandedIn`) */
function landedInOf(landed: Landed | null): string | null {
  switch (landed?.by.kind) {
    case "baseline":
      return landed.by.upstream
    case "robots":
      return landed.by.own ? null : landed.by.target
    default:
      return null
  }
}
