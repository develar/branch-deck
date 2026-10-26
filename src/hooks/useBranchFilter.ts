import { useEffect, useMemo } from "react"
import { getNodeKindNow, useNodeStores } from "@/hooks/useLandingNode"
import type { BranchChipMatches, BranchFilterChip } from "@/lib/branch-filter"
import { chipCount, CHIPS, collectChipMatches, FILTER_BAR_MIN_BRANCHES, filterBranches, isFilterActive, keepShown, movesByUrgency } from "@/lib/branch-filter"
import { clearBranchFilter, rememberShown, useBranchFilter, useShownBranches } from "@/stores/branch-filter"
import { branchSyncStore, getMainBranchOrder, useBranchSync } from "@/stores/branch-sync"
import { useCommitMove } from "@/stores/commit-move"

/** Every branch of the table is in a row of it (`branchTableRowsIn`): the backend order without the parked ones */
function chipMatchesNow(): BranchChipMatches {
  return collectChipMatches(getMainBranchOrder(), getNodeKindNow)
}

/** The rows F2 steps through now, a fix first (`movesByUrgency`) */
export function getMoveRowsNow(names: readonly string[]): string[] {
  return movesByUrgency(names, getNodeKindNow)
}

/**
 * The filter bar shows: from `FILTER_BAR_MIN_BRANCHES` branches on, or while a filter is active, so a filter left on a
 * smaller table can be cleared; its All chip counts the branches then, and the header leaves the count to it
 */
export function useHasFilterBar(branchCount: number): boolean {
  const filter = useBranchFilter()
  return branchCount >= FILTER_BAR_MIN_BRANCHES || isFilterActive(filter)
}

/**
 * The count of each chip, of all branches, by the node of each row; re-renders only when a count changes, on a change
 * of the stores the nodes read
 */
export function useChipCounts(): Readonly<Record<BranchFilterChip, number>> {
  // a string, so the stores notify only when a count changes
  const key = useNodeStores(() => {
    const matches = chipMatchesNow()
    return CHIPS.map(chip => chipCount(matches, chip)).join(",")
  })
  return useMemo(() => {
    const counts = key.split(",").map(Number)
    return Object.fromEntries(CHIPS.map((chip, index) => [chip, counts[index] ?? 0])) as Record<BranchFilterChip, number>
  }, [key])
}

/**
 * The branches the table shows under the filter of the selected repository, null while it shows all: the matches, and
 * those shown since the filter last changed (`keepShown`). Re-renders only when the matches change, not on every update
 * of a branch.
 */
export function useFilteredBranches(): ReadonlySet<string> | null {
  const filter = useBranchFilter()
  // a string, so the stores notify only when the matches change
  const key = useNodeStores(() => (isFilterActive(filter) ? filterBranches(chipMatchesNow(), branchSyncStore.getState().branches, filter)!.join("\n") : null))
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
