import { useEffect, useMemo } from "react"
import { useShallow } from "zustand/react/shallow"
import { useIsFullyLanded } from "@/hooks/useLandedIn"
import type { BranchFilterChip, BranchStateSources } from "@/lib/branch-filter"
import { chipCount, CHIPS, collectChipMatches, FILTER_BAR_MIN_BRANCHES, filterBranches, isFilterActive, keepShown } from "@/lib/branch-filter"
import { clearBranchFilter, rememberShown, useBranchFilter, useShownBranches } from "@/stores/branch-filter"
import type { BranchSyncState } from "@/stores/branch-sync"
import { useBranchSync } from "@/stores/branch-sync"
import { useCommitMove } from "@/stores/commit-move"
import { usePushHolds, useRepositoryRobots } from "@/stores/patronus"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"

type OtherSources = Omit<BranchStateSources, "branches">

/**
 * What the state chips read besides the branches; re-renders only when the robots, the pull requests, the landing or the
 * push holds change
 */
function useOtherSources(): OtherSources {
  const robots = useRepositoryRobots()
  const pullRequests = useRepositoryPullRequestMap()
  const isLanded = useIsFullyLanded()
  const holds = usePushHolds()
  return useMemo(() => ({ robots, pullRequests, isLanded, isHeld: (name: string) => holds.has(name) }), [robots, pullRequests, isLanded, holds])
}

/** Every branch is in a row of the table (`branchTableRowsIn`): the backend order has them all */
function chipMatchesIn(state: BranchSyncState, sources: OtherSources) {
  return collectChipMatches(state.branchOrder, { ...sources, branches: state.branches })
}

/**
 * The filter bar shows: from `FILTER_BAR_MIN_BRANCHES` branches on, or while a filter is active, so a filter left on a
 * smaller table can be cleared; its All chip counts the branches then, and the header leaves the count to it
 */
export function useHasFilterBar(branchCount: number): boolean {
  const filter = useBranchFilter()
  return branchCount >= FILTER_BAR_MIN_BRANCHES || isFilterActive(filter)
}

/** The count of each chip, of all branches; re-renders only when a count changes */
export function useChipCounts(): Readonly<Record<BranchFilterChip, number>> {
  const sources = useOtherSources()
  return useBranchSync(
    useShallow(state => {
      const matches = chipMatchesIn(state, sources)
      return Object.fromEntries(CHIPS.map(chip => [chip, chipCount(matches, chip)])) as Record<BranchFilterChip, number>
    }),
  )
}

/**
 * The branches the table shows under the filter of the selected repository, null while it shows all: the matches, and
 * those shown since the filter last changed (`keepShown`). Re-renders only when the matches change, not on every update
 * of a branch.
 */
export function useFilteredBranches(): ReadonlySet<string> | null {
  const filter = useBranchFilter()
  const sources = useOtherSources()
  // a string, so the store notifies only when the matches change
  const key = useBranchSync(state => (isFilterActive(filter) ? filterBranches(chipMatchesIn(state, sources), state.branches, filter)!.join("\n") : null))
  const matches = useMemo(() => (key === null ? null : key.split("\n").filter(name => name !== "")), [key])
  const shown = useShownBranches()
  useEffect(() => {
    rememberShown(matches)
  }, [matches])
  return matches === null ? null : keepShown(shown, matches)
}

/**
 * A reveal (a branch clicked in the landing overview, "Show auth", moved commits) of a branch that the filter hides
 * shows every branch again, so its row can expand and scroll into view; `shownNames` are the branch rows shown
 */
export function useRevealClearsFilter(shownNames: readonly string[]): void {
  const target = useCommitMove(state => state.reveal?.branchName ?? null)
  // a branch a move creates has no row before the next sync
  const exists = useBranchSync(state => target !== null && state.branches[target] !== undefined)
  const isHidden = exists && !shownNames.includes(target!)
  useEffect(() => {
    if (isHidden) {
      clearBranchFilter()
    }
  }, [isHidden])
}
