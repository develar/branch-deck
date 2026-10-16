import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { BranchFilter, BranchFilterChip } from "@/lib/branch-filter"
import { isFilterActive, keepShown, NO_FILTER } from "@/lib/branch-filter"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"

/**
 * The filter of the branch table, per window and not persisted: a restart or another repository starts without one.
 */
export interface BranchFilterState {
  /** The repository the filter was set for; the filter of another repository is none */
  readonly repositoryPath: string | null
  readonly filter: BranchFilter
  /** The branches the table shows: the matches since the filter last changed (`keepShown`); null while it shows all */
  readonly shown: ReadonlySet<string> | null
}

export const branchFilterStore = createStore<BranchFilterState>(() => ({ repositoryPath: null, filter: NO_FILTER, shown: null }))

function selectedPath(): string | null {
  return getSelectedProject()?.path ?? null
}

function filterIn(state: BranchFilterState, repositoryPath: string | null): BranchFilter {
  return state.repositoryPath === repositoryPath ? state.filter : NO_FILTER
}

function shownIn(state: BranchFilterState, repositoryPath: string | null): ReadonlySet<string> | null {
  return state.repositoryPath === repositoryPath ? state.shown : null
}

/** The filter of the selected repository */
export function useBranchFilter(): BranchFilter {
  const repositoryPath = useSelectedProjectPath() ?? null
  return useStore(branchFilterStore, state => filterIn(state, repositoryPath))
}

export function getBranchFilter(): BranchFilter {
  return filterIn(branchFilterStore.getState(), selectedPath())
}

/** The branches the table shows under the filter of the selected repository, null while it shows all */
export function useShownBranches(): ReadonlySet<string> | null {
  const repositoryPath = useSelectedProjectPath() ?? null
  return useStore(branchFilterStore, state => shownIn(state, repositoryPath))
}

export function getShownBranches(): ReadonlySet<string> | null {
  return shownIn(branchFilterStore.getState(), selectedPath())
}

/** Replaces the filter; the branches shown so far are forgotten, the new filter decides */
function setFilter(filter: BranchFilter): void {
  const current = getBranchFilter()
  if (current.text === filter.text && current.chip === filter.chip) {
    return
  }
  branchFilterStore.setState({ repositoryPath: selectedPath(), filter, shown: null })
}

export function setFilterText(text: string): void {
  setFilter({ ...getBranchFilter(), text })
}

/** Chooses a state chip; the chosen one again goes back to All */
export function toggleFilterChip(chip: BranchFilterChip): void {
  const current = getBranchFilter()
  setFilter({ ...current, chip: current.chip === chip ? "all" : chip })
}

/** Shows every branch again */
export function clearBranchFilter(): void {
  setFilter(NO_FILTER)
}

/**
 * Remembers that the table shows `matches` under the current filter (`keepShown`), so they stay until the filter
 * changes; nothing for an inactive filter or when they are known
 */
export function rememberShown(matches: readonly string[] | null): void {
  const state = branchFilterStore.getState()
  if (matches === null || state.repositoryPath !== selectedPath() || !isFilterActive(state.filter)) {
    return
  }
  const shown = keepShown(state.shown, matches)
  if (shown !== state.shown) {
    branchFilterStore.setState({ shown })
  }
}
